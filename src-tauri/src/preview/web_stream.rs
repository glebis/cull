use crate::db_core::models::ImageWithFile;
use crate::db_core::thumbnails;
use crate::preview::state::PreviewState;
use crate::AppState;
use bytes::Bytes;
use http::header;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use parking_lot::Mutex;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;
use std::convert::Infallible;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use subtle::ConstantTimeEq;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

pub const DEFAULT_PREVIEW_WEB_STREAM_HOST: &str = "0.0.0.0";
pub const DEFAULT_PREVIEW_WEB_STREAM_PORT: u16 = 8723;
pub const PREVIEW_WEB_STREAM_CHANGED_EVENT: &str = "preview:web-stream-changed";
const SERVER_PORT_FALLBACK_ATTEMPTS: u16 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewWebStreamToken(String);

impl PreviewWebStreamToken {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(hex::encode(bytes))
    }

    #[doc(hidden)]
    pub fn for_test(value: &str) -> Self {
        Self(value.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewWebStreamStatus {
    pub active: bool,
    pub url: Option<String>,
    pub host: Option<String>,
    pub bound_host: Option<String>,
    pub port: Option<u16>,
    pub remote_access: bool,
}

impl PreviewWebStreamStatus {
    pub fn inactive() -> Self {
        Self {
            active: false,
            url: None,
            host: None,
            bound_host: None,
            port: None,
            remote_access: false,
        }
    }
}

#[derive(Debug, Clone)]
struct PreviewWebStreamHandle {
    status: PreviewWebStreamStatus,
    cancel: CancellationToken,
}

#[derive(Default)]
pub struct PreviewWebStreamController {
    inner: Mutex<Option<PreviewWebStreamHandle>>,
}

impl PreviewWebStreamController {
    pub async fn start(
        &self,
        app: AppHandle,
        requested_host: Option<String>,
        requested_port: Option<u16>,
    ) -> Result<PreviewWebStreamStatus, String> {
        if let Some(handle) = self.inner.lock().as_ref() {
            return Ok(handle.status.clone());
        }

        let bind_host = preview_web_stream_bind_host(requested_host.as_deref());
        let port = requested_port.unwrap_or(DEFAULT_PREVIEW_WEB_STREAM_PORT);
        let (listener, actual_addr) = bind_preview_web_stream_listener(&bind_host, port).await?;
        let public_host = preview_web_stream_public_host(&bind_host);
        let token = PreviewWebStreamToken::generate();
        let url = preview_web_stream_url(&public_host, actual_addr.port(), &token);
        let cancel = CancellationToken::new();
        let status = PreviewWebStreamStatus {
            active: true,
            url: Some(url),
            host: Some(public_host),
            bound_host: Some(actual_addr.ip().to_string()),
            port: Some(actual_addr.port()),
            remote_access: bind_host == "0.0.0.0",
        };

        spawn_preview_web_stream_server(app, listener, token, cancel.clone());
        *self.inner.lock() = Some(PreviewWebStreamHandle {
            status: status.clone(),
            cancel,
        });
        Ok(status)
    }

    pub fn stop(&self) -> PreviewWebStreamStatus {
        if let Some(handle) = self.inner.lock().take() {
            handle.cancel.cancel();
        }
        PreviewWebStreamStatus::inactive()
    }

    pub fn status(&self) -> PreviewWebStreamStatus {
        self.inner
            .lock()
            .as_ref()
            .map(|handle| handle.status.clone())
            .unwrap_or_else(PreviewWebStreamStatus::inactive)
    }
}

pub fn preview_web_stream_url(
    host: &str,
    port: u16,
    token: &PreviewWebStreamToken,
) -> String {
    format!("http://{}:{}/?token={}", host, port, token.as_str())
}

pub fn preview_web_stream_port_candidates(port: u16) -> Vec<u16> {
    if port == 0 {
        return vec![0];
    }

    let mut candidates: Vec<u16> = (0..=SERVER_PORT_FALLBACK_ATTEMPTS)
        .filter_map(|offset| port.checked_add(offset))
        .collect();
    candidates.push(0);
    candidates
}

pub fn token_from_query(query: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == "token").then(|| value.to_string())
    })
}

pub fn constant_time_token_matches(
    expected: &PreviewWebStreamToken,
    provided: Option<&str>,
) -> bool {
    let Some(provided) = provided else {
        return false;
    };
    expected.as_str().as_bytes().ct_eq(provided.as_bytes()).into()
}

