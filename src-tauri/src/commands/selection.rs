use tauri::State;
use crate::AppState;
use crate::services::ServiceContext;
use crate::services::curation as svc;

#[tauri::command]
pub async fn set_rating(
    state: State<'_, AppState>,
    image_id: String,
    rating: u8,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::set_rating(&ctx, &image_id, rating).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_decision(
    state: State<'_, AppState>,
    image_id: String,
    decision: String,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::set_decision(&ctx, &image_id, &decision).map_err(|e| e.to_string())
}
