use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CAPTURE_DIR_SETTING: &str = "clipboard_monitor_capture_dir";
pub const LAST_COLLECTION_SETTING: &str = "clipboard_monitor_last_collection_id";
pub const DEFAULT_POLL_MS: u64 = 750;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClipboardAccessStatus {
    Supported,
    UnsupportedPlatform,
    PermissionRequired,
    PermissionDenied,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ClipboardCapture {
    pub bytes: Vec<u8>,
    pub extension: String,
    pub original_filename: Option<String>,
    pub source_url: Option<String>,
    pub source_app: Option<String>,
    pub change_count: Option<i64>,
}

#[derive(Debug, Default)]
pub struct ClipboardMonitorState {
    pub running: bool,
    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
    pub capture_dir: Option<PathBuf>,
    pub captured_count: u32,
    pub last_change_count: Option<i64>,
    pub last_hash: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardMonitorSession {
    pub collection_id: String,
    pub collection_name: String,
    pub capture_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardCaptureResult {
    pub imported: bool,
    pub image_id: Option<String>,
    pub path: String,
    pub filename: String,
}

pub fn default_capture_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("Clipboard Captures")
}

pub fn resolve_capture_dir(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    requested: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(path) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        return validate_capture_dir(PathBuf::from(path));
    }
    if let Some(saved) = db
        .get_setting(CAPTURE_DIR_SETTING)
        .map_err(|e| e.to_string())?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return validate_capture_dir(PathBuf::from(saved));
    }
    Ok(default_capture_dir(app_data_dir))
}

fn validate_capture_dir(path: PathBuf) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("Clipboard capture folder must not contain '..' components".to_string());
    }
    if path.is_absolute() {
        Ok(path)
    } else {
        Err("Clipboard capture folder must be an absolute path".to_string())
    }
}

pub fn build_clipboard_capture_filename(
    capture: &ClipboardCapture,
    now: DateTime<Utc>,
    sequence: u32,
) -> String {
    let timestamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();
    let label = capture
        .original_filename
        .as_deref()
        .and_then(file_stem_label)
        .or_else(|| capture.source_url.as_deref().and_then(host_label))
        .unwrap_or_else(|| "clipboard".to_string());
    let ext = sanitize_extension(&capture.extension);
    format!("{timestamp}_{label}_{sequence:03}.{ext}")
}

pub fn create_monitor_session(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    requested_capture_dir: Option<&str>,
) -> Result<ClipboardMonitorSession, String> {
    let capture_dir = resolve_capture_dir(db, app_data_dir, requested_capture_dir)?;
    std::fs::create_dir_all(&capture_dir)
        .map_err(|e| format!("Failed to create clipboard capture folder: {}", e))?;
    db.set_setting(CAPTURE_DIR_SETTING, &capture_dir.to_string_lossy())
        .map_err(|e| e.to_string())?;

    let now = Utc::now();
    let collection_name = format!("Clipboard {}", now.format("%Y.%m.%d %H:%M"));
    let collection_id = db
        .create_collection(&collection_name)
        .map_err(|e| e.to_string())?;
    db.set_setting(LAST_COLLECTION_SETTING, &collection_id)
        .map_err(|e| e.to_string())?;
    let settings = serde_json::json!({
        "source": "clipboard_monitor",
        "capture_dir": capture_dir.to_string_lossy(),
        "started_at": now.to_rfc3339(),
    });
    db.set_collection_settings_json(&collection_id, &settings.to_string())
        .map_err(|e| e.to_string())?;

    Ok(ClipboardMonitorSession {
        collection_id,
        collection_name,
        capture_dir: capture_dir.to_string_lossy().to_string(),
    })
}

pub fn capture_clipboard_image(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    session: &ClipboardMonitorSession,
    capture: &ClipboardCapture,
    sequence: u32,
) -> Result<ClipboardCaptureResult, String> {
    let capture_dir = PathBuf::from(&session.capture_dir);
    std::fs::create_dir_all(&capture_dir)
        .map_err(|e| format!("Failed to create clipboard capture folder: {}", e))?;
    let filename = unique_capture_filename(&capture_dir, capture, sequence);
    let path = capture_dir.join(&filename);
    write_capture_file(&path, &capture.bytes)?;

    let image_id = crate::db_core::import::import_file(db, &path, app_data_dir)?;
    let mut imported = false;
    if let Some(image_id) = image_id.as_deref() {
        let already_in_collection = db
            .list_collection_images(&session.collection_id)
            .map_err(|e| e.to_string())?
            .iter()
            .any(|image| image.image.id == image_id);
        db.add_to_collection(&session.collection_id, &[image_id])
            .map_err(|e| e.to_string())?;
        imported = !already_in_collection;
        if imported {
            let batch = db
                .create_import_batch("clipboard", 1, Some(&session.collection_id))
                .map_err(|e| e.to_string())?;
            let _ = db.set_image_batch(image_id, &batch);
        }
    }

    Ok(ClipboardCaptureResult {
        imported,
        image_id,
        path: path.to_string_lossy().to_string(),
        filename,
    })
}

fn unique_capture_filename(
    capture_dir: &Path,
    capture: &ClipboardCapture,
    start_sequence: u32,
) -> String {
    let now = Utc::now();
    for sequence in start_sequence..start_sequence.saturating_add(1000) {
        let filename = build_clipboard_capture_filename(capture, now, sequence);
        if !capture_dir.join(&filename).exists() {
            return filename;
        }
    }
    build_clipboard_capture_filename(capture, now, start_sequence)
}

