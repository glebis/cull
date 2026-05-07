use tauri::State;
use crate::AppState;
use crate::db_core::models::ImageWithFile;
use crate::db_core::thumbnails;

#[tauri::command]
pub async fn list_images(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ImageWithFile>, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let mut images = db.list_images(limit, offset).map_err(|e| e.to_string())?;

    for img in &mut images {
        let thumb = thumbnails::thumbnail_path(app_data_dir, &img.image.id);
        if thumb.exists() {
            img.thumbnail_path = Some(thumb.to_string_lossy().to_string());
        }
    }

    Ok(images)
}

#[tauri::command]
pub async fn get_image_count(state: State<'_, AppState>) -> Result<u32, String> {
    state.db.image_count().map_err(|e| e.to_string())
}
