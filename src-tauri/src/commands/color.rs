use crate::db_core::models::{ImageColorMetrics, ImageWithFile};
use crate::services::{Pagination, ServiceContext};
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn analyze_image_colors(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let total = image_ids.len() as u32;
    let mut analyzed = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let ctx = ServiceContext::from_app_state(&state, None);
        match crate::services::ai::analyze_image_color_metrics(&ctx, image_id) {
            Ok(_) => analyzed += 1,
            Err(e) => crate::safe_eprintln!("Color analysis error for {}: {}", image_id, e),
        }

        let _ = app.emit(
            "color-progress",
            serde_json::json!({
                "current": i + 1,
                "total": total,
                "analyzer": crate::db_core::color::COLOR_ANALYZER_VERSION,
            }),
        );
    }

    Ok(analyzed)
}

#[tauri::command]
pub async fn get_image_color_metrics(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Option<ImageColorMetrics>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_image_color_metrics(&ctx, &image_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_color_metrics_count(state: State<'_, AppState>) -> Result<u32, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_color_metrics_count(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images_by_color_bucket(
    state: State<'_, AppState>,
    bucket: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    crate::services::ai::list_images_by_color_bucket(
        &ctx,
        &bucket,
        Pagination::clamped(offset.unwrap_or(0), limit.unwrap_or(100)),
    )
    .map_err(|e| e.to_string())
}
