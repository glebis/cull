use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CAPTURE_DIR_SETTING: &str = "clipboard_monitor_capture_dir";
pub const LAST_COLLECTION_SETTING: &str = "clipboard_monitor_last_collection_id";
pub const CAPTURE_EXISTING_ON_START_SETTING: &str = "clipboard_monitor_capture_existing_on_start";
pub const DEFAULT_POLL_MS: u64 = 750;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClipboardAccessStatus {
    Supported,
    UnsupportedPlatform,
    PermissionRequired,
    PermissionDenied,
    Error(String),
}

impl ClipboardAccessStatus {
    pub fn label(&self) -> String {
        match self {
            ClipboardAccessStatus::Supported => "supported".to_string(),
            ClipboardAccessStatus::UnsupportedPlatform => "unsupported_platform".to_string(),
            ClipboardAccessStatus::PermissionRequired => "permission_required".to_string(),
            ClipboardAccessStatus::PermissionDenied => "permission_denied".to_string(),
            ClipboardAccessStatus::Error(message) => format!("error:{message}"),
        }
    }
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

pub trait ClipboardImageReader: Send + 'static {
    fn status(&self) -> ClipboardAccessStatus;
    fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, String>;
}

#[derive(Debug, Default)]
pub struct ClipboardMonitorState {
    pub running: bool,
    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
    pub capture_dir: Option<PathBuf>,
    pub captured_count: u32,
    pub capture_existing_on_start: bool,
    pub baseline_complete: bool,
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
    pub source_url: Option<String>,
    pub source_app: Option<String>,
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

pub fn capture_existing_on_start_enabled(
    db: &crate::db_core::db::Database,
) -> Result<bool, String> {
    let Some(value) = db
        .get_setting(CAPTURE_EXISTING_ON_START_SETTING)
        .map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };
    Ok(matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    ))
}

pub fn set_capture_existing_on_start(
    db: &crate::db_core::db::Database,
    enabled: bool,
) -> Result<(), String> {
    db.set_setting(
        CAPTURE_EXISTING_ON_START_SETTING,
        if enabled { "true" } else { "false" },
    )
    .map_err(|e| e.to_string())
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
        source_url: capture.source_url.clone(),
        source_app: capture.source_app.clone(),
    })
}

pub fn process_reader_once<R: ClipboardImageReader>(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    session: &ClipboardMonitorSession,
    state: &mut ClipboardMonitorState,
    reader: &mut R,
) -> Result<Option<ClipboardCaptureResult>, String> {
    ensure_reader_access(reader.status())?;
    let capture = reader.read_if_changed()?;
    if !state.baseline_complete {
        state.baseline_complete = true;
        if !state.capture_existing_on_start {
            if let Some(capture) = capture {
                state.last_hash = Some(sha256_bytes(&capture.bytes));
                state.last_change_count = capture.change_count;
            }
            return Ok(None);
        }
    }

    let Some(capture) = capture else {
        return Ok(None);
    };
    let hash = sha256_bytes(&capture.bytes);
    if state.last_hash.as_deref() == Some(hash.as_str()) {
        return Ok(None);
    }

    let sequence = state.captured_count.saturating_add(1);
    let result = capture_clipboard_image(db, app_data_dir, session, &capture, sequence)?;
    if result.imported {
        state.captured_count = state.captured_count.saturating_add(1);
        state.last_hash = Some(hash);
        state.last_change_count = capture.change_count;
    }
    Ok(Some(result))
}

fn ensure_reader_access(status: ClipboardAccessStatus) -> Result<(), String> {
    match status {
        ClipboardAccessStatus::Supported => Ok(()),
        ClipboardAccessStatus::UnsupportedPlatform => {
            Err("Clipboard Monitor is not supported on this platform".to_string())
        }
        ClipboardAccessStatus::PermissionRequired => {
            Err("Clipboard access permission is required".to_string())
        }
        ClipboardAccessStatus::PermissionDenied => {
            Err("Clipboard access permission was denied".to_string())
        }
        ClipboardAccessStatus::Error(message) => Err(message),
    }
}

