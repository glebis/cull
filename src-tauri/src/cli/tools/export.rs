use serde_json::Value;

use super::HeadlessContext;

pub fn list_export_presets() -> Result<Value, String> {
    serde_json::to_value(crate::services::export::list_presets()).map_err(|e| e.to_string())
}

pub fn export_images(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: crate::services::export::ExportImagesParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid export_images params: {}", e))?;
    let result = crate::services::export::export_images(&ctx.db, &ctx.app_data_dir, parsed)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}
