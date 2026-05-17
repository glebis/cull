use crate::db_core::models::{ImagePerceptualHash, NearDuplicateImage};
use crate::services::ServiceContext;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn analyze_perceptual_hashes(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let total = image_ids.len() as u32;
    let mut analyzed = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let ctx = ServiceContext::from_app_state(&state, None);
        match crate::services::ai::analyze_image_perceptual_hash(&ctx, image_id) {
            Ok(_) => analyzed += 1,
            Err(e) => eprintln!("pHash error for {}: {}", image_id, e),
        }

        let _ = app.emit(
            "perceptual-hash-progress",
            serde_json::json!({
                "current": i + 1,
                "total": total,
                "algorithm": crate::db_core::perceptual_hash::PHASH_ALGORITHM,
            }),
        );
    }

    Ok(analyzed)
}

#[tauri::command]
pub async fn get_image_perceptual_hash(
    state: State<'_, AppState>,
    image_id: String,
    algorithm: Option<String>,
) -> Result<Option<ImagePerceptualHash>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_image_perceptual_hash(&ctx, &image_id, algorithm.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_perceptual_hash_count(
    state: State<'_, AppState>,
    algorithm: Option<String>,
) -> Result<u32, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_perceptual_hash_count(&ctx, algorithm.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_near_duplicates_by_phash(
    state: State<'_, AppState>,
    image_id: String,
    max_distance: Option<u32>,
    limit: Option<u32>,
    algorithm: Option<String>,
) -> Result<Vec<NearDuplicateImage>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    crate::services::ai::find_near_duplicates_by_phash(
        &ctx,
        &image_id,
        max_distance.unwrap_or(8),
        limit.unwrap_or(50),
        algorithm.as_deref(),
    )
    .map_err(|e| e.to_string())
}
