use tauri::State;
use crate::AppState;
use crate::db_core::models::ImageWithFile;
use crate::services::{Pagination, ServiceContext};
use crate::services::library as svc;

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
    svc::list_images(&ctx, Pagination::clamped(offset, limit))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<u32, String> {
    state.db.delete_images_by_folder(&folder).map_err(|e| e.to_string())
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
    svc::list_images_filtered(&ctx, min_width, min_height, Pagination::clamped(offset, limit))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_image_count(state: State<'_, AppState>) -> Result<u32, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::get_image_count(&ctx).map_err(|e| e.to_string())
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
        let found = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        if let Some(img) = found.first() {
            #[cfg(target_os = "macos")]
            {
                let status = std::process::Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application \"Finder\" to delete POSIX file \"{}\"",
                        img.path.replace('"', "\\\"")
                    )])
                    .output();
                if let Ok(output) = status {
                    if output.status.success() {
                        trashed += 1;
                        let filename = std::path::Path::new(&img.path)
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
        let found = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        if let Some(img) = found.first() {
            let path = std::path::Path::new(&img.path);
            if path.exists() && std::fs::remove_file(path).is_ok() {
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
    state.db.set_setting(&key, &value).map_err(|e| e.to_string())
}
