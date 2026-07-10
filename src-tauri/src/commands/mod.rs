pub mod agent_proposals;
pub mod agent_snapshots;
pub mod catalog;
pub mod clipboard_monitor;
pub mod collections;
pub mod color;
pub mod deeplink;
pub mod detection;
pub mod diagnostics;
pub mod dictation;
pub mod embeddings;
pub mod exchange;
pub mod export;
pub mod files;
pub mod generation;
pub mod import;
pub mod jobs;
pub mod library;
pub mod lineage;
pub mod mcp;
pub mod media;
pub mod ocr;
pub mod perceptual_hash;
pub mod plugins;
pub mod preview;
pub mod privacy;
pub mod quality;
pub mod raw;
pub mod selection;
pub mod sessions;
pub mod smart_collections;
pub mod static_publishing;
pub mod tags;
pub mod transform;
pub mod undo;
pub mod vision;
pub mod window;

use crate::db_core::models::NewSessionEvent;
use crate::AppState;

pub(crate) fn log_library_event(
    state: &AppState,
    event_type: &str,
    subject_type: Option<&str>,
    subject_id: Option<String>,
    payload: serde_json::Value,
) {
    let _ = state.db.log_session_event(&NewSessionEvent {
        session_id: None,
        event_type: event_type.to_string(),
        actor_type: "user".to_string(),
        actor_id: None,
        subject_type: subject_type.map(str::to_string),
        subject_id,
        payload_json: payload.to_string(),
    });
}

pub fn resolve_image_path_for_ml(
    img: &crate::db_core::models::ImageWithFile,
    app_data_dir: &std::path::Path,
) -> std::path::PathBuf {
    let ext = std::path::Path::new(&img.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if crate::extensions::should_use_thumbnail_for_ml(ext) {
        let thumbnail = crate::db_core::thumbnails::thumbnail_path(app_data_dir, &img.image.id);
        if thumbnail.exists() {
            thumbnail
        } else {
            std::path::PathBuf::from(&img.path)
        }
    } else {
        std::path::PathBuf::from(&img.path)
    }
}
