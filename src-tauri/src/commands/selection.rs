use tauri::State;
use crate::AppState;
use crate::services::undo::Action;

#[tauri::command]
pub async fn set_rating(
    state: State<'_, AppState>,
    image_id: String,
    rating: u8,
) -> Result<(), String> {
    state.action_manager.execute(&state.db, Action::SetRating { image_id, rating })?;
    Ok(())
}

#[tauri::command]
pub async fn set_decision(
    state: State<'_, AppState>,
    image_id: String,
    decision: String,
) -> Result<(), String> {
    state.action_manager.execute(&state.db, Action::SetDecision { image_id, decision })?;
    Ok(())
}
