use crate::AppState;
use image::GenericImageView;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub async fn crop_image(
    state: State<'_, AppState>,
    image_id: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    save_as_copy: bool,
) -> Result<String, String> {
    let images = state.db.get_images_by_ids(&[&image_id]).map_err(|e| e.to_string())?;
    let img_record = images.first().ok_or("Image not found")?;
    let path = PathBuf::from(&img_record.path);

    if width == 0 || height == 0 {
        return Err("Crop dimensions must be non-zero".to_string());
    }

    let img = image::open(&path).map_err(|e| format!("Failed to open image: {e}"))?;
    let (img_w, img_h) = img.dimensions();

    if x.checked_add(width).map_or(true, |r| r > img_w)
        || y.checked_add(height).map_or(true, |r| r > img_h)
    {
        return Err(format!(
            "Crop region ({x},{y},{width},{height}) exceeds image dimensions ({img_w}x{img_h})"
        ));
    }

    let cropped = img.crop_imm(x, y, width, height);

    let output_path = if save_as_copy {
        let stem = path.file_stem().ok_or("Invalid file path: no stem")?.to_string_lossy();
        let ext = path.extension().unwrap_or_default().to_string_lossy();
        let parent = path.parent().ok_or("Invalid file path: no parent")?;
        let new_name = format!("{stem}_crop.{ext}");
        parent.join(new_name)
    } else {
        path.clone()
    };

    cropped
        .save(&output_path)
        .map_err(|e| format!("Failed to save: {e}"))?;

    if !save_as_copy {
        state
            .db
            .update_image_dimensions(&image_id, width, height)
            .map_err(|e| e.to_string())?;
        Ok(image_id)
    } else {
        Ok(output_path.to_string_lossy().to_string())
    }
}

#[tauri::command]
pub async fn rotate_image(
    state: State<'_, AppState>,
    image_id: String,
    degrees: i32,
) -> Result<(), String> {
    let images = state.db.get_images_by_ids(&[&image_id]).map_err(|e| e.to_string())?;
    let img_record = images.first().ok_or("Image not found")?;
    let path = PathBuf::from(&img_record.path);

    let img = image::open(&path).map_err(|e| format!("Failed to open image: {e}"))?;

    let rotated = match degrees.rem_euclid(360) {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => {
            return Err(format!(
                "Only 90/180/270 degree rotations supported, got {degrees}"
            ))
        }
    };

    let (new_w, new_h) = rotated.dimensions();
    rotated
        .save(&path)
        .map_err(|e| format!("Failed to save: {e}"))?;

    state
        .db
        .update_image_dimensions(&image_id, new_w, new_h)
        .map_err(|e| e.to_string())?;

    Ok(())
}
