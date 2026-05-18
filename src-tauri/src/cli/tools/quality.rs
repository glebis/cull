use serde::Deserialize;
use serde_json::Value;

use super::HeadlessContext;

#[derive(Debug, Deserialize)]
struct AnalyzeQualityParams {
    image_ids: Option<Vec<String>>,
    all: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ImageQualityParams {
    image_id: String,
}

pub fn analyze_image_quality(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: AnalyzeQualityParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid analyze_image_quality params: {}", e))?;
    let image_ids = image_ids_for_request(ctx, parsed)?;
    let total = image_ids.len() as u32;
    let mut analyzed = 0u32;
    let mut failed = 0u32;
    let mut errors = Vec::new();

    for image_id in &image_ids {
        match analyze_one(ctx, image_id) {
            Ok(()) => analyzed += 1,
            Err(e) => {
                failed += 1;
                if errors.len() < 20 {
                    errors.push(serde_json::json!({ "image_id": image_id, "error": e }));
                }
            }
        }
    }

    Ok(serde_json::json!({
        "status": "completed",
        "analyzer": crate::db_core::quality::QUALITY_ANALYZER_VERSION,
        "total": total,
        "analyzed": analyzed,
        "failed": failed,
        "errors": errors,
    }))
}

pub fn get_image_quality(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: ImageQualityParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid get_image_quality params: {}", e))?;
    let metrics = ctx
        .db
        .get_image_quality_metrics(&parsed.image_id)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(metrics).map_err(|e| e.to_string())
}

pub fn get_quality_count(ctx: &HeadlessContext) -> Result<Value, String> {
    let count = ctx.db.quality_metrics_count().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "count": count }))
}

fn image_ids_for_request(
    ctx: &HeadlessContext,
    params: AnalyzeQualityParams,
) -> Result<Vec<String>, String> {
    if params.all.unwrap_or(false) {
        return ctx.db.list_image_ids().map_err(|e| e.to_string());
    }

    let image_ids = params.image_ids.unwrap_or_default();
    if image_ids.is_empty() {
        return Err("analyze_image_quality requires image_ids or all=true".to_string());
    }
    Ok(image_ids)
}

fn analyze_one(ctx: &HeadlessContext, image_id: &str) -> Result<(), String> {
    let images = ctx
        .db
        .get_images_by_ids(&[image_id])
        .map_err(|e| e.to_string())?;
    let image = images
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_id))?;
    let ml_path = crate::commands::resolve_image_path_for_ml(image, &ctx.app_data_dir);
    let metrics = crate::db_core::quality::analyze_image_quality(image_id, &ml_path)?;
    ctx.db
        .store_image_quality_metrics(&metrics)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;

    #[test]
    fn analyze_all_empty_library_returns_zero_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = HeadlessContext {
            db: Database::open(std::path::Path::new(":memory:")).unwrap(),
            app_data_dir: tmp.path().to_path_buf(),
        };

        let result = analyze_image_quality(&ctx, serde_json::json!({ "all": true })).unwrap();

        assert_eq!(result["status"], "completed");
        assert_eq!(result["total"], 0);
        assert_eq!(result["analyzed"], 0);
        assert_eq!(result["failed"], 0);
    }
}
