use tauri::State;
use crate::AppState;
use crate::db_core::smart_collections::SmartCollection;
use crate::db_core::models::ImageWithFile;
use crate::db_core::nl_parser::parse_query;

#[tauri::command]
pub async fn create_smart_collection(
    state: State<'_, AppState>,
    name: String,
    filter_json: String,
    nl_query: Option<String>,
) -> Result<String, String> {
    state.db.create_smart_collection(&name, &filter_json, nl_query.as_deref(), false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_smart_collections(
    state: State<'_, AppState>,
) -> Result<Vec<SmartCollection>, String> {
    state.db.list_smart_collections()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn evaluate_smart_collection(
    state: State<'_, AppState>,
    filter_json: String,
) -> Result<Vec<ImageWithFile>, String> {
    state.db.evaluate_smart_collection(&filter_json)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_smart_collection(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state.db.delete_smart_collection(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_smart_collection(
    state: State<'_, AppState>,
    id: String,
    name: String,
    filter_json: String,
    nl_query: Option<String>,
) -> Result<(), String> {
    state.db.update_smart_collection(&id, &name, &filter_json, nl_query.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn parse_nl_query(query: String) -> Result<String, String> {
    let filter = parse_query(&query);
    serde_json::to_string(&filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backfill_image_metadata(
    state: State<'_, AppState>,
) -> Result<u32, String> {
    state.db.backfill_image_metadata()
        .map_err(|e| e.to_string())
}