fn write_capture_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| "Capture path has no file name".to_string())?
        .to_string_lossy();
    let tmp = path.with_file_name(format!(".{}.tmp", file_name));
    std::fs::write(&tmp, bytes).map_err(|e| format!("Failed to write clipboard capture: {}", e))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("Failed to finalize clipboard capture: {}", e))?;
    Ok(())
}

fn file_stem_label(filename: &str) -> Option<String> {
    let stem = Path::new(filename).file_stem()?.to_string_lossy();
    slug_component(&stem)
}

fn host_label(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme.split('/').next()?.trim();
    let host = host.strip_prefix("www.").unwrap_or(host);
    let first = host.split('.').next().unwrap_or(host);
    slug_component(first)
}

fn sanitize_extension(extension: &str) -> String {
    let ext = extension
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if ext.is_empty() {
        "png".to_string()
    } else {
        ext
    }
}

fn slug_component(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            previous_dash = false;
        } else if !previous_dash && !out.is_empty() {
            out.push('-');
            previous_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use chrono::{TimeZone, Utc};
    use image::{ImageBuffer, Rgba};
    use tempfile::tempdir;

    fn capture(
        original_filename: Option<&str>,
        source_url: Option<&str>,
        extension: &str,
    ) -> ClipboardCapture {
        ClipboardCapture {
            bytes: vec![1, 2, 3],
            extension: extension.to_string(),
            original_filename: original_filename.map(str::to_string),
            source_url: source_url.map(str::to_string),
            source_app: None,
            change_count: Some(42),
        }
    }

    fn png_bytes(color: [u8; 4]) -> Vec<u8> {
        let image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(2, 2, Rgba(color));
        let mut bytes = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn filename_prefers_original_filename_and_local_24h_timestamp() {
        let now = Utc.with_ymd_and_hms(2026, 5, 30, 12, 35, 22).unwrap();
        let name = build_clipboard_capture_filename(
            &capture(
                Some("Living Room Ref@2x.JPG"),
                Some("https://www.pinterest.com/pin/123"),
                "png",
            ),
            now,
            1,
        );
        assert_eq!(name, "2026-05-30_12-35-22_living-room-ref-2x_001.png");
    }

    #[test]
    fn filename_falls_back_to_source_host() {
        let now = Utc.with_ymd_and_hms(2026, 5, 30, 12, 36, 8).unwrap();
        let name = build_clipboard_capture_filename(
            &capture(None, Some("https://dribbble.com/shots/abc"), "png"),
            now,
            2,
        );
        assert_eq!(name, "2026-05-30_12-36-08_dribbble_002.png");
    }

    #[test]
    fn filename_falls_back_to_clipboard_and_sanitizes_extension() {
        let now = Utc.with_ymd_and_hms(2026, 5, 30, 12, 36, 41).unwrap();
        let name = build_clipboard_capture_filename(&capture(None, None, "../PNG"), now, 3);
        assert_eq!(name, "2026-05-30_12-36-41_clipboard_003.png");
    }

    #[test]
    fn start_session_creates_collection_and_metadata() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let tmp = tempdir().unwrap();
        let app_data = tmp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();

        let session = create_monitor_session(&db, &app_data, None).unwrap();

        assert!(session.collection_name.starts_with("Clipboard "));
        let settings = db
            .get_collection_settings_json(&session.collection_id)
            .unwrap()
            .unwrap();
        assert!(settings.contains(r#""source":"clipboard_monitor""#));
        assert!(std::path::Path::new(&session.capture_dir).exists());
    }

    #[test]
    fn capture_bytes_writes_file_imports_and_adds_to_collection() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let tmp = tempdir().unwrap();
        let app_data = tmp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let session = create_monitor_session(&db, &app_data, None).unwrap();
        let capture = ClipboardCapture {
            bytes: png_bytes([255, 0, 0, 255]),
            extension: "png".to_string(),
            original_filename: Some("Red Reference.png".to_string()),
            source_url: Some("https://www.pinterest.com/pin/red".to_string()),
            source_app: None,
            change_count: Some(1),
        };

        let result = capture_clipboard_image(&db, &app_data, &session, &capture, 1).unwrap();

        assert_eq!(result.imported, true);
        assert!(std::path::Path::new(&result.path).exists());
        assert!(result.path.ends_with("_red-reference_001.png"));
        let images = db.list_collection_images(&session.collection_id).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].image.id, result.image_id.unwrap());
    }

    #[test]
    fn duplicate_capture_hash_is_reported_as_existing_import() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let tmp = tempdir().unwrap();
        let app_data = tmp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let session = create_monitor_session(&db, &app_data, None).unwrap();
        let capture = ClipboardCapture {
            bytes: png_bytes([0, 255, 0, 255]),
            extension: "png".to_string(),
            original_filename: None,
            source_url: None,
            source_app: None,
            change_count: Some(1),
        };

        let first = capture_clipboard_image(&db, &app_data, &session, &capture, 1).unwrap();
        let second = capture_clipboard_image(&db, &app_data, &session, &capture, 2).unwrap();

        assert!(first.imported);
        assert!(!second.imported);
        assert_eq!(db.list_collection_images(&session.collection_id).unwrap().len(), 1);
    }
}
