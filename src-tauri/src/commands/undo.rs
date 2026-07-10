use crate::services::undo::UndoStatus;
use crate::services::undo_history::{enrich_undo_history, UndoHistoryEntry};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn undo(state: State<'_, AppState>) -> Result<Option<String>, String> {
    state.action_manager.undo(&state.db)
}

#[tauri::command]
pub async fn redo(state: State<'_, AppState>) -> Result<Option<String>, String> {
    state.action_manager.redo(&state.db)
}

#[tauri::command]
pub async fn get_undo_status(state: State<'_, AppState>) -> Result<UndoStatus, String> {
    Ok(state.action_manager.status(&state.db))
}

#[tauri::command]
pub async fn list_undo_history(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<UndoHistoryEntry>, String> {
    enrich_undo_history(&state.db, &state.app_data_dir, limit.unwrap_or(20))
}
