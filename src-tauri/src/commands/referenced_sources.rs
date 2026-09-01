use crate::db_core::models::ReferencedSource;
use crate::mounted_sources::{refresh_mounted_sources, PlatformMountedSourceProvider};
use crate::services::referenced_sources::{
    self as service, OpenReferencedFolder, ReferencedFolderPage, ReferencedFolderUpdate,
};
use crate::AppState;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter, State};

static SOURCE_JOBS: LazyLock<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[tauri::command]
pub async fn list_referenced_sources(
    state: State<'_, AppState>,
) -> Result<Vec<ReferencedSource>, String> {
    let _ = refresh_mounted_sources(&state.db, &PlatformMountedSourceProvider);
    state
        .db
        .list_referenced_sources()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn remember_referenced_folder(
    state: State<'_, AppState>,
    path: String,
) -> Result<ReferencedSource, String> {
    let canonical = std::fs::canonicalize(&path).map_err(|error| error.to_string())?;
    if !canonical.is_dir() {
        return Err("Dropped path is not a folder".to_string());
    }
    let canonical_string = canonical.to_string_lossy().to_string();
    if let Some(mut existing) = state
        .db
        .list_referenced_sources()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|source| {
            source.source_kind == crate::db_core::models::ReferencedSourceKind::Folder
                && source.last_mount_path.as_deref() == Some(canonical_string.as_str())
        })
    {
        existing.offline_at = None;
        existing.last_seen_at = chrono::Utc::now().to_rfc3339();
        state
            .db
            .upsert_referenced_source(&existing)
            .map_err(|error| error.to_string())?;
        return Ok(existing);
    }
    let source = ReferencedSource {
        id: uuid::Uuid::new_v4().to_string(),
        platform_volume_id: None,
        display_name: canonical
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Folder".to_string()),
        last_mount_path: Some(canonical_string),
        source_kind: crate::db_core::models::ReferencedSourceKind::Folder,
        capacity_bytes: None,
        recursive_default: false,
        settings_json: "{}".to_string(),
        last_seen_at: chrono::Utc::now().to_rfc3339(),
        offline_at: None,
    };
    state
        .db
        .upsert_referenced_source(&source)
        .map_err(|error| error.to_string())?;
    Ok(source)
}

#[tauri::command]
pub async fn list_source_folders(
    state: State<'_, AppState>,
    source_id: String,
    relative_path: String,
) -> Result<Vec<String>, String> {
    service::list_source_folders(&state.db, &source_id, &relative_path)
}

#[tauri::command]
pub async fn list_images_in_referenced_folder(
    state: State<'_, AppState>,
    source_id: String,
    relative_path: String,
    recursive: bool,
    limit: u32,
    offset: u32,
    include_rejected: Option<bool>,
) -> Result<Vec<crate::db_core::models::ImageWithFile>, String> {
    let mut images = state
        .db
        .list_images_in_referenced_folder(
            &source_id,
            &relative_path,
            recursive,
            limit.clamp(1, 250),
            offset,
            include_rejected.unwrap_or(false),
        )
        .map_err(|error| error.to_string())?;
    crate::services::library::enrich_thumbnails(&mut images, &state.app_data_dir);
    Ok(images)
}

#[tauri::command]
pub async fn open_referenced_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    request: OpenReferencedFolder,
) -> Result<ReferencedFolderPage, String> {
    let (page, paths) = service::discover_folder_page(&state.db, &request)?;
    let job_id = page.job_id.clone();
    let source_id = page.source_id.clone();
    let relative_path = page.relative_path.clone();
    let cancelled = Arc::new(AtomicBool::new(false));
    SOURCE_JOBS.lock().insert(job_id.clone(), cancelled.clone());
    let db = state.db.clone();
    let app_data_dir = state.app_data_dir.clone();
    std::thread::Builder::new()
        .name("cull-referenced-source-page".to_string())
        .spawn(move || {
            let result = service::register_referenced_paths(
                &db,
                &app_data_dir,
                &source_id,
                &paths,
                &cancelled,
            );
            let was_cancelled = cancelled.load(Ordering::Relaxed);
            let (image_ids, error) = match result {
                Ok(image_ids) => (image_ids, None),
                Err(error) => (Vec::new(), Some(error)),
            };
            let _ = app.emit(
                "referenced-source:page-updated",
                ReferencedFolderUpdate {
                    job_id: job_id.clone(),
                    source_id,
                    relative_path,
                    image_ids,
                    completed: true,
                    cancelled: was_cancelled,
                    error,
                },
            );
            SOURCE_JOBS.lock().remove(&job_id);
        })
        .map_err(|error| error.to_string())?;
    Ok(page)
}

#[tauri::command]
pub async fn set_source_recursive_default(
    state: State<'_, AppState>,
    source_id: String,
    recursive: bool,
) -> Result<(), String> {
    let mut source = state
        .db
        .list_referenced_sources()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|source| source.id == source_id)
        .ok_or_else(|| "Referenced source not found".to_string())?;
    source.recursive_default = recursive;
    state
        .db
        .upsert_referenced_source(&source)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn remove_referenced_source(
    state: State<'_, AppState>,
    source_id: String,
) -> Result<Vec<String>, String> {
    let orphaned = state
        .db
        .remove_referenced_source(&source_id)
        .map_err(|error| error.to_string())?;
    for image_id in &orphaned {
        crate::db_core::thumbnails::remove_thumbnails_for_image(&state.app_data_dir, image_id);
    }
    Ok(orphaned)
}

#[tauri::command]
pub async fn cancel_referenced_source_job(job_id: String) -> bool {
    let Some(cancelled) = SOURCE_JOBS.lock().get(&job_id).cloned() else {
        return false;
    };
    cancelled.store(true, Ordering::Relaxed);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_an_unknown_job_is_a_safe_noop() {
        assert!(!tauri::async_runtime::block_on(
            cancel_referenced_source_job("missing".to_string())
        ));
    }
}
