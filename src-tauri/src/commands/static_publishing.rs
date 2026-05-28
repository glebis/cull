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

use crate::db_core::canvas_document::{
    CanvasDocument, CanvasExportBackground, CanvasExportBounds, CanvasItem, CanvasItemFit,
};
use crate::db_core::models::{Canvas, ImageWithFile};
use crate::AppState;

const MODULE_KEY: &str = "module_static_publishing";
const SCHEMA_VERSION: &str = "cull.static_publishing.v1";
const MAX_SNAPSHOT_LONG_EDGE: f64 = 4096.0;

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct StaticPublishCanvasItem {
    pub image_id: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct StaticPublishLink {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct StaticPublishRequest {
    pub canvas_name: String,
    pub items: Vec<StaticPublishCanvasItem>,
    pub layout_json: Option<String>,
    pub output_dir: Option<String>,
    pub share_url: Option<String>,
    #[serde(default)]
    pub site_title: Option<String>,
    #[serde(default)]
    pub site_description: Option<String>,
    #[serde(default)]
    pub indexable: bool,
    #[serde(default)]
    pub links: Vec<StaticPublishLink>,
    pub include_thumbnails: bool,
    pub include_web: bool,
    pub include_full: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct StaticPublishCanvasRequest {
    pub canvas_id: String,
    pub output_dir: Option<String>,
    pub share_url: Option<String>,
    #[serde(default)]
    pub site_title: Option<String>,
    #[serde(default)]
    pub site_description: Option<String>,
    #[serde(default)]
    pub indexable: bool,
    #[serde(default)]
    pub links: Vec<StaticPublishLink>,
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
    pub snapshot_png_path: Option<String>,
    pub snapshot_pdf_path: Option<String>,
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

struct SnapshotExport {
    png_path: String,
    pdf_path: String,
    manifest: Value,
}

#[derive(Debug, Clone, Copy)]
struct SnapshotBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
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
    export_static_publish_package_with_canvas_inner(state, request, None)
}

pub fn export_static_publish_canvas_inner(
    state: &AppState,
    request: StaticPublishCanvasRequest,
) -> Result<StaticPublishResult, String> {
    ensure_module_enabled(state)?;

    let canvas = state
        .db
        .get_canvas(&request.canvas_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Canvas '{}' was not found", request.canvas_id))?;
    let document = CanvasDocument::from_layout_json(&canvas.layout_json)
        .map_err(|e| format!("Invalid canvas layout_json: {}", e))?;
    let layout_json = document
        .to_layout_json()
        .map_err(|e| format!("Invalid canvas layout_json: {}", e))?;
    let items = document
        .items
        .iter()
        .map(|item| StaticPublishCanvasItem {
            image_id: item.image_id.clone(),
            x: Some(item.x),
            y: Some(item.y),
            width: Some(item.width),
            height: Some(item.height),
            hidden: Some(item.hidden),
        })
        .collect();

    export_static_publish_package_with_canvas_inner(
        state,
        StaticPublishRequest {
            canvas_name: canvas.name.clone(),
            items,
            layout_json: Some(layout_json),
            output_dir: request.output_dir,
            share_url: request.share_url,
            site_title: request.site_title.or_else(|| Some(canvas.name.clone())),
            site_description: request.site_description,
            indexable: request.indexable,
            links: request.links,
            include_thumbnails: request.include_thumbnails,
            include_web: request.include_web,
            include_full: request.include_full,
        },
        Some(&canvas),
    )
}

fn export_static_publish_package_with_canvas_inner(
    state: &AppState,
    request: StaticPublishRequest,
    source_canvas: Option<&Canvas>,
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

    let share_url = match request
        .share_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => validate_share_url(raw)?,
        None => "http://localhost:8000/".to_string(),
    };
    let access_phrase = generate_access_phrase();
    let qr_svg_path = site_dir.join("qr.svg");
    write_qr_svg(&qr_svg_path, &share_url)?;
    let site_title = normalize_optional_text(request.site_title.as_deref())
        .or_else(|| normalize_optional_text(Some(&request.canvas_name)))
        .unwrap_or_else(|| "Cull Canvas".to_string());
    let site_description = normalize_optional_text(request.site_description.as_deref());
    let site_links = validate_site_links(&request.links)?;

    let layout: Value = request
        .layout_json
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_else(|| json!({ "type": "ordered_canvas_snapshot" }));
    let canvas_document = request
        .layout_json
        .as_deref()
        .and_then(|raw| CanvasDocument::from_layout_json(raw).ok());
    let snapshot = canvas_document
        .as_ref()
        .and_then(|document| {
            export_canvas_snapshot(state, &document, &images_by_id, &site_dir, &mut warnings)
                .transpose()
        })
        .transpose()?;
    let annotation_export =
        write_annotation_sidecar(&data_dir, source_canvas, canvas_document.as_ref())?;

    let manifest = json!({
        "schema": SCHEMA_VERSION,
        "generated_at": Utc::now().to_rfc3339(),
        "canvas_name": request.canvas_name,
        "canvas": source_canvas.map(|canvas| json!({
            "id": canvas.id,
            "session_id": canvas.session_id,
            "name": canvas.name,
            "type": canvas.canvas_type,
            "updated_at": canvas.updated_at,
        })),
        "share": {
            "url": share_url,
            "qr_svg": "qr.svg",
            "access_phrase": access_phrase,
            "access_note": "Use this phrase for tunnel, reverse proxy, or host-level access control. The static package itself is read-only and does not enforce server-side auth."
        },
        "site": {
            "title": site_title,
            "description": site_description,
            "indexable": request.indexable,
            "links": site_links,
        },
        "variants": {
            "thumb": request.include_thumbnails,
            "web": request.include_web,
            "full": request.include_full
        },
        "layout": layout,
        "annotations": annotation_export,
        "snapshots": snapshot.as_ref().map(|snapshot| snapshot.manifest.clone()),
        "images": manifest_items,
        "warnings": warnings
    });

    let manifest_path = data_dir.join("canvas.json");
    write_json(&manifest_path, &manifest)?;
    fs::write(site_dir.join("index.html"), render_index_html(&manifest))
        .map_err(|e| format!("Failed to write index.html: {}", e))?;
    write_robots_txt(&site_dir, request.indexable)?;
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
        snapshot_png_path: snapshot.as_ref().map(|snapshot| snapshot.png_path.clone()),
        snapshot_pdf_path: snapshot.as_ref().map(|snapshot| snapshot.pdf_path.clone()),
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

fn write_annotation_sidecar(
    data_dir: &Path,
    source_canvas: Option<&Canvas>,
    document: Option<&CanvasDocument>,
) -> Result<Value, String> {
    let Some(document) = document else {
        return Ok(json!({
            "count": 0,
            "file": Value::Null,
            "policy": "No Canvas v1 annotation sidecar was generated because no valid Canvas document was available."
        }));
    };

    let count = document.annotations.len();
    if count == 0 {
        return Ok(json!({
            "count": 0,
            "file": Value::Null,
            "policy": "No Canvas annotations were present at export time."
        }));
    }

    let sidecar = json!({
        "schema": SCHEMA_VERSION,
        "canvas": source_canvas.map(|canvas| json!({
            "id": canvas.id,
            "session_id": canvas.session_id,
            "name": canvas.name,
            "type": canvas.canvas_type,
            "updated_at": canvas.updated_at,
        })),
        "annotations": &document.annotations,
        "policy": "Read-only Canvas annotation sidecar. Add writable comments as a separate data file or service-backed layer."
    });
    write_json(&data_dir.join("annotations.json"), &sidecar)?;

    Ok(json!({
        "count": count,
        "file": "data/annotations.json",
        "policy": "Canvas annotations are exported as a read-only sidecar; writable comment layers must use a separate service or data file."
    }))
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

    let requested_host = host.unwrap_or_else(|| "127.0.0.1".to_string());
    if requested_host != "127.0.0.1" && requested_host != "localhost" && requested_host != "::1" {
        crate::safe_eprintln!(
            "Static publishing: rejecting non-localhost host '{}', binding to 127.0.0.1",
            requested_host
        );
    }
    let host = "127.0.0.1".to_string();
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
                    crate::safe_eprintln!("Static publishing HTTP connection error: {}", err);
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
    let path = if let Some(path) = requested.map(str::trim).filter(|path| !path.is_empty()) {
        PathBuf::from(path)
    } else if let Some(saved) = state
        .db
        .get_setting("static_publishing_output_dir")
        .map_err(|e| e.to_string())?
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
    {
        PathBuf::from(saved)
    } else {
        return Ok(state.app_data_dir.join("static-publishing").join("canvas"));
    };

    // Reject path traversal
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Export path must not contain '..' components".to_string());
    }

    // Ensure path is under home directory or system temp directory
    if let Some(home) = dirs::home_dir() {
        let canonical = if path.exists() {
            path.canonicalize()
                .map_err(|e| format!("Invalid export path: {}", e))?
        } else if let Some(parent) = path.parent().filter(|p| p.exists()) {
            let canonical_parent = parent
                .canonicalize()
                .map_err(|e| format!("Invalid export path parent: {}", e))?;
            canonical_parent.join(path.file_name().unwrap_or_default())
        } else {
            path.clone()
        };
        let home_canonical = home.canonicalize().unwrap_or_else(|_| home.clone());
        let temp_dir = std::env::temp_dir();
        let temp_canonical = temp_dir.canonicalize().unwrap_or(temp_dir);
        if !canonical.starts_with(&home)
            && !canonical.starts_with(&home_canonical)
            && !canonical.starts_with(&temp_canonical)
        {
            return Err(format!(
                "Export path must be under the home directory ({})",
                home.display()
            ));
        }
    }

    Ok(path)
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

fn export_canvas_snapshot(
    state: &AppState,
    document: &CanvasDocument,
    images_by_id: &HashMap<String, ImageWithFile>,
    site_dir: &Path,
    warnings: &mut Vec<String>,
) -> Result<Option<SnapshotExport>, String> {
    let mut render_items: Vec<(&CanvasItem, &ImageWithFile)> = document
        .items
        .iter()
        .filter(|item| !item.hidden)
        .filter_map(|item| images_by_id.get(&item.image_id).map(|image| (item, image)))
        .collect();

    if render_items.is_empty() {
        return Ok(None);
    }

    render_items.sort_by_key(|(item, _)| item.z);
    let Some(bounds) = calculate_snapshot_bounds(&render_items, &document.export.bounds, warnings)
    else {
        return Ok(None);
    };

    let scale = snapshot_scale(bounds);
    let output_width = scaled_dimension(bounds.width, scale);
    let output_height = scaled_dimension(bounds.height, scale);
    let background = background_pixel(&document.export.background);
    let mut canvas = image::RgbaImage::from_pixel(output_width, output_height, background);

    for (item, image) in render_items {
        let source_path = crate::commands::resolve_image_path_for_ml(image, &state.app_data_dir);
        if !source_path.exists() {
            warnings.push(format!(
                "Snapshot skipped image '{}' because source file does not exist at {}",
                image.image.id,
                source_path.display()
            ));
            continue;
        }

        let decoded = image::open(&source_path).map_err(|e| {
            format!(
                "Snapshot could not decode image '{}' at {}: {}",
                image.image.id,
                source_path.display(),
                e
            )
        })?;
        let decoded = if let Some(crop) = item.transform.crop.as_ref() {
            crop_image(decoded, crop)
        } else {
            decoded
        };

        let decoded = rotate_snapshot_image(decoded, item.transform.rotation_degrees);

        let target_width = scaled_dimension(item.width, scale);
        let target_height = scaled_dimension(item.height, scale);
        let rendered =
            render_item_image(&decoded, target_width, target_height, &item.transform.fit);
        let x = scaled_offset(item.x, bounds.x, scale);
        let y = scaled_offset(item.y, bounds.y, scale);
        image::imageops::overlay(&mut canvas, &rendered, x, y);
    }

    let exports_dir = site_dir.join("exports");
    fs::create_dir_all(&exports_dir)
        .map_err(|e| format!("Failed to create snapshot export dir: {}", e))?;
    let png_path = exports_dir.join("canvas.png");
    let pdf_path = exports_dir.join("canvas.pdf");

    canvas
        .save_with_format(&png_path, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to write canvas PNG snapshot: {}", e))?;

    let png_path_str = png_path.to_string_lossy().to_string();
    let pdf_path_str = pdf_path.to_string_lossy().to_string();
    crate::export::pdf::assemble_pdf(
        &[png_path_str.clone()],
        output_width,
        output_height,
        &pdf_path_str,
    )?;

    Ok(Some(SnapshotExport {
        png_path: png_path_str,
        pdf_path: pdf_path_str,
        manifest: json!({
            "policy": {
                "bounds": "content",
                "requested_bounds": snapshot_bounds_name(&document.export.bounds),
                "large_canvas": format!(
                    "Full content bounds are scaled down when the longest edge exceeds {}px.",
                    MAX_SNAPSHOT_LONG_EDGE as u32
                )
            },
            "background": snapshot_background_name(&document.export.background),
            "bounds": {
                "x": bounds.x,
                "y": bounds.y,
                "width": bounds.width,
                "height": bounds.height,
            },
            "output": {
                "width": output_width,
                "height": output_height,
                "scale": scale,
            },
            "png": "exports/canvas.png",
            "pdf": "exports/canvas.pdf",
        }),
    }))
}

fn calculate_snapshot_bounds(
    render_items: &[(&CanvasItem, &ImageWithFile)],
    requested_bounds: &CanvasExportBounds,
    warnings: &mut Vec<String>,
) -> Option<SnapshotBounds> {
    match requested_bounds {
        CanvasExportBounds::Content => {}
        CanvasExportBounds::Viewport => warnings.push(
            "Canvas snapshot bounds 'viewport' are not exportable without viewport dimensions; using full content bounds".to_string(),
        ),
        CanvasExportBounds::Selection => warnings.push(
            "Canvas snapshot bounds 'selection' are not exportable without a saved selection frame; using full content bounds".to_string(),
        ),
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (item, _) in render_items {
        min_x = min_x.min(item.x);
        min_y = min_y.min(item.y);
        max_x = max_x.max(item.x + item.width);
        max_y = max_y.max(item.y + item.height);
    }

    let width = max_x - min_x;
    let height = max_y - min_y;
    if width <= 0.0 || height <= 0.0 {
        None
    } else {
        Some(SnapshotBounds {
            x: min_x,
            y: min_y,
            width,
            height,
        })
    }
}

fn snapshot_scale(bounds: SnapshotBounds) -> f64 {
    let longest_edge = bounds.width.max(bounds.height);
    if longest_edge > MAX_SNAPSHOT_LONG_EDGE {
        MAX_SNAPSHOT_LONG_EDGE / longest_edge
    } else {
        1.0
    }
}

fn scaled_dimension(value: f64, scale: f64) -> u32 {
    ((value * scale).round().max(1.0)) as u32
}

fn scaled_offset(value: f64, origin: f64, scale: f64) -> i64 {
    ((value - origin) * scale).round() as i64
}

fn background_pixel(background: &CanvasExportBackground) -> image::Rgba<u8> {
    match background {
        CanvasExportBackground::Transparent => image::Rgba([0, 0, 0, 0]),
        CanvasExportBackground::Canvas => image::Rgba([8, 8, 12, 255]),
        CanvasExportBackground::White => image::Rgba([255, 255, 255, 255]),
    }
}

fn crop_image(
    image: image::DynamicImage,
    crop: &crate::db_core::canvas_document::CanvasCrop,
) -> image::DynamicImage {
    let width = image.width() as f64;
    let height = image.height() as f64;
    let normalized = crop.x >= 0.0 && crop.y >= 0.0 && crop.width <= 1.0 && crop.height <= 1.0;

    let x = if normalized { crop.x * width } else { crop.x }.clamp(0.0, width - 1.0);
    let y = if normalized { crop.y * height } else { crop.y }.clamp(0.0, height - 1.0);
    let crop_width = if normalized {
        crop.width * width
    } else {
        crop.width
    }
    .clamp(1.0, width - x);
    let crop_height = if normalized {
        crop.height * height
    } else {
        crop.height
    }
    .clamp(1.0, height - y);

    image.crop_imm(
        x.round() as u32,
        y.round() as u32,
        crop_width.round() as u32,
        crop_height.round() as u32,
    )
}

fn rotate_snapshot_image(image: image::DynamicImage, rotation_degrees: f64) -> image::DynamicImage {
    let normalized = ((rotation_degrees / 90.0).round() as i32 * 90).rem_euclid(360);
    match normalized {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    }
}

fn render_item_image(
    image: &image::DynamicImage,
    width: u32,
    height: u32,
    fit: &CanvasItemFit,
) -> image::RgbaImage {
    let filter = image::imageops::FilterType::Lanczos3;
    match fit {
        CanvasItemFit::Stretch => image.resize_exact(width, height, filter).to_rgba8(),
        CanvasItemFit::Cover => image.resize_to_fill(width, height, filter).to_rgba8(),
        CanvasItemFit::Contain => {
            let resized = image.resize(width, height, filter).to_rgba8();
            let x = ((width.saturating_sub(resized.width())) / 2) as i64;
            let y = ((height.saturating_sub(resized.height())) / 2) as i64;
            let mut layer = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
            image::imageops::overlay(&mut layer, &resized, x, y);
            layer
        }
    }
}

fn snapshot_bounds_name(bounds: &CanvasExportBounds) -> &'static str {
    match bounds {
        CanvasExportBounds::Content => "content",
        CanvasExportBounds::Viewport => "viewport",
        CanvasExportBounds::Selection => "selection",
    }
}

fn snapshot_background_name(background: &CanvasExportBackground) -> &'static str {
    match background {
        CanvasExportBackground::Transparent => "transparent",
        CanvasExportBackground::Canvas => "canvas",
        CanvasExportBackground::White => "white",
    }
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

fn write_robots_txt(site_dir: &Path, indexable: bool) -> Result<(), String> {
    let body = if indexable {
        "User-agent: *\nAllow: /\n"
    } else {
        "User-agent: *\nDisallow: /\n"
    };
    fs::write(site_dir.join("robots.txt"), body)
        .map_err(|e| format!("Failed to write robots.txt: {}", e))
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| text.chars().take(300).collect())
}

fn validate_site_links(links: &[StaticPublishLink]) -> Result<Vec<StaticPublishLink>, String> {
    let mut normalized = Vec::new();
    for link in links.iter().take(12) {
        let Some(label) = normalize_optional_text(Some(&link.label)) else {
            continue;
        };
        let url = validate_share_url(&link.url)?;
        normalized.push(StaticPublishLink { label, url });
    }
    Ok(normalized)
}

fn validate_share_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("Share URL must not be empty".to_string());
    }
    let lower = trimmed.to_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(format!(
            "Share URL must use http:// or https:// scheme, got: {}",
            trimmed
        ));
    }
    let with_slash = if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{}/", trimmed)
    };
    Ok(with_slash)
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

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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

fn render_index_html(manifest: &Value) -> String {
    let title = manifest["site"]["title"]
        .as_str()
        .or_else(|| manifest["canvas_name"].as_str())
        .unwrap_or("Cull Canvas");
    let description = manifest["site"]["description"].as_str().unwrap_or("");
    let indexable = manifest["site"]["indexable"].as_bool().unwrap_or(false);
    let robots = if indexable {
        "index,follow"
    } else {
        "noindex,nofollow"
    };
    let description_meta = if description.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"  <meta name="description" content="{}" />
"#,
            html_escape(description)
        )
    };

    let html = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>__TITLE__</title>
