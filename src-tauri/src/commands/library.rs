use crate::commands::log_library_event;
use crate::db_core::models::{ImageWithFile, TrashImageResult, TrashImagesDetailedResult};
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
    delete_folder_inner(&state, &folder)
}

fn delete_folder_inner(state: &AppState, folder: &str) -> Result<u32, String> {
    let deleted_ids = state
        .db
        .delete_images_by_folder(folder)
        .map_err(|e| e.to_string())?;
    let deleted = deleted_ids.len() as u32;

    // Thumbnail cleanup is best-effort and must never fail the command:
    // the DB transaction already committed by this point.
    for image_id in &deleted_ids {
        crate::db_core::thumbnails::remove_thumbnails_for_image(&state.app_data_dir, image_id);
    }

    if deleted > 0 {
        log_library_event(
            state,
            "folder_removed_from_library",
            Some("folder"),
            Some(folder.to_string()),
            serde_json::json!({
                "folder": folder,
                "image_count": deleted,
            }),
        );
    }
    Ok(deleted)
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
    trash_images_inner(&state, &image_ids)
}

fn trash_images_inner(state: &AppState, image_ids: &[String]) -> Result<u32, String> {
    let mut trashed = 0u32;
    for image_id in image_ids {
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
                        log_library_event(
                            state,
                            "image_moved_to_trash",
                            Some("image"),
                            Some(image_id.clone()),
                            serde_json::json!({
                                "image_id": image_id,
                                "path": &img.path,
                                "filename": filename.clone(),
                            }),
                        );
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
pub async fn trash_images_detailed(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<TrashImagesDetailedResult, String> {
    trash_images_detailed_inner(&state, &image_ids)
}

pub(crate) fn trash_images_detailed_inner(
    state: &AppState,
    image_ids: &[String],
) -> Result<TrashImagesDetailedResult, String> {
    let mut results = Vec::new();

    for image_id in image_ids {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let found = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;

        let Some(img) = found.first() else {
            results.push(TrashImageResult {
                image_id: image_id.clone(),
                path: None,
                status: "not_found".to_string(),
                error: Some("Image was not found in the library".to_string()),
            });
            continue;
        };

        let path = std::path::Path::new(&img.path);
        if !path.exists() {
            results.push(TrashImageResult {
                image_id: image_id.clone(),
                path: Some(img.path.clone()),
                status: "missing".to_string(),
                error: Some("File is already missing on disk".to_string()),
            });
            continue;
        }

        match trash::delete(path) {
            Ok(()) => {
                let _ = state.db.mark_file_missing(&img.path);
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                log_library_event(
                    state,
                    "image_moved_to_trash",
                    Some("image"),
                    Some(image_id.clone()),
                    serde_json::json!({
                        "image_id": image_id,
                        "path": &img.path,
                        "filename": filename.clone(),
                    }),
                );
                let _ = state.action_manager.record_action(
                    &state.db,
                    "trash_image",
                    format!("Trash {}", filename),
                    serde_json::json!({"image_id": image_id, "path": &img.path}).to_string(),
                    serde_json::json!({"image_id": image_id, "path": &img.path, "trashed": true})
                        .to_string(),
                    image_id.clone(),
                    true,
                );
                results.push(TrashImageResult {
                    image_id: image_id.clone(),
                    path: Some(img.path.clone()),
                    status: "trashed".to_string(),
                    error: None,
                });
            }
            Err(e) => {
                results.push(TrashImageResult {
                    image_id: image_id.clone(),
                    path: Some(img.path.clone()),
                    status: "failed".to_string(),
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let succeeded = results.iter().filter(|r| r.status == "trashed").count() as u32;
    let failed = results.len() as u32 - succeeded;
    Ok(TrashImagesDetailedResult {
        requested: image_ids.len() as u32,
        succeeded,
        failed,
        results,
    })
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
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                log_library_event(
                    &state,
                    "image_deleted_permanently",
                    Some("image"),
                    Some(image_id.clone()),
                    serde_json::json!({
                        "image_id": image_id,
                        "path": &img.path,
                        "filename": filename,
                    }),
                );
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
            crate::db_core::thumbnails::remove_thumbnails_for_image(app_data_dir, id);
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
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::models::{Image, ImageFile};
    use crate::db_core::secrets::MemoryStore;
    use crate::{services, watcher};
    use std::path::Path;

    fn test_state(tmp: &Path) -> AppState {
        let db = Database::open(&tmp.join("test.db")).unwrap();
        let app_data_dir = tmp.join("app-data");
        let model_dir = tmp.join("models");
        std::fs::create_dir_all(&app_data_dir).unwrap();

        AppState {
            db,
            app_data_dir,
            embedding_engine: parking_lot::Mutex::new(EmbeddingEngine::new(&model_dir)),
            detection_engine: parking_lot::Mutex::new(DetectionEngine::new_yolo(&model_dir)),
            safety_engine: parking_lot::Mutex::new(DetectionEngine::new_nudenet(&model_dir)),
            secrets: Box::new(MemoryStore::new()),
            jobs: services::jobs::JobRegistry::default(),
            action_manager: services::undo::ActionManager::new(),
            file_watcher: parking_lot::Mutex::new(watcher::FileWatcher::new()),
            clipboard_monitor: parking_lot::Mutex::new(
                services::clipboard_monitor::ClipboardMonitorState::default(),
            ),
            static_publish_server: parking_lot::Mutex::new(
                crate::commands::static_publishing::StaticPublishServerState::default(),
            ),
            preview_state: crate::preview::state::PreviewStateStore::default(),
            preview_web_stream: crate::preview::web_stream::PreviewWebStreamController::default(),
            agent_snapshots: parking_lot::Mutex::new(
                services::agent_snapshots::AgentSnapshotRegistry::default(),
            ),
            agent_snapshot_requests: parking_lot::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn insert_test_image(db: &Database, image_id: &str, file_path: &Path) {
        let now = "2026-07-06T00:00:00Z".to_string();
        let file_size = std::fs::metadata(file_path).unwrap().len();
        db.insert_image(&Image {
            id: image_id.to_string(),
            sha256_hash: format!("hash-{image_id}"),
            width: 1,
            height: 1,
            format: "png".to_string(),
            file_size,
            created_at: now.clone(),
            imported_at: now.clone(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: format!("file-{image_id}"),
            image_id: image_id.to_string(),
            path: file_path.to_string_lossy().to_string(),
            last_seen_at: now,
            missing_at: None,
            last_seen_size: Some(file_size),
            last_seen_mtime: None,
        })
        .unwrap();
    }

    /// Creates fake thumbnail files (base + every sized variant) for an image
    /// id, so tests can assert they get cleaned up.
    fn write_fake_thumbnails(app_data_dir: &Path, image_id: &str) {
        let base = crate::db_core::thumbnails::thumbnail_path(app_data_dir, image_id);
        std::fs::write(&base, b"fake-thumb").unwrap();
        for &size in &crate::db_core::thumbnails::THUMBNAIL_SIZES {
            let sized =
                crate::db_core::thumbnails::sized_thumbnail_path(app_data_dir, image_id, size);
            std::fs::write(&sized, b"fake-thumb").unwrap();
        }
    }

    #[test]
    fn delete_folder_removes_thumbnails_of_exactly_the_deleted_images() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("delete-me");
        std::fs::create_dir_all(&folder).unwrap();
        let deleted_path = folder.join("deleted.png");
        std::fs::write(&deleted_path, b"fake image data").unwrap();

        let kept_folder = dir.path().join("keep-me");
        std::fs::create_dir_all(&kept_folder).unwrap();
        let kept_path = kept_folder.join("kept.png");
        std::fs::write(&kept_path, b"fake image data").unwrap();

        let state = test_state(dir.path());
        insert_test_image(&state.db, "img-deleted", &deleted_path);
        insert_test_image(&state.db, "img-kept", &kept_path);

        write_fake_thumbnails(&state.app_data_dir, "img-deleted");
        write_fake_thumbnails(&state.app_data_dir, "img-kept");

        let count = delete_folder_inner(&state, &folder.to_string_lossy()).unwrap();

        assert_eq!(count, 1);

        let deleted_base =
            crate::db_core::thumbnails::thumbnail_path(&state.app_data_dir, "img-deleted");
        assert!(
            !deleted_base.exists(),
            "base thumbnail of the deleted image should be removed"
        );
        for &size in &crate::db_core::thumbnails::THUMBNAIL_SIZES {
            let sized = crate::db_core::thumbnails::sized_thumbnail_path(
                &state.app_data_dir,
                "img-deleted",
                size,
            );
            assert!(
                !sized.exists(),
                "sized thumbnail {} of the deleted image should be removed",
                size
            );
        }

        let kept_base = crate::db_core::thumbnails::thumbnail_path(&state.app_data_dir, "img-kept");
        assert!(
            kept_base.exists(),
            "thumbnail of an image outside the deleted folder must survive"
        );
        for &size in &crate::db_core::thumbnails::THUMBNAIL_SIZES {
            let sized = crate::db_core::thumbnails::sized_thumbnail_path(
                &state.app_data_dir,
                "img-kept",
                size,
            );
            assert!(
                sized.exists(),
                "sized thumbnail {} of an image outside the deleted folder must survive",
                size
            );
        }
    }

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

    #[test]
    fn trash_images_moves_file_marks_missing_and_records_audit_and_undo() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("trash-command-success.png");
        std::fs::write(&file_path, b"fake image data").unwrap();
        let state = test_state(dir.path());
        insert_test_image(&state.db, "img-trash-success", &file_path);

        let count = trash_images_inner(&state, &["img-trash-success".to_string()]).unwrap();

        assert_eq!(count, 1);
        assert!(
            !file_path.exists(),
            "file should be moved out of source path"
        );
        let image_file = state
            .db
            .get_image_file_by_path(&file_path.to_string_lossy())
            .unwrap()
            .unwrap();
        assert!(image_file.missing_at.is_some());

        let events = state.db.list_session_events(None, 10).unwrap();
        assert!(events.iter().any(|event| {
            event.event_type == "image_moved_to_trash"
                && event.subject_id.as_deref() == Some("img-trash-success")
        }));

        let undo_records = state.db.list_undo_records(10).unwrap();
        assert_eq!(undo_records.len(), 1);
        assert_eq!(undo_records[0].action_type, "trash_image");
        assert!(undo_records[0].has_file_backup);
    }

    #[test]
    fn trash_images_detailed_reports_partial_failures_without_stopping_batch() {
        let dir = tempfile::tempdir().unwrap();
        let existing_path = dir.path().join("trash-detailed-success.png");
        let missing_path = dir.path().join("trash-detailed-missing.png");
        std::fs::write(&existing_path, b"fake image data").unwrap();
        std::fs::write(&missing_path, b"fake image data").unwrap();
        let state = test_state(dir.path());
        insert_test_image(&state.db, "img-trash-ok", &existing_path);
        insert_test_image(&state.db, "img-trash-missing", &missing_path);
        trash::delete(&missing_path).unwrap();

        let result = trash_images_detailed_inner(
            &state,
            &[
                "img-trash-ok".to_string(),
                "img-trash-missing".to_string(),
                "img-trash-unknown".to_string(),
            ],
        )
        .unwrap();

        assert_eq!(result.requested, 3);
        assert_eq!(result.succeeded, 1);
        assert_eq!(result.failed, 2);
        assert_eq!(result.results[0].status, "trashed");
        assert_eq!(result.results[1].status, "missing");
        assert_eq!(result.results[2].status, "not_found");
        assert!(!existing_path.exists());

        let image_file = state
            .db
            .get_image_file_by_path(&existing_path.to_string_lossy())
            .unwrap()
            .unwrap();
        assert!(image_file.missing_at.is_some());
    }
}
