use tauri::State;
use crate::AppState;
use crate::db_core::models::ImageWithFile;
use crate::services::ServiceContext;
use crate::services::curation as svc;

#[tauri::command]
pub async fn create_collection(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::create_collection(&ctx, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> Result<Vec<(String, String, u32)>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_collections(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_collection(state: State<'_, AppState>, collection_id: String, image_ids: Vec<String>) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    svc::add_to_collection(&ctx, &collection_id, &refs).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_collection_images(state: State<'_, AppState>, collection_id: String) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_collection_images(&ctx, &collection_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_from_collection(state: State<'_, AppState>, collection_id: String, image_ids: Vec<String>) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    svc::remove_from_collection(&ctx, &collection_id, &refs).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_collection(state: State<'_, AppState>, collection_id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_collection(&ctx, &collection_id).map_err(|e| e.to_string())
}
