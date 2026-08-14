use crate::apple_photos::{
    self, PhotosAlbum, PhotosAsset, PhotosAssetFilter, PhotosAssetSort, PhotosAuthorizationStatus,
    PhotosPage, SystemPhotosCatalog,
};
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

const DEFAULT_PAGE_LIMIT: u32 = 50;

fn update_apple_photos_job(
    jobs: &crate::services::jobs::JobRegistry,
    progress: &apple_photos::PhotosImportProgress,
) -> Option<crate::services::jobs::JobSnapshot> {
    // `current` identifies the item being worked on. Expose completed items to
    // the shared panel so it does not declare the final item complete early.
    let completed = progress.current.saturating_sub(1);
    let message = match progress.phase.as_str() {
        "discovery" => "Reading Apple Photos metadata".to_string(),
        "download" => progress
            .filename
            .as_deref()
            .map(|name| format!("Downloading {name}"))
            .unwrap_or_else(|| "Downloading from Apple Photos".into()),
        "import" => progress
            .filename
            .as_deref()
            .map(|name| format!("Importing {name}"))
            .unwrap_or_else(|| "Importing from Apple Photos".into()),
        _ => "Importing from Apple Photos".into(),
    };
    jobs.update_progress(&progress.job_id, completed, Some(&message));
    jobs.get(&progress.job_id)
}

async fn blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, crate::apple_photos::PhotosError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("Apple Photos worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn photos_authorization_status() -> Result<PhotosAuthorizationStatus, String> {
    blocking(|| apple_photos::authorization_status(&SystemPhotosCatalog)).await
}

#[tauri::command]
pub async fn photos_request_authorization() -> Result<PhotosAuthorizationStatus, String> {
    blocking(|| apple_photos::request_authorization(&SystemPhotosCatalog)).await
}

#[tauri::command]
pub async fn photos_list_albums(
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<PhotosPage<PhotosAlbum>, String> {
    blocking(move || {
        apple_photos::list_albums(
            &SystemPhotosCatalog,
            offset.unwrap_or(0),
            limit.unwrap_or(DEFAULT_PAGE_LIMIT),
        )
    })
    .await
}

#[tauri::command]
pub async fn photos_list_assets(
    album_id: Option<String>,
    offset: Option<u32>,
    limit: Option<u32>,
    filter: Option<PhotosAssetFilter>,
    sort: Option<PhotosAssetSort>,
) -> Result<PhotosPage<PhotosAsset>, String> {
    blocking(move || {
        apple_photos::list_assets(
            &SystemPhotosCatalog,
            album_id.as_deref(),
            offset.unwrap_or(0),
            limit.unwrap_or(DEFAULT_PAGE_LIMIT),
            filter.unwrap_or_default(),
            sort.unwrap_or_default(),
        )
    })
    .await
}

#[tauri::command]
pub async fn photos_load_local_preview(
    asset_id: String,
    size: Option<u32>,
) -> Result<Option<String>, String> {
    blocking(move || {
        apple_photos::load_local_preview(&SystemPhotosCatalog, &asset_id, size.unwrap_or(320))
    })
    .await
}

#[tauri::command]
pub async fn photos_start_import_assets(
    app: AppHandle,
    state: State<'_, AppState>,
    asset_ids: Vec<String>,
    representation: String,
    source_album_id: Option<String>,
    progress_id: Option<String>,
) -> Result<apple_photos::PhotosImportStarted, String> {
    if representation != "current" {
        return Err("This release supports only the current Apple Photos representation".into());
    }
    let mut seen = std::collections::HashSet::new();
    let asset_ids: Vec<String> = asset_ids
        .into_iter()
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect();
    if asset_ids.is_empty() {
        return Err("Select at least one Apple Photos asset".into());
    }
    if asset_ids.len() > 250 {
        return Err("Apple Photos imports are limited to 250 selected assets per batch".into());
    }

    let db = state.db.clone();
    let jobs = state.jobs.clone();
    let app_data_dir = state.app_data_dir.clone();
    let (started, cancel) = apple_photos::create_current_import(&db, &jobs, asset_ids.len() as u32)
        .map_err(|error| error.to_string())?;
    if let Some(snapshot) = jobs.get(&started.job_id) {
        let _ = app.emit("job-status-changed", snapshot);
    }
    let worker_started = started.clone();
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = apple_photos::run_current_import_job(
            &SystemPhotosCatalog,
            &db,
            &app_data_dir,
            &jobs,
            worker_started.clone(),
            cancel,
            asset_ids,
            source_album_id,
            |progress| {
                if let Some(snapshot) = update_apple_photos_job(&jobs, progress) {
                    let _ = db.save_job(&snapshot);
                    let _ = worker_app.emit("job-status-changed", snapshot);
                }
                let _ = worker_app.emit(
                    "photos-import-progress",
                    serde_json::json!({
                        "job_id": progress.job_id,
                        "progress_id": progress_id,
                        "phase": progress.phase,
                        "current": progress.current,
                        "total": progress.total,
                        "filename": progress.filename,
                        "bytes_current": progress.bytes_current,
                        "bytes_total": progress.bytes_total,
                        "fraction": progress.fraction,
                    }),
                );
            },
        );
        match result {
            Ok(summary) => {
                let _ = worker_app.emit("photos-import-finished", &summary);
                let _ = crate::tray::refresh_tray_menu(&worker_app);
            }
            Err(error) => {
                jobs.finish_from_worker(
                    &worker_started.job_id,
                    crate::services::jobs::WorkerTerminalOutcome::Failed(error.to_string()),
                );
                if let Some(snapshot) = jobs.get(&worker_started.job_id) {
                    let _ = db.save_job(&snapshot);
                }
                let _ = worker_app.emit(
                    "photos-import-finished",
                    serde_json::json!({
                        "job_id": worker_started.job_id,
                        "batch_id": worker_started.batch_id,
                        "error": error.to_string(),
                    }),
                );
            }
        }
        if let Some(snapshot) = jobs.get(&worker_started.job_id) {
            let _ = worker_app.emit("job-status-changed", snapshot);
        }
    });
    Ok(started)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_operation_uses_standard_command_error_string() {
        let error = crate::apple_photos::PhotosError::Unsupported.to_string();
        assert_eq!(error, "Apple Photos is unsupported on this platform");
    }

    #[test]
    fn shared_job_snapshot_reports_progress_and_remains_cancellable() {
        let jobs = crate::services::jobs::JobRegistry::default();
        let (job_id, cancel) = jobs.create_job("import", 2);
        let snapshot = update_apple_photos_job(
            &jobs,
            &apple_photos::PhotosImportProgress {
                job_id: job_id.clone(),
                phase: "download".into(),
                current: 1,
                total: 2,
                filename: Some("photo.jpg".into()),
                bytes_current: None,
                bytes_total: None,
                fraction: Some(0.5),
            },
        )
        .unwrap();

        assert_eq!(snapshot.kind, "import");
        assert_eq!(snapshot.status, "running");
        assert_eq!(snapshot.current, 0);
        assert_eq!(snapshot.total, 2);
        assert_eq!(snapshot.message.as_deref(), Some("Downloading photo.jpg"));
        jobs.cancel(&job_id).unwrap();
        assert!(cancel.is_cancelled());
    }
}
