use crate::db_core::models::{ImageTag, TagBackfillResult, TagSummary};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn backfill_image_tags(state: State<'_, AppState>) -> Result<TagBackfillResult, String> {
    state.db.backfill_image_tags().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_image_tags(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Vec<ImageTag>, String> {
    state
        .db
        .list_image_tags(&image_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_tags(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<TagSummary>, String> {
    state
        .db
        .list_tags(limit.unwrap_or(100), offset.unwrap_or(0))
        .map_err(|e| e.to_string())
}
