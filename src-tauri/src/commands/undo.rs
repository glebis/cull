use crate::services::undo::{UndoManyResult, UndoStatus};
use crate::services::undo_history::{enrich_undo_history, UndoHistoryEntry};
use crate::{db_core::models::UndoRecord, AppState};
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn undo(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let record = state.action_manager.undo_record(&state.db)?;
    emit_selection_run_updated(&app, record.as_ref());
    Ok(record.map(|record| record.label))
}

#[tauri::command]
pub async fn redo(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let record = state.action_manager.redo_record(&state.db)?;
    emit_selection_run_updated(&app, record.as_ref());
    Ok(record.map(|record| record.label))
}

#[tauri::command]
pub async fn undo_many(state: State<'_, AppState>, count: u32) -> Result<UndoManyResult, String> {
    state.action_manager.undo_many(&state.db, count)
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
    let mut entries = enrich_undo_history(&state.db, &state.app_data_dir, limit.unwrap_or(20))?;
    let undoable = state.action_manager.undoable_count(&state.db)? as usize;
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.can_undo = index < undoable;
    }
    Ok(entries)
}

/// Shortlist undo/redo changes selection-run membership outside the mode's own
/// commands, so surfaces are notified with the same `selection-run:updated`
/// event they already observe.
fn emit_selection_run_updated(app: &AppHandle, record: Option<&UndoRecord>) {
    let Some(record) = record else {
        return;
    };
    if !record.action_type.starts_with("shortlist_") {
        return;
    }
    let selection_id = serde_json::from_str::<serde_json::Value>(&record.before_json)
        .ok()
        .and_then(|value| {
            value
                .get("selection_id")
                .and_then(|id| id.as_str().map(str::to_string))
        });
    if let Some(selection_id) = selection_id {
        let _ = app.emit(
            "selection-run:updated",
            serde_json::json!({ "selection_id": selection_id }),
        );
    }
}
