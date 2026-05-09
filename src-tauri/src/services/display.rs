use crate::services::ServiceError;
use tauri::Emitter;

pub fn show_image(app_handle: &tauri::AppHandle, image_id: &str) -> Result<(), ServiceError> {
    let params = serde_json::json!({
        "path": null,
        "paths": null,
        "folder": null,
        "view": "loupe",
        "focus": image_id,
    });
    app_handle.emit("open-with-params", params)
        .map_err(|e| ServiceError::Engine(e.to_string()))
}

pub fn navigate_to_folder(app_handle: &tauri::AppHandle, folder_path: &str) -> Result<(), ServiceError> {
    let params = serde_json::json!({
        "folder": folder_path,
        "view": "grid",
    });
    app_handle.emit("open-with-params", params)
        .map_err(|e| ServiceError::Engine(e.to_string()))
}

pub fn show_collection(app_handle: &tauri::AppHandle, collection_id: &str) -> Result<(), ServiceError> {
    app_handle.emit("navigate-collection", serde_json::json!({
        "collection_id": collection_id,
    }))
    .map_err(|e| ServiceError::Engine(e.to_string()))
}
