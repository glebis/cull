use crate::db_core::models::ImageWithFile;
use crate::services::library as svc;
use crate::services::{Pagination, ServiceContext};
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Result<Vec<(String, u32)>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_folders(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images_by_folder(
    state: State<'_, AppState>,
    folder: String,
    limit: u32,
    offset: u32,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_images_by_folder(&ctx, &folder, Pagination::clamped(offset, limit))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_images(&ctx, Pagination::clamped(offset, limit)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, folder: String) -> Result<u32, String> {
    state
        .db
        .delete_images_by_folder(&folder)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images_filtered(
    state: State<'_, AppState>,
    min_width: Option<u32>,
    min_height: Option<u32>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_images_filtered(
        &ctx,
        min_width,
        min_height,
        Pagination::clamped(offset, limit),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_image_count(state: State<'_, AppState>) -> Result<u32, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::get_image_count(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_image_ids(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_image_ids(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_images_by_ids(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let id_refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    svc::get_images_by_ids(&ctx, &id_refs).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_image_by_path(
    state: State<'_, AppState>,
    path: String,
) -> Result<Option<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::get_image_by_path(&ctx, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_iteration_siblings(
    state: State<'_, AppState>,
    parent_id: String,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::get_iteration_siblings(&ctx, &parent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trash_images(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let mut trashed = 0u32;
    for image_id in &image_ids {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let found = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;
        if let Some(img) = found.first() {
            let path = std::path::Path::new(&img.path);
            if path.exists() {
                match trash::delete(path) {
                    Ok(()) => {
                        trashed += 1;
                        let _ = state.db.mark_file_missing(&img.path);
                        let filename = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("file")
                            .to_string();
                        let _ = state.action_manager.record_action(
                            &state.db,
                            "trash_image",
                            format!("Trash {}", filename),
                            serde_json::json!({"image_id": image_id, "path": &img.path}).to_string(),
                            serde_json::json!({"image_id": image_id, "path": &img.path, "trashed": true}).to_string(),
                            image_id.clone(),
                            true,
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to trash {}: {}", img.path, e);
                    }
                }
            }
        }
    }
    Ok(trashed)
}

#[tauri::command]
pub async fn delete_images_permanently(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let mut deleted = 0u32;
    for image_id in &image_ids {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let found = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;
        if let Some(img) = found.first() {
            let path = std::path::Path::new(&img.path);
            if path.exists() && std::fs::remove_file(path).is_ok() {
                let _ = state.db.mark_file_missing(&img.path);
                deleted += 1;
            }
        }
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn get_app_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    state.db.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_app_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    state
        .db
        .set_setting(&key, &value)
        .map_err(|e| e.to_string())?;
    if key == "module_raw" {
        let enabled = value == "true";
        state
            .file_watcher
            .lock()
            .module_raw
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

#[derive(Clone, serde::Serialize)]
pub struct LibraryHealthResult {
    pub purged: u32,
    pub missing_sources: u32,
    pub to_regenerate: Vec<String>,
}

#[tauri::command]
pub async fn check_library_health(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LibraryHealthResult, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;

    let auto_purge = db
        .get_setting("auto_purge_missing")
        .unwrap_or(None)
        .unwrap_or_else(|| "false".to_string());
    let auto_purge = auto_purge == "true";

    let image_ids = db.list_image_ids().map_err(|e| e.to_string())?;
    let total = image_ids.len() as u32;
    let mut purged = 0u32;
    let mut missing_sources = 0u32;
    let mut to_regenerate = Vec::new();
    let mut processed = 0u32;

    // Phase 1: soft-mark missing files
    for chunk in image_ids.chunks(250) {
        let id_refs: Vec<&str> = chunk.iter().map(|id| id.as_str()).collect();
        let images = db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        for img in images {
            let source_path = std::path::Path::new(&img.path);
            if !source_path.exists() {
                let _ = db.mark_file_missing(&img.path);
                missing_sources += 1;
            } else {
                let thumb = crate::db_core::thumbnails::thumbnail_path(app_data_dir, &img.image.id);
                if !thumb.exists() {
                    to_regenerate.push(img.image.id.clone());
                }
            }

            processed += 1;
            if processed % 100 == 0 || processed == total {
                let _ = app.emit(
                    "health-check-progress",
                    serde_json::json!({
                        "current": processed, "total": total
                    }),
                );
            }
        }
    }

    // Phase 2: purge records that have been missing longer than the threshold
    if auto_purge {
        let purge_days: i64 = db
            .get_setting("purge_after_days")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let threshold = format!("-{} days", purge_days);
        let ids_to_purge: Vec<String> = {
            let conn = db.conn.lock();
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT i.id FROM images i
                 WHERE NOT EXISTS (
                     SELECT 1 FROM image_files f
                     WHERE f.image_id = i.id AND f.missing_at IS NULL
                 )
                 AND NOT EXISTS (
                     SELECT 1 FROM image_files f
                     WHERE f.image_id = i.id
                     AND f.missing_at > datetime('now', ?1)
                 )",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(rusqlite::params![threshold], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let mut result = Vec::new();
            for row in rows {
                if let Ok(id) = row {
                    result.push(id);
                }
            }
            result
        };

        for id in &ids_to_purge {
            let conn = db.conn.lock();
            let _ = conn.execute("DELETE FROM images WHERE id = ?1", rusqlite::params![id]);
            drop(conn);
            let thumb = crate::db_core::thumbnails::thumbnail_path(app_data_dir, id);
            if thumb.exists() {
                let _ = std::fs::remove_file(&thumb);
            }
            for &size in &crate::db_core::thumbnails::THUMBNAIL_SIZES {
                let sized =
                    crate::db_core::thumbnails::sized_thumbnail_path(app_data_dir, id, size);
                if sized.exists() {
                    let _ = std::fs::remove_file(&sized);
                }
            }
            purged += 1;
        }
    }

    Ok(LibraryHealthResult {
        purged,
        missing_sources,
        to_regenerate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the trash crate can handle filenames with special characters
    /// that would have caused AppleScript injection with the old implementation.
    #[test]
    fn trash_special_char_filename() {
        let dir = tempfile::tempdir().unwrap();
        let evil_name = "test \"file' with $(special) chars.png";
        let file_path = dir.path().join(evil_name);
        std::fs::write(&file_path, b"fake image data").unwrap();
        assert!(file_path.exists());

        trash::delete(&file_path).expect("trash::delete should handle special characters");
        assert!(
            !file_path.exists(),
            "file should no longer exist at original path"
        );
    }
}
