use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;
use chrono::Utc;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use qrcode::render::svg;
use qrcode::QrCode;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::db_core::models::ImageWithFile;
use crate::AppState;

const MODULE_KEY: &str = "module_static_publishing";
const SCHEMA_VERSION: &str = "cull.static_publishing.v1";

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct StaticPublishCanvasItem {
    pub image_id: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct StaticPublishRequest {
    pub canvas_name: String,
    pub items: Vec<StaticPublishCanvasItem>,
    pub layout_json: Option<String>,
    pub output_dir: Option<String>,
    pub share_url: Option<String>,
    pub include_thumbnails: bool,
    pub include_web: bool,
    pub include_full: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticPublishResult {
    pub export_dir: String,
    pub site_dir: String,
    pub manifest_path: String,
    pub instructions_path: String,
    pub qr_svg_path: String,
    pub qr_target_url: String,
    pub access_phrase: String,
    pub image_count: usize,
    pub skipped_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticPublishServerResult {
    pub url: String,
    pub host: String,
    pub port: u16,
    pub site_dir: String,
}

#[derive(Debug, Clone)]
struct VariantPaths {
    thumb: Option<String>,
    web: Option<String>,
    full: Option<String>,
}

#[tauri::command]
pub async fn export_static_publish_package(
    state: State<'_, AppState>,
    request: StaticPublishRequest,
) -> Result<StaticPublishResult, String> {
    export_static_publish_package_inner(state.inner(), request)
}

pub fn export_static_publish_package_inner(
    state: &AppState,
    request: StaticPublishRequest,
) -> Result<StaticPublishResult, String> {
    ensure_module_enabled(state)?;
    if request.items.is_empty() {
        return Err("No canvas items were provided".to_string());
    }
    if !request.include_thumbnails && !request.include_web && !request.include_full {
        return Err("At least one image variant must be enabled".to_string());
    }

    let id_refs: Vec<&str> = request
        .items
        .iter()
        .map(|item| item.image_id.as_str())
        .collect();
    let images = state
        .db
        .get_images_by_ids(&id_refs)
        .map_err(|e| e.to_string())?;
    let images_by_id: HashMap<String, ImageWithFile> = images
        .into_iter()
        .map(|img| (img.image.id.clone(), img))
        .collect();

    let export_root = resolve_export_root(state, request.output_dir.as_deref())?;
    let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let slug = slugify(&request.canvas_name);
    let export_dir = export_root.join(format!("{}-{}", slug, stamp));
    let site_dir = export_dir.join("site");
    let data_dir = site_dir.join("data");
    let image_dir = site_dir.join("images");
    let instructions_dir = export_dir.join("instructions");
    fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create data dir: {}", e))?;
    fs::create_dir_all(&image_dir).map_err(|e| format!("Failed to create image dir: {}", e))?;
    fs::create_dir_all(&instructions_dir)
        .map_err(|e| format!("Failed to create instructions dir: {}", e))?;

    let mut warnings = Vec::new();
    let mut skipped_count = 0;
    let mut manifest_items = Vec::new();

    for (idx, canvas_item) in request.items.iter().enumerate() {
        let Some(image) = images_by_id.get(&canvas_item.image_id) else {
            warnings.push(format!(
                "Image '{}' was not found or has no present file",
                canvas_item.image_id
            ));
            skipped_count += 1;
            continue;
        };

        match export_image_variants(state, image, &image_dir, idx, &request) {
            Ok(variants) => {
                if variants.thumb.is_none() && variants.web.is_none() && variants.full.is_none() {
                    warnings.push(format!(
                        "No variants were written for image '{}'",
                        image.image.id
                    ));
                    skipped_count += 1;
                    continue;
                }
                manifest_items.push(json!({
                    "id": image.image.id,
                    "filename": Path::new(&image.path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("image"),
                    "width": image.image.width,
                    "height": image.image.height,
                    "format": image.image.format,
                    "source_label": image.source_label,
                    "ai_prompt": image.image.ai_prompt,
                    "rating": image.selection.as_ref().and_then(|selection| selection.star_rating),
                    "decision": image.selection.as_ref().map(|selection| selection.decision.clone()),
                    "canvas": {
                        "x": canvas_item.x,
                        "y": canvas_item.y,
                        "width": canvas_item.width,
                        "height": canvas_item.height,
                        "hidden": canvas_item.hidden.unwrap_or(false),
                    },
                    "files": {
                        "thumb": variants.thumb,
                        "web": variants.web,
                        "full": variants.full,
                    }
                }));
            }
            Err(err) => {
                warnings.push(format!("Image '{}' skipped: {}", image.image.id, err));
                skipped_count += 1;
            }
        }
    }

    let share_url = clean_share_url(request.share_url.as_deref())
        .unwrap_or_else(|| "http://localhost:8000/".to_string());
    let access_phrase = generate_access_phrase();
    let qr_svg_path = site_dir.join("qr.svg");
    write_qr_svg(&qr_svg_path, &share_url)?;

    let layout: Value = request
        .layout_json
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_else(|| json!({ "type": "ordered_canvas_snapshot" }));

    let manifest = json!({
        "schema": SCHEMA_VERSION,
        "generated_at": Utc::now().to_rfc3339(),
        "canvas_name": request.canvas_name,
        "share": {
            "url": share_url,
            "qr_svg": "qr.svg",
            "access_phrase": access_phrase,
            "access_note": "Use this phrase for tunnel, reverse proxy, or host-level access control. The static package itself is read-only and does not enforce server-side auth."
        },
        "variants": {
            "thumb": request.include_thumbnails,
            "web": request.include_web,
            "full": request.include_full
        },
        "layout": layout,
        "images": manifest_items,
        "warnings": warnings
    });

    let manifest_path = data_dir.join("canvas.json");
    write_json(&manifest_path, &manifest)?;
    fs::write(site_dir.join("index.html"), render_index_html())
        .map_err(|e| format!("Failed to write index.html: {}", e))?;
    let instructions_path = instructions_dir.join("CLAUDE.md");
    fs::write(
        &instructions_path,
        render_claude_handoff(&manifest, &site_dir, &share_url, &access_phrase),
    )
    .map_err(|e| format!("Failed to write Claude handoff: {}", e))?;

    Ok(StaticPublishResult {
        export_dir: export_dir.to_string_lossy().to_string(),
        site_dir: site_dir.to_string_lossy().to_string(),
        manifest_path: manifest_path.to_string_lossy().to_string(),
        instructions_path: instructions_path.to_string_lossy().to_string(),
        qr_svg_path: qr_svg_path.to_string_lossy().to_string(),
        qr_target_url: share_url,
        access_phrase,
        image_count: manifest["images"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or(0),
        skipped_count,
        warnings,
    })
}

#[tauri::command]
pub async fn serve_static_publish_package(
    state: State<'_, AppState>,
    site_dir: String,
    host: Option<String>,
    port: Option<u16>,
) -> Result<StaticPublishServerResult, String> {
    serve_static_publish_package_inner(state.inner(), site_dir, host, port).await
}

pub async fn serve_static_publish_package_inner(
    state: &AppState,
    site_dir: String,
    host: Option<String>,
    port: Option<u16>,
) -> Result<StaticPublishServerResult, String> {
    ensure_module_enabled(state)?;

    let site_dir = PathBuf::from(site_dir);
    if !site_dir.join("index.html").exists() {
        return Err(format!(
            "No static package index.html exists at {}",
            site_dir.display()
        ));
    }

    let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
    let port = port.unwrap_or(8000);
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| format!("Invalid server address: {}", e))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to start static server on {}: {}", addr, e))?;
    let actual_addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to read server address: {}", e))?;
    let site_root = Arc::new(site_dir.clone());

    tauri::async_runtime::spawn(async move {
        loop {
            let Ok((stream, _remote_addr)) = listener.accept().await else {
                break;
            };
            let io = TokioIo::new(stream);
            let site_root = site_root.clone();
            tokio::spawn(async move {
                let service = hyper::service::service_fn(move |req: Request<Incoming>| {
                    let site_root = site_root.clone();
                    async move { Ok::<_, Infallible>(serve_static_file(site_root, req).await) }
                });
                if let Err(err) = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, service)
                    .await
                {
                    eprintln!("Static publishing HTTP connection error: {}", err);
                }
            });
        }
    });

    Ok(StaticPublishServerResult {
        url: format!("http://{}:{}/", actual_addr.ip(), actual_addr.port()),
        host: actual_addr.ip().to_string(),
        port: actual_addr.port(),
        site_dir: site_dir.to_string_lossy().to_string(),
    })
}

fn ensure_module_enabled(state: &AppState) -> Result<(), String> {
    let enabled = state
        .db
        .get_setting(MODULE_KEY)
        .map_err(|e| e.to_string())?
        .map(|value| value == "true")
        .unwrap_or(false);
    if enabled {
        Ok(())
    } else {
        Err("Static Publishing module is disabled in Settings".to_string())
    }
}

async fn serve_static_file(
    site_root: Arc<PathBuf>,
    req: Request<Incoming>,
) -> Response<Full<Bytes>> {
    if req.method() != hyper::Method::GET && req.method() != hyper::Method::HEAD {
        return text_response(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed");
    }

    let Some(path) = request_path_to_file(site_root.as_ref(), req.uri().path()) else {
        return text_response(StatusCode::BAD_REQUEST, "Bad Request");
    };

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = mime_for_path(&path);
            Response::builder()
                .status(StatusCode::OK)
                .header(http::header::CONTENT_TYPE, mime)
                .header(http::header::CACHE_CONTROL, "no-store")
                .body(Full::new(Bytes::from(bytes)))
                .unwrap_or_else(|_| {
                    text_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
                })
        }
        Err(_) => text_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

fn request_path_to_file(site_root: &Path, request_path: &str) -> Option<PathBuf> {
    let clean = request_path.trim_start_matches('/');
    let clean = if clean.is_empty() {
        "index.html"
    } else {
        clean
    };
    let path = Path::new(clean);
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(site_root.join(path))
}

fn text_response(status: StatusCode, text: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Full::new(Bytes::from(text.to_string())))
        .unwrap()
}

fn mime_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn resolve_export_root(state: &AppState, requested: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = requested.map(str::trim).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if let Some(saved) = state
        .db
        .get_setting("static_publishing_output_dir")
        .map_err(|e| e.to_string())?
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
    {
        return Ok(PathBuf::from(saved));
    }

    Ok(state.app_data_dir.join("static-publishing").join("canvas"))
}

fn export_image_variants(
    state: &AppState,
    image: &ImageWithFile,
    image_dir: &Path,
    index: usize,
    request: &StaticPublishRequest,
) -> Result<VariantPaths, String> {
    let source_path = crate::commands::resolve_image_path_for_ml(image, &state.app_data_dir);
    if !source_path.exists() {
        return Err(format!(
            "source file does not exist at {}",
            source_path.display()
        ));
    }

    let base = format!("{:04}-{}", index + 1, slugify(&image.image.id));
    let mut thumb = None;
    let mut web = None;
    let mut full = None;

    if request.include_thumbnails || request.include_web {
        let decoded = image::open(&source_path)
            .map_err(|e| format!("could not decode source image: {}", e))?;
        if request.include_thumbnails {
            let rel = format!("images/{}-thumb.jpg", base);
            let path = image_dir.join(format!("{}-thumb.jpg", base));
            decoded
                .thumbnail(420, 420)
                .save_with_format(&path, image::ImageFormat::Jpeg)
                .map_err(|e| format!("could not write thumbnail: {}", e))?;
            thumb = Some(rel);
        }
        if request.include_web {
            let rel = format!("images/{}-web.jpg", base);
            let path = image_dir.join(format!("{}-web.jpg", base));
            decoded
                .thumbnail(1800, 1800)
                .save_with_format(&path, image::ImageFormat::Jpeg)
                .map_err(|e| format!("could not write web image: {}", e))?;
            web = Some(rel);
        }
    }

    if request.include_full {
        let ext = sanitize_ext(
            source_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("jpg"),
        );
        let rel = format!("images/{}-full.{}", base, ext);
        let path = image_dir.join(format!("{}-full.{}", base, ext));
        fs::copy(&source_path, &path).map_err(|e| format!("could not copy full image: {}", e))?;
        full = Some(rel);
    }

    Ok(VariantPaths { thumb, web, full })
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let payload = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(path, payload).map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

fn write_qr_svg(path: &Path, target: &str) -> Result<(), String> {
    let code =
        QrCode::new(target.as_bytes()).map_err(|e| format!("Failed to build QR code: {}", e))?;
    let image = code
        .render::<svg::Color>()
        .min_dimensions(280, 280)
        .dark_color(svg::Color("#08080c"))
        .light_color(svg::Color("#ffffff"))
        .build();
    fs::write(path, image).map_err(|e| format!("Failed to write QR code: {}", e))
}

fn clean_share_url(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.ends_with('/') {
                value.to_string()
            } else {
                format!("{}/", value)
            }
        })
}

fn generate_access_phrase() -> String {
    const WORDS: &[&str] = &[
        "amber", "atlas", "basil", "beacon", "birch", "bright", "canvas", "cedar", "cobalt",
        "comet", "coral", "delta", "ember", "field", "forest", "garden", "harbor", "hazel",
        "indigo", "island", "juniper", "lantern", "meadow", "mint", "north", "olive", "orbit",
        "pearl", "pixel", "prairie", "quartz", "river", "saffron", "silver", "sketch", "solar",
        "spruce", "studio", "sumac", "tempo", "tundra", "violet", "willow", "winter", "zenith",
    ];
    let mut rng = rand::thread_rng();
    (0..3)
        .filter_map(|_| WORDS.choose(&mut rng).copied())
        .collect::<Vec<_>>()
        .join("-")
}

fn sanitize_ext(ext: &str) -> String {
    let cleaned: String = ext
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(8)
        .flat_map(|ch| ch.to_lowercase())
        .collect();
    if cleaned.is_empty() {
        "jpg".to_string()
    } else if cleaned == "jpeg" {
        "jpg".to_string()
    } else {
        cleaned
    }
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "canvas".to_string()
    } else {
        slug
    }
}

fn render_index_html() -> &'static str {
    r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Cull Canvas</title>
  <style>
    :root { color-scheme: dark; --bg: #08080c; --surface: #0c0c12; --border: #1a1a2e; --text: #e0e0e0; --muted: #7a7fa0; --blue: #7aa2f7; --green: #9ece6a; }
    * { box-sizing: border-box; }
    body { margin: 0; background: var(--bg); color: var(--text); font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
    header { position: sticky; top: 0; z-index: 2; display: flex; justify-content: space-between; gap: 16px; align-items: center; padding: 14px 18px; background: color-mix(in srgb, var(--bg) 92%, transparent); border-bottom: 1px solid var(--border); backdrop-filter: blur(18px); }
    h1 { margin: 0; font-size: 15px; font-weight: 700; }
    .meta { color: var(--muted); font-size: 12px; }
    .share { display: flex; gap: 10px; align-items: center; min-width: 0; }
    .share img { width: 56px; height: 56px; border-radius: 4px; background: white; }
    .share a { color: var(--blue); overflow-wrap: anywhere; }
    main { padding: 18px; }
    .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; }
    figure { margin: 0; border: 1px solid var(--border); border-radius: 4px; background: var(--surface); overflow: hidden; }
    figure img { display: block; width: 100%; aspect-ratio: 1; object-fit: cover; background: #050508; }
    figcaption { display: grid; gap: 3px; padding: 8px; min-height: 56px; color: var(--muted); font-size: 11px; }
    figcaption strong { color: var(--text); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .empty { color: var(--muted); padding: 32px 0; }
    @media (max-width: 640px) {
      header { align-items: flex-start; flex-direction: column; }
      .share img { width: 48px; height: 48px; }
      main { padding: 12px; }
      .grid { grid-template-columns: repeat(auto-fill, minmax(138px, 1fr)); gap: 8px; }
    }
  </style>
</head>
<body>
  <header>
    <div>
      <h1 id="title">Cull Canvas</h1>
      <div class="meta" id="summary"></div>
    </div>
    <div class="share">
      <img id="qr" alt="QR code" src="qr.svg" />
      <a id="share-url" href="#"></a>
    </div>
  </header>
  <main>
    <div id="grid" class="grid"></div>
    <p id="empty" class="empty" hidden>No images in this package.</p>
  </main>
  <script>
    const grid = document.getElementById('grid');
    const empty = document.getElementById('empty');
    const title = document.getElementById('title');
    const summary = document.getElementById('summary');
    const shareUrl = document.getElementById('share-url');

    fetch('./data/canvas.json')
      .then(response => response.json())
      .then(data => {
        const images = data.images || [];
        title.textContent = data.canvas_name || 'Cull Canvas';
        summary.textContent = `${images.length} images · ${new Date(data.generated_at).toLocaleString()}`;
        shareUrl.textContent = data.share?.url || window.location.href;
        shareUrl.href = data.share?.url || window.location.href;
        empty.hidden = images.length !== 0;

        for (const item of images) {
          const src = item.files?.web || item.files?.thumb || item.files?.full;
          if (!src) continue;
          const fig = document.createElement('figure');
          const img = document.createElement('img');
          img.loading = 'lazy';
          img.src = src;
          img.alt = item.ai_prompt || item.filename || item.id;
          const cap = document.createElement('figcaption');
          const name = document.createElement('strong');
          name.textContent = item.filename || item.id;
          const details = document.createElement('span');
          const rating = item.rating ? `${item.rating} stars` : 'unrated';
          details.textContent = `${item.width}x${item.height} · ${rating}`;
          cap.append(name, details);
          fig.append(img, cap);
          grid.append(fig);
        }
      })
      .catch(error => {
        empty.hidden = false;
        empty.textContent = `Could not load canvas.json: ${error}`;
      });
  </script>
</body>
</html>
"##
}

fn render_claude_handoff(
    manifest: &Value,
    site_dir: &Path,
    share_url: &str,
    access_phrase: &str,
) -> String {
    let image_count = manifest["images"]
        .as_array()
        .map(|items| items.len())
        .unwrap_or(0);
    format!(
        r#"# Cull Static Publishing Handoff

Build or publish this package as a read-only presentation site.

## Package

- Site root: `{}`
- Manifest: `data/canvas.json`
- Images: `images/`
- QR code: `qr.svg`
- Share URL for QR: `{}`
- Access phrase: `{}`
- Images exported: `{}`

## Intended Workflow

1. Preview locally from the site root:
   `python3 -m http.server 8000`
2. Expose the preview with Cloudflare Tunnel, Tailscale Funnel, a VPN-only host, or a static host.
3. Use the access phrase with host-level protection when the provider supports it.
4. Keep the interface read-only unless an annotation or moderation layer is explicitly added.
5. If publishing to Vercel, create a static project whose public root is this `site` folder.
6. If publishing to S3-compatible storage, upload the contents of `site` with cacheable image assets and no writable endpoints.

## Agent Editing Contract

- Preserve `data/canvas.json` as the source of truth.
- Preserve original image filenames in captions unless asked to editorialize them.
- Add annotation/commenting only as a separate data file or service-backed layer.
- Do not overwrite source library files.
- When generating responsive assets, keep `images/*-thumb.jpg` and `images/*-web.jpg` naming stable.
"#,
        site_dir.display(),
        share_url,
        access_phrase,
        image_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::secrets::MemoryStore;
    use crate::{services, watcher, AppState};

    #[test]
    fn slugify_defaults_to_canvas() {
        assert_eq!(slugify(""), "canvas");
        assert_eq!(slugify("Current Canvas!"), "current-canvas");
    }

    #[test]
    fn sanitize_ext_removes_unexpected_chars() {
        assert_eq!(sanitize_ext("jpeg"), "jpg");
        assert_eq!(sanitize_ext("PNG"), "png");
        assert_eq!(sanitize_ext("../bad"), "bad");
    }

    #[test]
    fn access_phrase_has_three_words() {
        assert_eq!(generate_access_phrase().split('-').count(), 3);
    }

    #[test]
    fn export_rejects_when_module_is_disabled() {
        let (state, _tmp) = test_state();
        let request = StaticPublishRequest {
            canvas_name: "Disabled".to_string(),
            items: vec![StaticPublishCanvasItem {
                image_id: "missing".to_string(),
                x: None,
                y: None,
                width: None,
                height: None,
                hidden: None,
            }],
            layout_json: None,
            output_dir: None,
            share_url: None,
            include_thumbnails: true,
            include_web: false,
            include_full: false,
        };

        let err = export_static_publish_package_inner(&state, request).unwrap_err();
        assert!(err.contains("disabled"));
    }

    #[test]
    fn export_writes_static_package_for_enabled_module() {
        let (state, tmp) = test_state();
        state.db.set_setting(MODULE_KEY, "true").unwrap();

        let source_path = tmp.path().join("source.jpg");
        write_test_image(&source_path);
        let image_id =
            crate::db_core::import::import_file(&state.db, &source_path, &state.app_data_dir)
                .unwrap()
                .unwrap();
        let output_dir = tmp.path().join("exports");

        let result = export_static_publish_package_inner(
            &state,
            StaticPublishRequest {
                canvas_name: "Month Sketches".to_string(),
                items: vec![StaticPublishCanvasItem {
                    image_id,
                    x: Some(10.0),
                    y: Some(20.0),
                    width: Some(320.0),
                    height: Some(240.0),
                    hidden: Some(false),
                }],
                layout_json: Some(r#"{"type":"test_canvas"}"#.to_string()),
                output_dir: Some(output_dir.to_string_lossy().to_string()),
                share_url: Some("https://example.test/canvas".to_string()),
                include_thumbnails: true,
                include_web: true,
                include_full: false,
            },
        )
        .unwrap();

        assert_eq!(result.image_count, 1);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.qr_target_url, "https://example.test/canvas/");
        assert_eq!(result.access_phrase.split('-').count(), 3);

        let site_dir = PathBuf::from(&result.site_dir);
        assert!(site_dir.join("index.html").exists());
        assert!(PathBuf::from(&result.qr_svg_path).exists());
        assert!(PathBuf::from(&result.instructions_path).exists());

        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(&result.manifest_path).expect("manifest should be readable"),
        )
        .unwrap();
        assert_eq!(manifest["schema"], SCHEMA_VERSION);
        assert_eq!(manifest["share"]["url"], "https://example.test/canvas/");
        assert_eq!(manifest["layout"]["type"], "test_canvas");

        let first_image = &manifest["images"][0];
        let thumb = first_image["files"]["thumb"].as_str().unwrap();
        let web = first_image["files"]["web"].as_str().unwrap();
        assert!(site_dir.join(thumb).exists());
        assert!(site_dir.join(web).exists());
        assert!(first_image["files"]["full"].is_null());
    }

    #[test]
    fn request_path_rejects_parent_dirs() {
        let root = PathBuf::from("/tmp/site");
        assert!(request_path_to_file(&root, "/../secret").is_none());
        assert_eq!(
            request_path_to_file(&root, "/").unwrap(),
            PathBuf::from("/tmp/site/index.html")
        );
    }

    fn test_state() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();
        let app_data_dir = tmp.path().join("app-data");
        let model_dir = tmp.path().join("models");
        fs::create_dir_all(&app_data_dir).unwrap();

        let state = AppState {
            db,
            app_data_dir,
            embedding_engine: parking_lot::Mutex::new(EmbeddingEngine::new(&model_dir)),
            detection_engine: parking_lot::Mutex::new(DetectionEngine::new_yolo(&model_dir)),
            safety_engine: parking_lot::Mutex::new(DetectionEngine::new_nudenet(&model_dir)),
            secrets: Box::new(MemoryStore::new()),
            jobs: services::jobs::JobRegistry::default(),
            action_manager: services::undo::ActionManager::new(),
            file_watcher: parking_lot::Mutex::new(watcher::FileWatcher::new()),
        };
        (state, tmp)
    }

    fn write_test_image(path: &Path) {
        let image: image::RgbImage =
            image::ImageBuffer::from_pixel(48, 32, image::Rgb([32, 96, 160]));
        image.save(path).unwrap();
    }
}
