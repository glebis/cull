use serde_json::Value;

use super::HeadlessContext;

pub fn import_folder(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: crate::services::import::ImportFolderParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid import_folder params: {}", e))?;
    let result = crate::services::import::import_folder(&ctx.db, &ctx.app_data_dir, parsed)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

pub fn import_files(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: crate::services::import::ImportFilesParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid import_files params: {}", e))?;
    let result = crate::services::import::import_files(&ctx.db, &ctx.app_data_dir, parsed)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}