pub fn move_capture_folder(
    db: &crate::db_core::db::Database,
    old_dir: &str,
    new_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(new_dir)
        .map_err(|e| format!("Failed to create destination capture folder: {}", e))?;
    let files = db
        .list_image_files_under_path(old_dir)
        .map_err(|e| e.to_string())?;
    for (image_file_id, old_path) in files {
        let old = PathBuf::from(&old_path);
        if !old.exists() {
            continue;
        }
        let Some(file_name) = old.file_name() else {
            continue;
        };
        let new_path = new_dir.join(file_name);
        std::fs::copy(&old, &new_path)
            .map_err(|e| format!("Failed to copy clipboard capture: {}", e))?;
        let old_size = std::fs::metadata(&old).map_err(|e| e.to_string())?.len();
        let new_size = std::fs::metadata(&new_path)
            .map_err(|e| e.to_string())?
            .len();
        if old_size != new_size {
            return Err(format!(
                "Copied capture size mismatch for {}",
                old.display()
            ));
        }
        db.update_image_file_path(&image_file_id, &new_path.to_string_lossy())
            .map_err(|e| e.to_string())?;
    }
    db.set_setting(CAPTURE_DIR_SETTING, &new_dir.to_string_lossy())
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
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
    fn capture_existing_on_start_setting_defaults_to_false_and_round_trips() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();

        assert!(!capture_existing_on_start_enabled(&db).unwrap());

        set_capture_existing_on_start(&db, true).unwrap();
        assert!(capture_existing_on_start_enabled(&db).unwrap());

        set_capture_existing_on_start(&db, false).unwrap();
        assert!(!capture_existing_on_start_enabled(&db).unwrap());
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
        assert_eq!(
            db.list_collection_images(&session.collection_id)
                .unwrap()
                .len(),
            1
        );
    }

    struct FakeReader {
        captures: std::collections::VecDeque<ClipboardCapture>,
    }

    impl ClipboardImageReader for FakeReader {
        fn status(&self) -> ClipboardAccessStatus {
            ClipboardAccessStatus::Supported
        }

        fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, String> {
            Ok(self.captures.pop_front())
        }
    }

    struct OptionalFakeReader {
        captures: std::collections::VecDeque<Option<ClipboardCapture>>,
    }

    impl ClipboardImageReader for OptionalFakeReader {
        fn status(&self) -> ClipboardAccessStatus {
            ClipboardAccessStatus::Supported
        }

        fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, String> {
            Ok(self.captures.pop_front().flatten())
        }
    }

    #[test]
    fn process_reader_capture_skips_same_hash_twice() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let tmp = tempdir().unwrap();
        let app_data = tmp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let session = create_monitor_session(&db, &app_data, None).unwrap();
        let capture = ClipboardCapture {
            bytes: png_bytes([0, 0, 255, 255]),
            extension: "png".to_string(),
            original_filename: None,
            source_url: None,
            source_app: None,
            change_count: Some(2),
        };
        let mut state = ClipboardMonitorState {
            capture_existing_on_start: true,
            ..Default::default()
        };
        let mut reader = FakeReader {
            captures: vec![capture.clone(), capture].into(),
        };

        let first = process_reader_once(&db, &app_data, &session, &mut state, &mut reader).unwrap();
        let second =
            process_reader_once(&db, &app_data, &session, &mut state, &mut reader).unwrap();

        assert!(first.is_some());
        assert!(second.is_none());
        assert_eq!(
            db.list_collection_images(&session.collection_id)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn process_reader_captures_next_image_after_empty_default_baseline() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let tmp = tempdir().unwrap();
        let app_data = tmp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let session = create_monitor_session(&db, &app_data, None).unwrap();
        let capture = ClipboardCapture {
            bytes: png_bytes([12, 34, 56, 255]),
            extension: "png".to_string(),
            original_filename: None,
            source_url: None,
            source_app: None,
            change_count: Some(13),
        };
        let mut state = ClipboardMonitorState::default();
        let mut reader = OptionalFakeReader {
            captures: vec![None, Some(capture)].into(),
        };

        let first = process_reader_once(&db, &app_data, &session, &mut state, &mut reader).unwrap();
        let second =
            process_reader_once(&db, &app_data, &session, &mut state, &mut reader).unwrap();

        assert!(first.is_none());
        assert!(second.is_some());
        assert_eq!(state.captured_count, 1);
        assert_eq!(
            db.list_collection_images(&session.collection_id)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn process_reader_skips_current_clipboard_image_by_default() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let tmp = tempdir().unwrap();
        let app_data = tmp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let session = create_monitor_session(&db, &app_data, None).unwrap();
        let capture = ClipboardCapture {
            bytes: png_bytes([220, 20, 60, 255]),
            extension: "png".to_string(),
            original_filename: None,
            source_url: None,
            source_app: None,
            change_count: Some(12),
        };
        let mut state = ClipboardMonitorState::default();
        let mut reader = FakeReader {
            captures: vec![capture].into(),
        };

        let result =
            process_reader_once(&db, &app_data, &session, &mut state, &mut reader).unwrap();

        assert!(result.is_none());
        assert_eq!(state.captured_count, 0);
        assert_eq!(
            db.list_collection_images(&session.collection_id)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn move_capture_folder_copies_files_and_updates_paths() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let tmp = tempdir().unwrap();
        let app_data = tmp.path().join("app-data");
        let new_dir = tmp.path().join("moved-captures");
        std::fs::create_dir_all(&app_data).unwrap();
        let session = create_monitor_session(&db, &app_data, None).unwrap();
        let capture = ClipboardCapture {
            bytes: png_bytes([20, 30, 40, 255]),
            extension: "png".to_string(),
            original_filename: Some("Move Me.png".to_string()),
            source_url: None,
            source_app: None,
            change_count: Some(1),
        };
        let result = capture_clipboard_image(&db, &app_data, &session, &capture, 1).unwrap();

        move_capture_folder(&db, &session.capture_dir, &new_dir).unwrap();

        let moved = new_dir.join(std::path::Path::new(&result.path).file_name().unwrap());
        assert!(moved.exists());
        let image = db
            .get_images_by_ids(&[result.image_id.unwrap().as_str()])
            .unwrap()
            .remove(0);
        assert_eq!(image.path, moved.to_string_lossy());
    }
}
