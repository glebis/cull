use crate::AppState;
use image::{DynamicImage, GenericImageView, ImageFormat};
use std::io::{BufWriter, Seek, Write};
use std::path::{Path, PathBuf};
use tauri::State;

fn derivative_candidate_path(source: &Path, suffix: &str, index: usize) -> Result<PathBuf, String> {
    let parent = source.parent().ok_or("Invalid file path: no parent")?;
    let stem = source
        .file_stem()
        .ok_or("Invalid file path: no stem")?
        .to_string_lossy();
    let ext = source.extension().map(|ext| ext.to_string_lossy());

    let indexed_suffix = if index == 0 {
        suffix.to_string()
    } else {
        format!("{suffix}_{}", index + 1)
    };
    let file_name = match &ext {
        Some(ext) if !ext.is_empty() => format!("{stem}_{indexed_suffix}.{ext}"),
        _ => format!("{stem}_{indexed_suffix}"),
    };
    Ok(parent.join(file_name))
}

fn write_derivative_image<W: Write + Seek>(
    image: &DynamicImage,
    format: ImageFormat,
    output: &mut W,
) -> Result<(), String> {
    let mut writer = BufWriter::new(output);
    image
        .write_to(&mut writer, format)
        .map_err(|e| format!("Failed to save: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("Failed to flush derivative temp file: {e}"))?;
    Ok(())
}

fn save_derivative_image(
    image: &DynamicImage,
    source: &Path,
    suffix: &str,
) -> Result<PathBuf, String> {
    for index in 0..10_000 {
        let candidate = derivative_candidate_path(source, suffix, index)?;
        if candidate.exists() {
            continue;
        }
        let format = ImageFormat::from_path(&candidate)
            .map_err(|e| format!("Unsupported derivative image format: {e}"))?;
        let parent = candidate
            .parent()
            .ok_or("Invalid derivative file path: no parent")?;
        let mut temp = tempfile::Builder::new()
            .prefix(".cull-transform-")
            .tempfile_in(parent)
            .map_err(|e| format!("Failed to create derivative temp file: {e}"))?;
        write_derivative_image(image, format, temp.as_file_mut())?;
        temp.as_file_mut()
            .sync_all()
            .map_err(|e| format!("Failed to sync derivative temp file: {e}"))?;
        match temp.persist_noclobber(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(err) if err.error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("Failed to install derivative file: {}", err.error)),
        }
    }

    Err("Could not find an available derivative file name".to_string())
}

fn register_derivative(state: &AppState, output_path: &Path) -> Result<(), String> {
    crate::db_core::import::import_file(&state.db, output_path, &state.app_data_dir)?.ok_or_else(
        || {
            format!(
                "Derivative was written but not registered: {}",
                output_path.to_string_lossy()
            )
        },
    )?;
    Ok(())
}

fn crop_image_inner(
    state: &AppState,
    image_id: &str,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let images = state
        .db
        .get_images_by_ids(&[image_id])
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
    let output_path = save_derivative_image(&cropped, &path, "crop")?;
    register_derivative(state, &output_path)?;

    Ok(output_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn crop_image(
    state: State<'_, AppState>,
    image_id: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    crop_image_inner(&state, &image_id, x, y, width, height)
}

fn rotate_image_inner(state: &AppState, image_id: &str, degrees: i32) -> Result<String, String> {
    let images = state
        .db
        .get_images_by_ids(&[image_id])
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

    let output_path =
        save_derivative_image(&rotated, &path, &format!("rot{}", degrees.rem_euclid(360)))?;
    register_derivative(state, &output_path)?;

    Ok(output_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn rotate_image(
    state: State<'_, AppState>,
    image_id: String,
    degrees: i32,
) -> Result<String, String> {
    rotate_image_inner(&state, &image_id, degrees)
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
    use std::io::{Cursor, Seek, SeekFrom, Write};
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
            static_publish_server: parking_lot::Mutex::new(
                crate::commands::static_publishing::StaticPublishServerState::default(),
            ),
        }
    }

    #[tokio::test]
    async fn crop_default_path_leaves_original_file_bytes_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("source.png");
        write_test_image(&image_path, 8, 6);
        let original_bytes = fs::read(&image_path).unwrap();

        let state = test_state(tmp.path(), &image_path, 8, 6);

        let result = crop_image_inner(&state, IMAGE_ID, 1, 1, 4, 3).unwrap();

        assert_ne!(result, IMAGE_ID);
        assert_eq!(fs::read(&image_path).unwrap(), original_bytes);

        let output_path = PathBuf::from(result);
        assert_ne!(output_path, image_path);
        assert_eq!(image::open(&output_path).unwrap().dimensions(), (4, 3));

        let images = state.db.get_images_by_ids(&[IMAGE_ID]).unwrap();
        assert_eq!(images[0].image.width, 8);
        assert_eq!(images[0].image.height, 6);

        let derivative_file = state
            .db
            .get_image_file_by_path(&output_path.to_string_lossy())
            .unwrap()
            .expect("derivative should be imported into the library");
        assert_ne!(derivative_file.image_id, IMAGE_ID);
        let derivative = state
            .db
            .get_images_by_ids(&[derivative_file.image_id.as_str()])
            .unwrap();
        assert_eq!(derivative[0].image.width, 4);
        assert_eq!(derivative[0].image.height, 3);
    }

    #[tokio::test]
    async fn rotate_default_path_leaves_original_file_bytes_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("source.png");
        write_test_image(&image_path, 8, 6);
        let original_bytes = fs::read(&image_path).unwrap();

        let state = test_state(tmp.path(), &image_path, 8, 6);

        let result = rotate_image_inner(&state, IMAGE_ID, 90).unwrap();

        assert_eq!(fs::read(&image_path).unwrap(), original_bytes);

        let output_path = PathBuf::from(result);
        assert_ne!(output_path, image_path);
        assert_eq!(image::open(&output_path).unwrap().dimensions(), (6, 8));

        let images = state.db.get_images_by_ids(&[IMAGE_ID]).unwrap();
        assert_eq!(images[0].image.width, 8);
        assert_eq!(images[0].image.height, 6);

        let derivative_file = state
            .db
            .get_image_file_by_path(&output_path.to_string_lossy())
            .unwrap()
            .expect("derivative should be imported into the library");
        assert_ne!(derivative_file.image_id, IMAGE_ID);
        let derivative = state
            .db
            .get_images_by_ids(&[derivative_file.image_id.as_str()])
            .unwrap();
        assert_eq!(derivative[0].image.width, 6);
        assert_eq!(derivative[0].image.height, 8);
    }

    #[tokio::test]
    async fn crop_uses_collision_safe_derivative_path() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("source.png");
        let occupied_path = tmp.path().join("source_crop.png");
        write_test_image(&image_path, 8, 6);
        write_test_image(&occupied_path, 2, 2);
        let occupied_bytes = fs::read(&occupied_path).unwrap();

        let state = test_state(tmp.path(), &image_path, 8, 6);

        let result = crop_image_inner(&state, IMAGE_ID, 1, 1, 4, 3).unwrap();

        let output_path = PathBuf::from(result);
        assert_eq!(output_path.file_name().unwrap(), "source_crop_2.png");
        assert_eq!(fs::read(&occupied_path).unwrap(), occupied_bytes);
        assert_eq!(image::open(output_path).unwrap().dimensions(), (4, 3));
    }

    #[test]
    fn derivative_save_error_does_not_leave_final_output_file() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("source.invalid");
        fs::write(&image_path, b"source bytes").unwrap();
        let image = image::DynamicImage::new_rgba8(2, 2);

        let result = save_derivative_image(&image, &image_path, "crop");

        assert!(result.is_err());
        assert!(!tmp.path().join("source_crop.invalid").exists());
        let leftovers: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".cull-transform-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    struct FlushFailWriter {
        inner: Cursor<Vec<u8>>,
    }

    impl Write for FlushFailWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.inner.write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "injected flush failure",
            ))
        }
    }

    impl Seek for FlushFailWriter {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn derivative_writer_reports_flush_errors() {
        let image = image::DynamicImage::new_rgba8(2, 2);
        let mut writer = FlushFailWriter {
            inner: Cursor::new(Vec::new()),
        };

        let result = write_derivative_image(&image, ImageFormat::Png, &mut writer);

        assert!(result
            .unwrap_err()
            .contains("Failed to flush derivative temp file"),);
    }
}
