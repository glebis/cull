use crate::db_core::models::ImageWithFile;
use crate::db_core::nl_parser::parse_query;
use crate::db_core::smart_collections::SmartCollection;
use crate::services::curation as svc;
use crate::services::ServiceContext;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn create_smart_collection(
    state: State<'_, AppState>,
    name: String,
    filter_json: String,
    nl_query: Option<String>,
) -> Result<String, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::create_smart_collection(&ctx, &name, &filter_json, nl_query.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_smart_collections(
    state: State<'_, AppState>,
) -> Result<Vec<SmartCollection>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_smart_collections(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn evaluate_smart_collection(
    state: State<'_, AppState>,
    filter_json: String,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::evaluate_smart_collection(&ctx, &filter_json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_smart_collection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_smart_collection(&ctx, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_smart_collection(
    state: State<'_, AppState>,
    id: String,
    name: String,
    filter_json: String,
    nl_query: Option<String>,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::update_smart_collection(&ctx, &id, &name, &filter_json, nl_query.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn parse_nl_query(query: String) -> Result<String, String> {
    let filter = parse_query(&query);
    serde_json::to_string(&filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backfill_image_metadata(state: State<'_, AppState>) -> Result<u32, String> {
    state
        .db
        .backfill_image_metadata()
        .map_err(|e| e.to_string())
}
