use crate::AppState;
use image::GenericImageView;
use std::path::{Path, PathBuf};
use tauri::State;

fn derivative_path(source: &Path, suffix: &str) -> Result<PathBuf, String> {
    let parent = source.parent().ok_or("Invalid file path: no parent")?;
    let stem = source
        .file_stem()
        .ok_or("Invalid file path: no stem")?
        .to_string_lossy();
    let ext = source.extension().map(|ext| ext.to_string_lossy());

    for index in 0..10_000 {
        let suffix = if index == 0 {
            suffix.to_string()
        } else {
            format!("{suffix}_{}", index + 1)
        };
        let file_name = match &ext {
            Some(ext) if !ext.is_empty() => format!("{stem}_{suffix}.{ext}"),
            _ => format!("{stem}_{suffix}"),
        };
        let candidate = parent.join(file_name);
        if !candidate
            .try_exists()
            .map_err(|e| format!("Failed to check output path: {e}"))?
        {
            return Ok(candidate);
        }
    }

    Err("Could not find an available derivative file name".to_string())
}

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
    let _ = save_as_copy;
    let images = state
        .db
        .get_images_by_ids(&[&image_id])
        .map_err(|e| e.to_string())?;
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
    let output_path = derivative_path(&path, "crop")?;

    cropped
        .save(&output_path)
        .map_err(|e| format!("Failed to save: {e}"))?;

    Ok(output_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn rotate_image(
    state: State<'_, AppState>,
    image_id: String,
    degrees: i32,
) -> Result<String, String> {
    let images = state
        .db
        .get_images_by_ids(&[&image_id])
        .map_err(|e| e.to_string())?;
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

    let output_path = derivative_path(&path, &format!("rot{}", degrees.rem_euclid(360)))?;
    rotated
        .save(&output_path)
        .map_err(|e| format!("Failed to save: {e}"))?;

    Ok(output_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::models::{Image, ImageFile};
    use crate::db_core::secrets::MemoryStore;
    use crate::{services, watcher};
    use image::{ImageBuffer, Rgba};
    use std::fs;
    use std::path::{Path, PathBuf};

    const IMAGE_ID: &str = "image-1";

    fn write_test_image(path: &Path, width: u32, height: u32) {
        let image = ImageBuffer::from_fn(width, height, |x, y| {
            Rgba([
                (x * 17 % 255) as u8,
                (y * 31 % 255) as u8,
                ((x + y) * 13 % 255) as u8,
                255,
            ])
        });
        image::DynamicImage::ImageRgba8(image).save(path).unwrap();
    }

    fn test_state(tmp: &Path, image_path: &Path, width: u32, height: u32) -> AppState {
        let db = Database::open(&tmp.join("test.db")).unwrap();
        let now = "2026-05-31T00:00:00Z".to_string();
        let file_size = fs::metadata(image_path).unwrap().len();
        db.insert_image(&Image {
            id: IMAGE_ID.to_string(),
            sha256_hash: "hash-1".to_string(),
            width,
            height,
            format: "png".to_string(),
            file_size,
            created_at: now.clone(),
            imported_at: now.clone(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: "file-1".to_string(),
            image_id: IMAGE_ID.to_string(),
            path: image_path.to_string_lossy().to_string(),
            last_seen_at: now,
            missing_at: None,
            last_seen_size: Some(file_size),
            last_seen_mtime: None,
        })
        .unwrap();

        let app_data_dir = tmp.join("app-data");
        let model_dir = tmp.join("models");
        fs::create_dir_all(&app_data_dir).unwrap();

        AppState {
            db,
            app_data_dir,
            embedding_engine: parking_lot::Mutex::new(EmbeddingEngine::new(&model_dir)),
            detection_engine: parking_lot::Mutex::new(DetectionEngine::new_yolo(&model_dir)),
            safety_engine: parking_lot::Mutex::new(DetectionEngine::new_nudenet(&model_dir)),
            secrets: Box::new(MemoryStore::new()),
            jobs: services::jobs::JobRegistry::default(),
            action_manager: services::undo::ActionManager::new(),
            file_watcher: parking_lot::Mutex::new(watcher::FileWatcher::new()),
            clipboard_monitor: parking_lot::Mutex::new(
                services::clipboard_monitor::ClipboardMonitorState::default(),
            ),
        }
    }

    fn command_state(state: &AppState) -> State<'_, AppState> {
        // Tauri does not expose a public State constructor; this keeps tests on
        // the command entrypoint without enabling extra crate features.
        unsafe { std::mem::transmute::<&AppState, State<'_, AppState>>(state) }
    }

    #[tokio::test]
    async fn crop_default_path_leaves_original_file_bytes_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("source.png");
        write_test_image(&image_path, 8, 6);
        let original_bytes = fs::read(&image_path).unwrap();

        let state = test_state(tmp.path(), &image_path, 8, 6);

        let result = crop_image(
            command_state(&state),
            IMAGE_ID.to_string(),
            1,
            1,
            4,
            3,
            false,
        )
        .await
        .unwrap();

        assert_ne!(result, IMAGE_ID);
        assert_eq!(fs::read(&image_path).unwrap(), original_bytes);

        let output_path = PathBuf::from(result);
        assert_ne!(output_path, image_path);
        assert_eq!(image::open(&output_path).unwrap().dimensions(), (4, 3));

        let images = state.db.get_images_by_ids(&[IMAGE_ID]).unwrap();
        assert_eq!(images[0].image.width, 8);
        assert_eq!(images[0].image.height, 6);
    }

    #[tokio::test]
    async fn rotate_default_path_leaves_original_file_bytes_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("source.png");
        write_test_image(&image_path, 8, 6);
        let original_bytes = fs::read(&image_path).unwrap();

        let state = test_state(tmp.path(), &image_path, 8, 6);

        let result = rotate_image(command_state(&state), IMAGE_ID.to_string(), 90)
            .await
            .unwrap();

        assert_eq!(fs::read(&image_path).unwrap(), original_bytes);

        let output_path = PathBuf::from(result);
        assert_ne!(output_path, image_path);
        assert_eq!(image::open(&output_path).unwrap().dimensions(), (6, 8));

        let images = state.db.get_images_by_ids(&[IMAGE_ID]).unwrap();
        assert_eq!(images[0].image.width, 8);
        assert_eq!(images[0].image.height, 6);
    }
}
