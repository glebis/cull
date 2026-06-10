// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::services::asset_events::{
    get_asset_load_events as get_asset_load_events_from_db, log_asset_load_event, AssetLoadEvent,
    NewAssetLoadEvent,
};
use crate::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetLoadEventRequest {
    pub view: String,
    pub image_id: Option<String>,
    pub asset_kind: String,
    pub image_format: Option<String>,
    pub fallback_used: bool,
    pub fallback_succeeded: Option<bool>,
    pub path_basename: Option<String>,
    pub path_hash: Option<String>,
    pub error_kind: String,
    pub details_json: Option<String>,
}

impl From<AssetLoadEventRequest> for NewAssetLoadEvent {
    fn from(value: AssetLoadEventRequest) -> Self {
        Self {
            view: value.view,
            image_id: value.image_id,
            asset_kind: value.asset_kind,
            image_format: value.image_format,
            fallback_used: value.fallback_used,
            fallback_succeeded: value.fallback_succeeded,
            path_basename: value.path_basename,
            path_hash: value.path_hash,
            error_kind: value.error_kind,
            details_json: value.details_json,
        }
    }
}

#[tauri::command]
pub async fn record_asset_load_event(
    state: State<'_, AppState>,
    event: AssetLoadEventRequest,
) -> Result<AssetLoadEvent, String> {
    log_asset_load_event(&state.db, event.into()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_asset_load_events(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<AssetLoadEvent>, String> {
    get_asset_load_events_from_db(&state.db, limit).map_err(|e| e.to_string())
}
