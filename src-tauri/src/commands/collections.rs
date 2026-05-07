use tauri::State;
use crate::AppState;
use crate::db_core::models::ImageWithFile;
use crate::db_core::thumbnails;

#[tauri::command]
pub async fn create_collection(state: State<'_, AppState>, name: String) -> Result<String, String> {
    state.db.create_collection(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> Result<Vec<(String, String, u32)>, String> {
    state.db.list_collections().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_collection(state: State<'_, AppState>, collection_id: String, image_ids: Vec<String>) -> Result<(), String> {
    let refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    state.db.add_to_collection(&collection_id, &refs).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_collection_images(state: State<'_, AppState>, collection_id: String) -> Result<Vec<ImageWithFile>, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let mut images = db.list_collection_images(&collection_id).map_err(|e| e.to_string())?;
    for img in &mut images {
        let thumb = thumbnails::thumbnail_path(app_data_dir, &img.image.id);
        if thumb.exists() {
            img.thumbnail_path = Some(thumb.to_string_lossy().to_string());
        }
    }
    Ok(images)
}

#[tauri::command]
pub async fn delete_collection(state: State<'_, AppState>, collection_id: String) -> Result<(), String> {
    state.db.delete_collection(&collection_id).map_err(|e| e.to_string())
}
