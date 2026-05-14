use crate::db_core::lineage::LineageGroup;
use crate::db_core::models::{GenerationRun, ImageWithFile};
use crate::db_core::sidecar;
use crate::AppState;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn list_lineage_groups(state: State<'_, AppState>) -> Result<Vec<LineageGroup>, String> {
    state.db.list_lineage_groups().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_lineage_group_images(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<Vec<ImageWithFile>, String> {
    let mut images = state
        .db
        .get_lineage_group_images(&group_id)
        .map_err(|e| e.to_string())?;
    crate::services::library::enrich_thumbnails(&mut images, &state.app_data_dir);
    Ok(images)
}

#[tauri::command]
pub async fn create_lineage_group_manual(
    state: State<'_, AppState>,
    name: String,
    image_ids: Vec<String>,
) -> Result<String, String> {
    let group_id = state
        .db
        .create_lineage_group(&name, "manual", 100.0)
        .map_err(|e| e.to_string())?;
    for (i, id) in image_ids.iter().enumerate() {
        state
            .db
            .assign_to_lineage_group(id, &group_id, i as i32)
            .map_err(|e| e.to_string())?;
    }
    Ok(group_id)
}

#[tauri::command]
pub async fn rename_lineage_group(
    state: State<'_, AppState>,
    group_id: String,
    name: String,
) -> Result<(), String> {
    state
        .db
        .rename_lineage_group(&group_id, &name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn merge_lineage_groups(
    state: State<'_, AppState>,
    keep_id: String,
    merge_id: String,
) -> Result<(), String> {
    state
        .db
        .merge_lineage_groups(&keep_id, &merge_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dissolve_lineage_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<(), String> {
    state
        .db
        .dissolve_lineage_group(&group_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_lineage_group(
    state: State<'_, AppState>,
    group_id: String,
    image_id: String,
) -> Result<(), String> {
    let images = state
        .db
        .get_lineage_group_images(&group_id)
        .map_err(|e| e.to_string())?;
    let order = images.len() as i32;
    state
        .db
        .assign_to_lineage_group(&image_id, &group_id, order)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_from_lineage_group(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<(), String> {
    state
        .db
        .remove_from_lineage_group(&image_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_batch_images(
    state: State<'_, AppState>,
    batch_id: String,
) -> Result<Vec<ImageWithFile>, String> {
    state
        .db
        .get_batch_images(&batch_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_lineage(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    use tauri::Emitter;
    let image_ids = state.db.list_image_ids().map_err(|e| e.to_string())?;
    let groups = state
        .db
        .detect_lineage_for_batch(&image_ids)
        .map_err(|e| e.to_string())?;
    let count = groups.len() as u32;
    let _ = app.emit(
        "lineage-scan-complete",
        serde_json::json!({ "groups": count }),
    );
    Ok(count)
}

#[tauri::command]
pub async fn get_generation_run(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Option<GenerationRun>, String> {
    state
        .db
        .get_generation_run_for_image(&image_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(dead_code)]
pub async fn rescan_sidecars(state: State<'_, AppState>) -> Result<u32, String> {
    let images = state
        .db
        .get_images_without_generation_run()
        .map_err(|e| e.to_string())?;
    let mut linked = 0u32;
    for (image_id, file_path) in &images {
        let path = Path::new(file_path);
        if let Some(sidecar_path) = sidecar::find_sidecar(path) {
            if let Ok(sc) = sidecar::parse_sidecar(&sidecar_path) {
                let run_id = Uuid::new_v4().to_string();
                let run = GenerationRun {
                    id: run_id.clone(),
                    prompt: sc.prompt,
                    negative_prompt: sc.negative_prompt,
                    provider: sc.provider,
                    model: sc.model,
                    settings_json: sc.settings_json,
                    seed: sc.seed,
                    parent_run_id: None,
                    source_type: "sidecar".to_string(),
                    source_path: Some(sidecar_path.to_string_lossy().to_string()),
                    raw_metadata_json: Some(sc.raw_json),
                    created_at: sc.created_at,
                    imported_at: chrono::Utc::now().to_rfc3339(),
                };
                if state.db.insert_generation_run(&run).is_ok() {
                    if state.db.link_image_to_run(image_id, &run_id).is_ok() {
                        linked += 1;
                    }
                }
            }
        }
    }
    Ok(linked)
}