__DESCRIPTION_META__  <meta name="robots" content="__ROBOTS__" />
  <style>
    :root { color-scheme: dark; --bg: #08080c; --surface: #0c0c12; --border: #1a1a2e; --text: #e0e0e0; --muted: #7a7fa0; --blue: #7aa2f7; --green: #9ece6a; }
    * { box-sizing: border-box; }
    body { margin: 0; background: var(--bg); color: var(--text); font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
    a { color: var(--blue); }
    .skip-link { position: absolute; left: 12px; top: -48px; z-index: 10; padding: 8px 10px; background: var(--green); color: var(--bg); border-radius: 4px; }
    .skip-link:focus { top: 12px; }
    header { position: sticky; top: 0; z-index: 2; display: flex; justify-content: space-between; gap: 16px; align-items: center; padding: 14px 18px; background: color-mix(in srgb, var(--bg) 92%, transparent); border-bottom: 1px solid var(--border); backdrop-filter: blur(18px); }
    h1 { margin: 0; font-size: 15px; font-weight: 700; }
    h2 { margin: 0; font-size: 13px; }
    .meta { color: var(--muted); font-size: 12px; }
    .description { margin: 4px 0 0; max-width: 70ch; color: var(--muted); font-size: 12px; }
    .site-links { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
    .site-links:empty { display: none; }
    .site-links a { border: 1px solid var(--border); border-radius: 4px; padding: 3px 7px; text-decoration: none; }
    .site-links a:focus-visible, .card:focus-visible, .snapshot-links a:focus-visible, .share a:focus-visible { outline: 2px solid var(--green); outline-offset: 2px; }
    .share { display: flex; gap: 10px; align-items: center; min-width: 0; }
    .share img { width: 56px; height: 56px; border-radius: 4px; background: white; }
    .share a { color: var(--blue); overflow-wrap: anywhere; }
    main { padding: 18px; }
    .snapshot { margin-bottom: 18px; border: 1px solid var(--border); border-radius: 4px; background: var(--surface); overflow: hidden; }
    .snapshot-head { display: flex; justify-content: space-between; gap: 12px; align-items: center; padding: 10px 12px; border-bottom: 1px solid var(--border); }
    .snapshot-links { display: flex; gap: 10px; }
    .snapshot img { display: block; width: 100%; height: auto; background: #ffffff; }
    .gallery-head { display: flex; justify-content: space-between; gap: 12px; align-items: baseline; margin-bottom: 10px; }
    .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; }
    .card { display: block; color: inherit; text-decoration: none; border-radius: 4px; }
    figure { margin: 0; border: 1px solid var(--border); border-radius: 4px; background: var(--surface); overflow: hidden; height: 100%; }
    figure img { display: block; width: 100%; aspect-ratio: 1; object-fit: cover; background: #050508; }
    figcaption { display: grid; gap: 3px; padding: 8px; min-height: 56px; color: var(--muted); font-size: 11px; }
    figcaption strong { color: var(--text); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .prompt { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
    .empty { color: var(--muted); padding: 32px 0; }
    @media (max-width: 640px) {
      header { align-items: flex-start; flex-direction: column; }
      .share img { width: 48px; height: 48px; }
      main { padding: 12px; }
      .gallery-head { flex-direction: column; gap: 4px; }
      .grid { grid-template-columns: 1fr; gap: 8px; }
    }
  </style>
</head>
<body>
  <a class="skip-link" href="#gallery">Skip to gallery</a>
  <header>
    <div>
      <h1 id="title">Cull Canvas</h1>
      <p class="description" id="description" hidden></p>
      <div class="meta" id="summary"></div>
      <nav class="site-links" id="site-links" aria-label="Related links"></nav>
    </div>
    <div class="share">
      <img id="qr" alt="QR code" src="qr.svg" />
      <a id="share-url" href="#"></a>
    </div>
  </header>
  <main>
    <section id="snapshot" class="snapshot" hidden>
      <div class="snapshot-head">
        <strong>Canvas snapshot</strong>
        <div class="snapshot-links">
          <a id="snapshot-png" href="#">PNG</a>
          <a id="snapshot-pdf" href="#">PDF</a>
        </div>
      </div>
      <img id="snapshot-image" alt="Canvas snapshot" />
    </section>
    <section id="gallery" aria-labelledby="gallery-title">
      <div class="gallery-head">
        <h2 id="gallery-title">Images</h2>
        <div class="meta" id="gallery-summary" role="status"></div>
      </div>
      <div id="grid" class="grid"></div>
    </section>
    <p id="empty" class="empty" hidden>No images in this package.</p>
  </main>
  <script>
    const grid = document.getElementById('grid');
    const empty = document.getElementById('empty');
    const title = document.getElementById('title');
    const description = document.getElementById('description');
    const summary = document.getElementById('summary');
    const gallerySummary = document.getElementById('gallery-summary');
    const siteLinks = document.getElementById('site-links');
    const shareUrl = document.getElementById('share-url');
    const snapshot = document.getElementById('snapshot');
    const snapshotImage = document.getElementById('snapshot-image');
    const snapshotPng = document.getElementById('snapshot-png');
    const snapshotPdf = document.getElementById('snapshot-pdf');

    fetch('./data/canvas.json')
      .then(response => response.json())
      .then(data => {
        const images = data.images || [];
        const site = data.site || {};
        const pageTitle = site.title || data.canvas_name || 'Cull Canvas';
        title.textContent = pageTitle;
        document.title = pageTitle;
        if (site.description) {
          description.hidden = false;
          description.textContent = site.description;
        }
        const generated = new Date(data.generated_at).toLocaleString();
        summary.textContent = `${images.length} images · Published ${generated}`;
        gallerySummary.textContent = `${images.length} image${images.length === 1 ? '' : 's'}`;
        shareUrl.textContent = data.share?.url || window.location.href;
        shareUrl.href = data.share?.url || window.location.href;
        for (const link of site.links || []) {
          if (!link.label || !link.url) continue;
          const a = document.createElement('a');
          a.href = link.url;
          a.textContent = link.label;
          a.rel = 'noopener noreferrer';
          siteLinks.append(a);
        }
        empty.hidden = images.length !== 0;
        if (data.snapshots?.png) {
          snapshot.hidden = false;
          snapshotImage.src = data.snapshots.png;
          snapshotImage.alt = `${pageTitle} canvas snapshot`;
          snapshotPng.href = data.snapshots.png;
          snapshotPdf.href = data.snapshots.pdf || data.snapshots.png;
        }

        for (const item of images) {
          const href = item.files?.full || item.files?.web || item.files?.thumb;
          const src = item.files?.web || item.files?.thumb || item.files?.full;
          if (!src) continue;
          const card = document.createElement('a');
          card.className = 'card';
          card.href = href || src;
          const fig = document.createElement('figure');
          const img = document.createElement('img');
          img.loading = 'lazy';
          img.src = src;
          img.alt = item.ai_prompt || item.filename || item.id;
          const cap = document.createElement('figcaption');
          const name = document.createElement('strong');
          name.textContent = item.filename || item.id;
          const details = document.createElement('span');
          const meta = [];
          if (Number.isFinite(item.width) && Number.isFinite(item.height)) meta.push(`${item.width}x${item.height}`);
          if (Number.isFinite(item.rating)) meta.push(`${item.rating} stars`);
          if (item.source_label) meta.push(item.source_label);
          details.textContent = meta.join(' · ') || 'Image';
          cap.append(name, details);
          if (item.ai_prompt) {
            const prompt = document.createElement('span');
            prompt.className = 'prompt';
            prompt.textContent = item.ai_prompt;
            cap.append(prompt);
          }
          fig.append(img, cap);
          card.append(fig);
          grid.append(card);
        }
      })
      .catch(error => {
        empty.hidden = false;
        empty.textContent = `Could not load canvas.json: ${error}`;
      });
  </script>
</body>
</html>
"##;
    html.replace("__TITLE__", &html_escape(title))
        .replace("__DESCRIPTION_META__", &description_meta)
        .replace("__ROBOTS__", robots)
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
    let snapshot_lines = manifest["snapshots"]
        .as_object()
        .map(|_| "- PNG snapshot: `exports/canvas.png`\n- PDF snapshot: `exports/canvas.pdf`\n")
        .unwrap_or("");
    format!(
        r#"# Cull Static Publishing Handoff

Build or publish this package as a read-only presentation site.

## Package

- Site root: `{}`
- Manifest: `data/canvas.json`
- Annotation sidecar: `data/annotations.json` when Canvas notes are present
- Images: `images/`
- QR code: `qr.svg`
- Share URL for QR: `{}`
- Access phrase: `{}`
- Images exported: `{}`
{}

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
- Preserve `data/annotations.json` as the read-only annotation/comment sidecar when present.
- Preserve original image filenames in captions unless asked to editorialize them.
- Add writable annotation/commenting only as a separate data file or service-backed layer.
- Do not overwrite source library files.
- When generating responsive assets, keep `images/*-thumb.jpg` and `images/*-web.jpg` naming stable.
"#,
        site_dir.display(),
        share_url,
        access_phrase,
        image_count,
        snapshot_lines
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
            site_title: None,
            site_description: None,
            indexable: false,
            links: Vec::new(),
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

        let source_path = tmp.path().join("source.png");
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
                site_title: None,
                site_description: None,
                indexable: false,
                links: Vec::new(),
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
    fn export_saved_canvas_uses_persisted_layout_items() {
        let (state, tmp) = test_state();
        state.db.set_setting(MODULE_KEY, "true").unwrap();

        let source_path = tmp.path().join("source.png");
        write_test_image(&source_path);
        let image_id =
            crate::db_core::import::import_file(&state.db, &source_path, &state.app_data_dir)
                .unwrap()
                .unwrap();
        let session_id = state
            .db
            .create_session("Saved Canvas Session", &tmp.path().to_string_lossy())
            .unwrap();
        let canvas_id = state
            .db
            .create_canvas(&session_id, "Saved Canvas", "manual")
            .unwrap();
        let missing_image_id = "missing-image";
        let layout_json = format!(
            r#"{{
                "version": 1,
                "viewport": {{ "panX": 4, "panY": 8, "zoom": 1.2 }},
                "items": [
                    {{
                        "id": "canvas-item-b",
                        "imageId": "{}",
                        "x": 220,
                        "y": 40,
                        "width": 320,
                        "height": 180,
                        "z": 2,
                        "hidden": true,
                        "label": "Hero",
                        "groupId": null
                    }},
                    {{
                        "id": "canvas-item-missing",
                        "imageId": "{}",
                        "x": 0,
                        "y": 0,
                        "width": 100,
                        "height": 100,
                        "z": 1,
                        "hidden": false,
                        "label": null,
                        "groupId": null
                    }}
                ],
                "groups": [],
                "connectors": [],
                "annotations": [],
                "export": {{ "defaultPresetId": null, "background": "transparent", "bounds": "content" }}
            }}"#,
            image_id, missing_image_id
        );
        state
            .db
            .update_canvas_layout(&canvas_id, &layout_json)
            .unwrap();

        let result = export_static_publish_canvas_inner(
            &state,
            StaticPublishCanvasRequest {
                canvas_id: canvas_id.clone(),
                output_dir: Some(tmp.path().join("exports").to_string_lossy().to_string()),
                share_url: None,
                site_title: None,
                site_description: None,
                indexable: false,
                links: Vec::new(),
                include_thumbnails: true,
                include_web: false,
                include_full: false,
            },
        )
        .unwrap();

        assert_eq!(result.image_count, 1);
        assert_eq!(result.skipped_count, 1);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains(missing_image_id)));

        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(&result.manifest_path).expect("manifest should be readable"),
        )
        .unwrap();
        assert_eq!(manifest["canvas"]["id"], canvas_id);
        assert_eq!(manifest["canvas"]["name"], "Saved Canvas");
        assert_eq!(manifest["layout"]["viewport"]["zoom"], 1.2);

        let first_image = &manifest["images"][0];
        assert_eq!(first_image["id"], image_id);
        assert_eq!(first_image["canvas"]["x"], 220.0);
        assert_eq!(first_image["canvas"]["y"], 40.0);
        assert_eq!(first_image["canvas"]["width"], 320.0);
        assert_eq!(first_image["canvas"]["height"], 180.0);
        assert_eq!(first_image["canvas"]["hidden"], true);
    }

    #[test]
    fn export_saved_canvas_writes_full_content_png_and_pdf_snapshot() {
        let (state, tmp) = test_state();
        state.db.set_setting(MODULE_KEY, "true").unwrap();

        let source_path = tmp.path().join("source.png");
        write_test_image(&source_path);
        let image_id =
            crate::db_core::import::import_file(&state.db, &source_path, &state.app_data_dir)
                .unwrap()
                .unwrap();
        let session_id = state
            .db
            .create_session("Snapshot Session", &tmp.path().to_string_lossy())
            .unwrap();
        let canvas_id = state
            .db
            .create_canvas(&session_id, "Snapshot Canvas", "manual")
            .unwrap();
        let layout_json = format!(
            r#"{{
                "version": 1,
                "viewport": {{ "panX": 0, "panY": 0, "zoom": 1 }},
                "items": [
                    {{
                        "id": "visible-item",
                        "imageId": "{}",
                        "x": 120,
                        "y": 80,
                        "width": 160,
                        "height": 120,
                        "z": 2,
                        "hidden": false,
                        "label": "Visible",
                        "groupId": null
                    }},
                    {{
                        "id": "hidden-item",
                        "imageId": "{}",
                        "x": -500,
                        "y": -500,
                        "width": 100,
                        "height": 100,
                        "z": 1,
                        "hidden": true,
                        "label": null,
                        "groupId": null
                    }}
                ],
                "groups": [],
                "connectors": [],
                "annotations": [],
                "export": {{ "defaultPresetId": null, "background": "white", "bounds": "content" }}
            }}"#,
            image_id, image_id
        );
        state
            .db
            .update_canvas_layout(&canvas_id, &layout_json)
            .unwrap();

        let result = export_static_publish_canvas_inner(
            &state,
            StaticPublishCanvasRequest {
                canvas_id,
                output_dir: Some(tmp.path().join("exports").to_string_lossy().to_string()),
                share_url: None,
                site_title: None,
                site_description: None,
                indexable: false,
                links: Vec::new(),
                include_thumbnails: true,
                include_web: false,
                include_full: false,
            },
        )
        .unwrap();

        let png_path = PathBuf::from(result.snapshot_png_path.unwrap());
        let pdf_path = PathBuf::from(result.snapshot_pdf_path.unwrap());
        assert!(png_path.exists());
        assert!(pdf_path.exists());

        let snapshot = image::open(&png_path).unwrap().to_rgba8();
        assert_eq!(snapshot.dimensions(), (160, 120));
        assert_eq!(snapshot.get_pixel(80, 60).0, [32, 96, 160, 255]);

        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(&result.manifest_path).expect("manifest should be readable"),
        )
        .unwrap();
        assert_eq!(manifest["snapshots"]["bounds"]["x"], 120.0);
        assert_eq!(manifest["snapshots"]["bounds"]["y"], 80.0);
        assert_eq!(manifest["snapshots"]["bounds"]["width"], 160.0);
        assert_eq!(manifest["snapshots"]["bounds"]["height"], 120.0);
        assert_eq!(manifest["snapshots"]["policy"]["bounds"], "content");
        assert_eq!(
            manifest["snapshots"]["policy"]["requested_bounds"],
            "content"
        );
        assert_eq!(manifest["snapshots"]["png"], "exports/canvas.png");
        assert_eq!(manifest["snapshots"]["pdf"], "exports/canvas.pdf");
    }

    #[test]
    fn export_saved_canvas_writes_annotation_sidecar() {
        let (state, tmp) = test_state();
        state.db.set_setting(MODULE_KEY, "true").unwrap();

        let source_path = tmp.path().join("source.png");
        write_test_image(&source_path);
        let image_id =
            crate::db_core::import::import_file(&state.db, &source_path, &state.app_data_dir)
                .unwrap()
                .unwrap();
        let session_id = state
            .db
            .create_session("Annotation Session", &tmp.path().to_string_lossy())
            .unwrap();
        let canvas_id = state
            .db
            .create_canvas(&session_id, "Annotation Canvas", "manual")
            .unwrap();
        let layout_json = format!(
            r#"{{
                "version": 1,
                "viewport": {{ "panX": 0, "panY": 0, "zoom": 1 }},
                "items": [{{
                    "id": "visible-item",
                    "imageId": "{}",
                    "x": 0,
                    "y": 0,
                    "width": 160,
                    "height": 120,
                    "z": 0,
                    "hidden": false,
                    "label": null,
                    "groupId": null
                }}],
                "groups": [],
                "connectors": [],
                "annotations": [{{
                    "id": "note-a",
                    "target": {{ "type": "item", "itemId": "visible-item" }},
                    "body": "Use this crop",
                    "x": 0.5,
                    "y": 0.5,
                    "createdAt": "2026-05-16T10:00:00Z",
                    "author": null
                }}],
                "export": {{ "defaultPresetId": null, "background": "white", "bounds": "content" }}
            }}"#,
            image_id
        );
        state
            .db
            .update_canvas_layout(&canvas_id, &layout_json)
            .unwrap();

        let result = export_static_publish_canvas_inner(
            &state,
            StaticPublishCanvasRequest {
                canvas_id,
                output_dir: Some(tmp.path().join("exports").to_string_lossy().to_string()),
                share_url: None,
                site_title: None,
                site_description: None,
                indexable: false,
                links: Vec::new(),
                include_thumbnails: true,
                include_web: false,
                include_full: false,
            },
        )
        .unwrap();

        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(&result.manifest_path).expect("manifest should be readable"),
        )
        .unwrap();
        assert_eq!(manifest["annotations"]["count"], 1);
        assert_eq!(manifest["annotations"]["file"], "data/annotations.json");

        let sidecar_path = PathBuf::from(&result.site_dir).join("data/annotations.json");
        let sidecar: Value = serde_json::from_str(
            &fs::read_to_string(sidecar_path).expect("annotation sidecar should be readable"),
        )
        .unwrap();
        assert_eq!(sidecar["annotations"][0]["body"], "Use this crop");
    }

    #[test]
    fn snapshot_rotation_applies_quarter_turns() {
        let image: image::RgbaImage =
            image::ImageBuffer::from_pixel(2, 3, image::Rgba([32, 96, 160, 255]));
        let rotated = rotate_snapshot_image(image::DynamicImage::ImageRgba8(image), 90.0);

        assert_eq!(rotated.width(), 3);
        assert_eq!(rotated.height(), 2);
    }

    #[test]
    fn validate_share_url_accepts_https() {
        let result = validate_share_url("https://example.com/gallery");
        assert_eq!(result.unwrap(), "https://example.com/gallery/");
    }

    #[test]
    fn validate_share_url_accepts_http() {
        let result = validate_share_url("http://localhost:8000");
        assert_eq!(result.unwrap(), "http://localhost:8000/");
    }

    #[test]
    fn validate_share_url_preserves_trailing_slash() {
        let result = validate_share_url("https://example.com/");
        assert_eq!(result.unwrap(), "https://example.com/");
    }

    #[test]
    fn validate_share_url_rejects_javascript() {
        let result = validate_share_url("javascript:alert(1)");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("http://"));
    }

    #[test]
    fn validate_share_url_rejects_data() {
        let result = validate_share_url("data:text/html,<script>alert(1)</script>");
        assert!(result.is_err());
    }

    #[test]
    fn validate_share_url_rejects_vbscript() {
        let result = validate_share_url("vbscript:MsgBox(1)");
        assert!(result.is_err());
    }

    #[test]
    fn validate_share_url_rejects_case_insensitive() {
        let result = validate_share_url("JAVASCRIPT:alert(1)");
        assert!(result.is_err());
    }

    #[test]
    fn validate_share_url_rejects_empty() {
        let result = validate_share_url("  ");
        assert!(result.is_err());
    }

    #[test]
    fn export_writes_site_customization_and_robots_policy() {
        let (state, tmp) = test_state();
        state.db.set_setting(MODULE_KEY, "true").unwrap();

        let source_path = tmp.path().join("source.png");
        write_test_image(&source_path);
        let image_id =
            crate::db_core::import::import_file(&state.db, &source_path, &state.app_data_dir)
                .unwrap()
                .unwrap();

        let result = export_static_publish_package_inner(
            &state,
            StaticPublishRequest {
                canvas_name: "Private Sketches".to_string(),
                items: vec![StaticPublishCanvasItem {
                    image_id,
                    x: None,
                    y: None,
                    width: None,
                    height: None,
                    hidden: None,
                }],
                layout_json: None,
                output_dir: Some(tmp.path().join("exports").to_string_lossy().to_string()),
                share_url: Some("https://example.test/private".to_string()),
                site_title: Some("Client Review".to_string()),
                site_description: Some("Shortlisted image directions.".to_string()),
                indexable: false,
                links: vec![StaticPublishLink {
                    label: "Project brief".to_string(),
                    url: "https://example.test/brief".to_string(),
                }],
                include_thumbnails: true,
                include_web: false,
                include_full: false,
            },
        )
        .unwrap();

        let site_dir = PathBuf::from(&result.site_dir);
        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(&result.manifest_path).expect("manifest should be readable"),
        )
        .unwrap();
        assert_eq!(manifest["site"]["title"], "Client Review");
        assert_eq!(
            manifest["site"]["description"],
            "Shortlisted image directions."
        );
        assert_eq!(manifest["site"]["indexable"], false);
        assert_eq!(manifest["site"]["links"][0]["label"], "Project brief");
        assert_eq!(
            manifest["site"]["links"][0]["url"],
            "https://example.test/brief/"
        );

        let index_html = fs::read_to_string(site_dir.join("index.html")).unwrap();
        assert!(index_html.contains("<title>Client Review</title>"));
        assert!(index_html.contains(r#"<meta name="robots" content="noindex,nofollow" />"#));
        assert!(index_html
            .contains(r#"<meta name="description" content="Shortlisted image directions." />"#));

        let robots = fs::read_to_string(site_dir.join("robots.txt")).unwrap();
        assert!(robots.contains("Disallow: /"));
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
