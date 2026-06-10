use crate::db_core::db::Database;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ImportFolderParams {
    pub folder_path: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ImportFilesParams {
    pub file_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub batch_id: Option<String>,
    pub image_ids: Vec<String>,
}

fn supported_entries(db: &Database, folder_path: &str) -> Vec<PathBuf> {
    let module_raw = crate::db_core::import::is_module_raw_enabled(db);
    let extensions = crate::extensions::supported_extensions(module_raw);

    walkdir::WalkDir::new(folder_path)
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
        .collect()
}

pub fn import_folder(
    db: &Database,
    app_data_dir: &Path,
    params: ImportFolderParams,
) -> Result<ImportResult, String> {
    let entries = supported_entries(db, &params.folder_path);
    let mut result = import_paths(db, app_data_dir, "folder", entries)?;
    let _ = db.add_library_root(&params.folder_path);
    if result.image_ids.is_empty() {
        return Ok(result);
    }

    let batch = db
        .create_import_batch("folder", result.image_ids.len() as u32, None)
        .map_err(|e| e.to_string())?;
    for id in &result.image_ids {
        let _ = db.set_image_batch(id, &batch);
    }
    let _ = db.detect_lineage_for_batch(&result.image_ids);
    result.batch_id = Some(batch);
    Ok(result)
}

pub fn import_files(
    db: &Database,
    app_data_dir: &Path,
    params: ImportFilesParams,
) -> Result<ImportResult, String> {
    let entries = params.file_paths.into_iter().map(PathBuf::from).collect();
    let mut result = import_paths(db, app_data_dir, "files", entries)?;
    if result.image_ids.is_empty() {
        return Ok(result);
    }

    let batch = db
        .create_import_batch("files", result.image_ids.len() as u32, None)
        .map_err(|e| e.to_string())?;
    for id in &result.image_ids {
        let _ = db.set_image_batch(id, &batch);
    }
    let _ = db.detect_lineage_for_batch(&result.image_ids);
    result.batch_id = Some(batch);
    Ok(result)
}

fn import_paths(
    db: &Database,
    app_data_dir: &Path,
    _source: &str,
    entries: Vec<PathBuf>,
) -> Result<ImportResult, String> {
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();
    let mut image_ids = Vec::new();

    for path in entries {
        match crate::db_core::import::import_file(db, &path, app_data_dir) {
            Ok(Some(id)) => {
                image_ids.push(id);
                imported += 1;
            }
            Ok(None) => skipped += 1,
            Err(e) => errors.push(format!("{}: {}", path.display(), e)),
        }
    }

    Ok(ImportResult {
        imported,
        skipped,
        errors,
        batch_id: None,
        image_ids,
    })
}
