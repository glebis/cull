use tauri::State;
use crate::AppState;

#[tauri::command]
pub async fn set_rating(
    state: State<'_, AppState>,
    image_id: String,
    rating: u8,
) -> Result<(), String> {
    state.db.set_rating(&image_id, rating).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_decision(
    state: State<'_, AppState>,
    image_id: String,
    decision: String,
) -> Result<(), String> {
    state.db.set_decision(&image_id, &decision).map_err(|e| e.to_string())
}
