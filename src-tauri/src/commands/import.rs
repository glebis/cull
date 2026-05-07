use std::path::Path;
use tauri::{AppHandle, Emitter, State};
use crate::AppState;

#[derive(serde::Serialize)]
pub struct ImportResponse {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
}

#[derive(Clone, serde::Serialize)]
struct ImportProgress {
    current: u32,
    total: u32,
    filename: String,
}

#[tauri::command]
pub async fn import_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<ImportResponse, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;

    // Collect all image files first so we know the total
    let extensions = ["jpg", "jpeg", "png", "webp", "gif"];
    let entries: Vec<std::path::PathBuf> = walkdir::WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total = entries.len() as u32;
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    for (i, path) in entries.iter().enumerate() {
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        // Emit progress event
        let _ = app.emit(
            "import-progress",
            ImportProgress {
                current: (i + 1) as u32,
                total,
                filename,
            },
        );

        match crate::db_core::import::import_file(db, path, app_data_dir) {
            Ok(Some(_)) => imported += 1,
            Ok(None) => skipped += 1,
            Err(e) => errors.push(format!("{}: {}", path.display(), e)),
        }
    }

    Ok(ImportResponse {
        imported,
        skipped,
        errors,
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
        match crate::db_core::import::import_file(db, Path::new(&path_str), app_data_dir) {
            Ok(Some(_)) => imported += 1,
            Ok(None) => skipped += 1,
            Err(e) => errors.push(format!("{}: {}", path_str, e)),
        }
    }

    Ok(ImportResponse { imported, skipped, errors })
}