fn preview_web_stream_bind_host(requested_host: Option<&str>) -> String {
    match requested_host.unwrap_or(DEFAULT_PREVIEW_WEB_STREAM_HOST) {
        "0.0.0.0" => "0.0.0.0".to_string(),
        "localhost" | "::1" | "127.0.0.1" => "127.0.0.1".to_string(),
        other => {
            crate::safe_eprintln!(
                "Preview Display web stream: rejecting unsupported host '{}', binding to 127.0.0.1",
                other
            );
            "127.0.0.1".to_string()
        }
    }
}

fn preview_web_stream_public_host(bind_host: &str) -> String {
    if bind_host == "0.0.0.0" {
        return local_lan_ipv4().unwrap_or_else(|| "127.0.0.1".to_string());
    }
    "127.0.0.1".to_string()
}

fn local_lan_ipv4() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    match addr.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

async fn bind_preview_web_stream_listener(
    host: &str,
    port: u16,
) -> Result<(tokio::net::TcpListener, SocketAddr), String> {
    let mut last_addr = None;
    let mut last_error = None;
    let candidates = preview_web_stream_port_candidates(port);
    let attempted_count = candidates.len();

    for candidate_port in candidates {
        let addr: SocketAddr = format!("{}:{}", host, candidate_port)
            .parse()
            .map_err(|e| format!("Invalid Preview Display web stream address: {}", e))?;

        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                let actual_addr = listener.local_addr().map_err(|e| {
                    format!("Failed to read Preview Display web stream address: {}", e)
                })?;
                if candidate_port != port {
                    crate::safe_eprintln!(
                        "Preview Display web stream: port {} was occupied, serving on {}",
                        port,
                        actual_addr.port()
                    );
                }
                return Ok((listener, actual_addr));
            }
            Err(err) if err.kind() == ErrorKind::AddrInUse && candidate_port != 0 => {
                last_addr = Some(addr);
                last_error = Some(err);
            }
            Err(err) => {
                return Err(format!(
                    "Failed to start Preview Display web stream on {}: {}",
                    addr, err
                ));
            }
        }
    }

    match (last_addr, last_error) {
        (Some(addr), Some(err)) => Err(format!(
            "Failed to start Preview Display web stream near {} after trying {} ports: {}",
            addr, attempted_count, err
        )),
        _ => Err("Failed to start Preview Display web stream: no candidate ports".to_string()),
    }
}

fn spawn_preview_web_stream_server(
    app: AppHandle,
    listener: tokio::net::TcpListener,
    token: PreviewWebStreamToken,
    cancel: CancellationToken,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                accepted = listener.accept() => {
                    let Ok((stream, _remote_addr)) = accepted else {
                        break;
                    };
                    let io = TokioIo::new(stream);
                    let app = app.clone();
                    let token = token.clone();
                    let cancel = cancel.clone();
                    tokio::spawn(async move {
                        let service = hyper::service::service_fn(move |req: Request<Incoming>| {
                            let app = app.clone();
                            let token = token.clone();
                            let cancel = cancel.clone();
                            async move {
                                Ok::<_, Infallible>(serve_preview_web_stream_request(app, token, cancel, req).await)
                            }
                        });
                        if let Err(err) = hyper::server::conn::http1::Builder::new()
                            .serve_connection(io, service)
                            .await
                        {
                            crate::safe_eprintln!("Preview Display web stream HTTP connection error: {}", err);
                        }
                    });
                }
            }
        }
    });
}

