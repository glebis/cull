use crate::db_core::models::ImageQualityMetrics;
use crate::services::ServiceContext;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn analyze_image_quality(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let total = image_ids.len() as u32;
    let mut analyzed = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let ctx = ServiceContext::from_app_state(&state, None);
        match crate::services::ai::analyze_image_quality(&ctx, image_id) {
            Ok(_) => analyzed += 1,
            Err(e) => crate::safe_eprintln!("Quality analysis error for {}: {}", image_id, e),
        }

        let _ = app.emit(
            "quality-progress",
            serde_json::json!({
                "current": i + 1,
                "total": total,
                "analyzer": crate::db_core::quality::QUALITY_ANALYZER_VERSION,
            }),
        );
    }

    Ok(analyzed)
}

#[tauri::command]
pub async fn get_image_quality(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Option<ImageQualityMetrics>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_image_quality(&ctx, &image_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_quality_count(state: State<'_, AppState>) -> Result<u32, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_quality_count(&ctx).map_err(|e| e.to_string())
}
