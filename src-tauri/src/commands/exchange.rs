use crate::exchange::{export_bundle, import_bundle, preview_import, ExchangeExportOptions};
use crate::AppState;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub async fn export_cull_exchange(
    state: State<'_, AppState>,
    options: ExchangeExportOptions,
) -> Result<String, String> {
    export_bundle(&state.db, options)
}

#[tauri::command]
pub async fn preview_cull_exchange_import(
    bundle_dir: String,
) -> Result<crate::exchange::ExchangeImportPreview, String> {
    Ok(preview_import(&PathBuf::from(bundle_dir)))
}

#[tauri::command]
pub async fn import_cull_exchange(
    state: State<'_, AppState>,
    bundle_dir: String,
) -> Result<crate::exchange::bundle::ExchangeImportResult, String> {
    import_bundle(&state.db, &PathBuf::from(bundle_dir))
}