async fn serve_preview_web_stream_request(
    app: AppHandle,
    token: PreviewWebStreamToken,
    cancel: CancellationToken,
    req: Request<Incoming>,
) -> Response<Full<Bytes>> {
    if req.method() != hyper::Method::GET && req.method() != hyper::Method::HEAD {
        return text_response(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed");
    }
    if cancel.is_cancelled()
        || !constant_time_token_matches(&token, req.uri().query().and_then(token_from_query).as_deref())
    {
        return text_response(StatusCode::FORBIDDEN, "Forbidden");
    }

    match req.uri().path() {
        "/" => html_response(preview_web_stream_viewer_html()),
        "/state" => match preview_web_stream_snapshot(&app, &token) {
            Ok(snapshot) => json_response(&snapshot),
            Err(err) => text_response(StatusCode::INTERNAL_SERVER_ERROR, &err),
        },
        path if path.starts_with("/image/") => {
            let image_id = path.trim_start_matches("/image/");
            serve_preview_web_stream_image(app, image_id).await
        }
        _ => text_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

#[derive(Debug, Serialize)]
struct PreviewWebStreamSnapshot {
    preview: PreviewState,
    image: Option<PreviewWebStreamImage>,
}

#[derive(Debug, Serialize)]
struct PreviewWebStreamImage {
    id: String,
    url: String,
    filename: String,
    width: u32,
    height: u32,
    rating: Option<u8>,
    decision: Option<String>,
    source_label: Option<String>,
    missing: bool,
}

fn preview_web_stream_snapshot(
    app: &AppHandle,
    token: &PreviewWebStreamToken,
) -> Result<PreviewWebStreamSnapshot, String> {
    let state = app.state::<AppState>();
    let preview = state.preview_state.get();
    let image = if preview.blanked {
        None
    } else if let Some(image_id) = preview.image_id.as_deref() {
        let images = state
            .db
            .get_images_by_ids(&[image_id])
            .map_err(|e| e.to_string())?;
        images
            .first()
            .map(|image| preview_web_stream_image(image, &state.app_data_dir, token, preview.version))
    } else {
        None
    };

    Ok(PreviewWebStreamSnapshot { preview, image })
}

fn preview_web_stream_image(
    image: &ImageWithFile,
    app_data_dir: &Path,
    token: &PreviewWebStreamToken,
    version: u64,
) -> PreviewWebStreamImage {
    let path = preview_image_file_for_web_stream(image, app_data_dir);
    PreviewWebStreamImage {
        id: image.image.id.clone(),
        url: format!(
            "/image/{}?token={}&v={}",
            url_encode_component(&image.image.id),
            token.as_str(),
            version
        ),
        filename: Path::new(&image.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&image.path)
            .to_string(),
        width: image.image.width,
        height: image.image.height,
        rating: image.selection.as_ref().and_then(|selection| selection.star_rating),
        decision: image
            .selection
            .as_ref()
            .map(|selection| selection.decision.clone()),
        source_label: image.source_label.clone(),
        missing: !path.exists(),
    }
}

async fn serve_preview_web_stream_image(
    app: AppHandle,
    image_id: &str,
) -> Response<Full<Bytes>> {
    if image_id.is_empty() || image_id.contains('/') {
        return text_response(StatusCode::BAD_REQUEST, "Bad Request");
    }

    let path = {
        let state = app.state::<AppState>();
        let images = match state.db.get_images_by_ids(&[image_id]) {
            Ok(images) => images,
            Err(_) => return text_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
        };
        let Some(image) = images.first() else {
            return text_response(StatusCode::NOT_FOUND, "Not Found");
        };
        preview_image_file_for_web_stream(image, &state.app_data_dir)
    };

    let mime = mime_for_image_path(&path);
    match tokio::fs::read(&path).await {
        Ok(bytes) => bytes_response(StatusCode::OK, mime, bytes),
        Err(_) => text_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

fn preview_image_file_for_web_stream(image: &ImageWithFile, app_data_dir: &Path) -> PathBuf {
    let source = PathBuf::from(&image.path);
    if browser_supported_image_path(&source) && source.exists() {
        return source;
    }

    thumbnails::thumbnail_path(app_data_dir, &image.image.id)
}

fn browser_supported_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "gif" | "svg" | "bmp")
    )
}

fn mime_for_image_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn url_encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn preview_web_stream_viewer_html() -> &'static str {
    r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex,nofollow">
<title>Cull Preview Display</title>
<style>
:root { color-scheme: dark; --bg: #08080c; --surface: #0c0c12; --border: #1a1a2e; --text: #e0e0e0; --text-secondary: #7a7fa0; --blue: #7aa2f7; --green: #9ece6a; --orange: #e0af68; --red: #f7768e; font-family: "JetBrains Mono", monospace; }
html, body { margin: 0; width: 100%; height: 100%; background: var(--bg); color: var(--text); overflow: hidden; }
#app { position: fixed; inset: 0; display: grid; grid-template-columns: 1fr auto; }
#stage { position: relative; min-width: 0; min-height: 0; display: grid; place-items: center; background: var(--bg); }
#image { max-width: 100vw; max-height: 100vh; width: auto; height: auto; object-fit: contain; }
#empty { color: var(--text-secondary); font-size: 12px; letter-spacing: 0; }
#overlay { position: absolute; left: 16px; bottom: 16px; display: flex; gap: 12px; align-items: center; padding: 8px 10px; background: rgba(12,12,18,.78); border: 1px solid var(--border); font-size: 12px; }
#rail { width: 280px; display: none; border-left: 1px solid var(--border); background: var(--surface); padding: 16px; box-sizing: border-box; overflow: hidden; }
#rail.visible { display: block; }
.label { color: var(--text-secondary); font-size: 10px; text-transform: uppercase; margin-top: 12px; }
.value { color: var(--text); font-size: 12px; overflow-wrap: anywhere; margin-top: 4px; }
.accent { color: var(--blue); }
</style>
</head>
<body>
<main id="app">
  <section id="stage">
    <img id="image" alt="">
    <div id="empty">Waiting for Preview Display</div>
    <div id="overlay"></div>
  </section>
  <aside id="rail"></aside>
</main>
<script>
const params = new URLSearchParams(location.search);
const token = params.get('token') || '';
const image = document.getElementById('image');
const empty = document.getElementById('empty');
const overlay = document.getElementById('overlay');
const rail = document.getElementById('rail');
let lastVersion = -1;
function text(value) { return value == null || value === '' ? '—' : String(value); }
function escapeHtml(value) {
  return text(value).replace(/[&<>"']/g, (char) => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;'
  })[char]);
}
function render(snapshot) {
  const preview = snapshot.preview;
  const item = snapshot.image;
  if (preview.version === lastVersion && item && image.src.endsWith(item.url)) return;
  lastVersion = preview.version;
  if (!item || preview.blanked) {
    image.removeAttribute('src');
    image.style.display = 'none';
    empty.style.display = 'block';
    empty.textContent = preview.blanked ? 'Preview Display blanked' : 'Waiting for Preview Display';
    overlay.style.display = 'none';
    rail.classList.remove('visible');
    rail.innerHTML = '';
    return;
  }
  image.src = item.url;
  image.style.display = 'block';
  empty.style.display = 'none';
  const parts = [];
  if (preview.overlay.showFilename) parts.push(text(item.filename));
  if (preview.overlay.showRating) parts.push(item.rating ? `${item.rating}★` : 'unrated');
  if (preview.overlay.showDecision) parts.push(text(item.decision));
  overlay.textContent = parts.join('  |  ');
  overlay.style.display = parts.length ? 'flex' : 'none';
  if (preview.overlay.showMetadataRail) {
    rail.classList.add('visible');
    rail.innerHTML = `<div class="label">Filename</div><div class="value accent">${escapeHtml(item.filename)}</div><div class="label">Status</div><div class="value">${escapeHtml(item.decision)}</div><div class="label">Rating</div><div class="value">${item.rating ? `${item.rating} star` : 'unrated'}</div><div class="label">Source</div><div class="value">${escapeHtml(item.source_label)}</div><div class="label">Size</div><div class="value">${item.width} × ${item.height}</div>`;
  } else {
    rail.classList.remove('visible');
    rail.innerHTML = '';
  }
}
async function tick() {
  try {
    const response = await fetch(`/state?token=${encodeURIComponent(token)}`, { cache: 'no-store' });
    if (!response.ok) throw new Error(String(response.status));
    render(await response.json());
  } catch (error) {
    image.style.display = 'none';
    empty.style.display = 'block';
    empty.textContent = 'Preview Display stream unavailable';
    overlay.style.display = 'none';
  }
}
tick();
setInterval(tick, 300);
</script>
</body>
</html>"#
}

fn html_response(html: &'static str) -> Response<Full<Bytes>> {
    bytes_response(StatusCode::OK, "text/html; charset=utf-8", html.as_bytes().to_vec())
}

fn json_response<T: Serialize>(value: &T) -> Response<Full<Bytes>> {
    match serde_json::to_vec(value) {
        Ok(bytes) => bytes_response(StatusCode::OK, "application/json; charset=utf-8", bytes),
        Err(_) => text_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
    }
}

fn text_response(status: StatusCode, text: &str) -> Response<Full<Bytes>> {
    bytes_response(status, "text/plain; charset=utf-8", text.as_bytes().to_vec())
}

fn bytes_response(status: StatusCode, content_type: &str, bytes: Vec<u8>) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::HeaderName::from_static("x-robots-tag"), "noindex, nofollow")
        .header(header::REFERRER_POLICY, "no-referrer")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'self'; img-src 'self' data:; style-src 'unsafe-inline'; script-src 'unsafe-inline'",
        )
        .body(Full::new(Bytes::from(bytes)))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from_static(b"Internal Server Error")))
                .unwrap()
        })
}
