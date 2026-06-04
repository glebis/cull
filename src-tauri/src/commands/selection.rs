use crate::db_core::models::ClientFeedback;
use crate::services::undo::Action;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn set_rating(
    state: State<'_, AppState>,
    image_id: String,
    rating: u8,
) -> Result<(), String> {
    state
        .action_manager
        .execute(&state.db, Action::SetRating { image_id, rating })?;
    Ok(())
}

#[tauri::command]
pub async fn set_decision(
    state: State<'_, AppState>,
    image_id: String,
    decision: String,
) -> Result<(), String> {
    state
        .action_manager
        .execute(&state.db, Action::SetDecision { image_id, decision })?;
    Ok(())
}

/// Client feedback (favorite + comment) is stored separately from curator
/// selections so the two never overwrite each other.
#[tauri::command]
pub async fn set_client_feedback(
    state: State<'_, AppState>,
    image_id: String,
    favorite: bool,
    comment: Option<String>,
) -> Result<(), String> {
    state
        .db
        .set_client_feedback(&image_id, favorite, comment.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_client_feedback(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Option<ClientFeedback>, String> {
    state
        .db
        .get_client_feedback(&image_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_client_feedback(
    state: State<'_, AppState>,
) -> Result<Vec<ClientFeedback>, String> {
    state.db.list_client_feedback().map_err(|e| e.to_string())
}
