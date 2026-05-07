use std::path::Path;
use tauri::State;
use crate::AppState;

#[derive(serde::Serialize)]
pub struct ImportResponse {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
}

#[tauri::command]
pub async fn import_folder(
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<ImportResponse, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let result = crate::core::import::import_folder(db, Path::new(&folder_path), app_data_dir);
    Ok(ImportResponse {
        imported: result.imported,
        skipped: result.skipped,
        errors: result.errors,
    })
}

#[tauri::command]
pub async fn import_files(
    state: State<'_, AppState>,
    file_paths: Vec<String>,
) -> Result<ImportResponse, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    for path_str in file_paths {
        match crate::core::import::import_file(db, Path::new(&path_str), app_data_dir) {
            Ok(Some(_)) => imported += 1,
            Ok(None) => skipped += 1,
            Err(e) => errors.push(format!("{}: {}", path_str, e)),
        }
    }

    Ok(ImportResponse { imported, skipped, errors })
}
