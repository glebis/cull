use tauri::State;
use crate::AppState;
use crate::db_core::lineage::LineageGroup;
use crate::db_core::models::ImageWithFile;

#[tauri::command]
pub async fn list_lineage_groups(
    state: State<'_, AppState>,
) -> Result<Vec<LineageGroup>, String> {
    state.db.list_lineage_groups().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_lineage_group_images(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<Vec<ImageWithFile>, String> {
    state.db.get_lineage_group_images(&group_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_lineage_group_manual(
    state: State<'_, AppState>,
    name: String,
    image_ids: Vec<String>,
) -> Result<String, String> {
    let group_id = state.db.create_lineage_group(&name, "manual", 100.0)
        .map_err(|e| e.to_string())?;
    for (i, id) in image_ids.iter().enumerate() {
        state.db.assign_to_lineage_group(id, &group_id, i as i32)
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
    state.db.rename_lineage_group(&group_id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn merge_lineage_groups(
    state: State<'_, AppState>,
    keep_id: String,
    merge_id: String,
) -> Result<(), String> {
    state.db.merge_lineage_groups(&keep_id, &merge_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dissolve_lineage_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<(), String> {
    state.db.dissolve_lineage_group(&group_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_lineage_group(
    state: State<'_, AppState>,
    group_id: String,
    image_id: String,
) -> Result<(), String> {
    let images = state.db.get_lineage_group_images(&group_id).map_err(|e| e.to_string())?;
    let order = images.len() as i32;
    state.db.assign_to_lineage_group(&image_id, &group_id, order).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_from_lineage_group(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<(), String> {
    state.db.remove_from_lineage_group(&image_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_batch_images(
    state: State<'_, AppState>,
    batch_id: String,
) -> Result<Vec<ImageWithFile>, String> {
    state.db.get_batch_images(&batch_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_lineage(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    use tauri::Emitter;
    let all_images = state.db.list_images(100000, 0).map_err(|e| e.to_string())?;
    let image_ids: Vec<String> = all_images.iter().map(|img| img.image.id.clone()).collect();
    let groups = state.db.detect_lineage_for_batch(&image_ids).map_err(|e| e.to_string())?;
    let count = groups.len() as u32;
    let _ = app.emit("lineage-scan-complete", serde_json::json!({ "groups": count }));
    Ok(count)
}
