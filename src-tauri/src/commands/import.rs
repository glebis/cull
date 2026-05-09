use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};
use crate::AppState;

#[derive(serde::Serialize)]
pub struct ImportResponse {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
}

#[derive(Clone, serde::Serialize)]
struct ImportProgress {
    current: u32,
    total: u32,
    filename: String,
}

#[tauri::command]
pub async fn import_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<ImportResponse, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;

    // Collect all image files first so we know the total
    let extensions = ["jpg", "jpeg", "png", "webp", "gif"];
    let entries: Vec<std::path::PathBuf> = walkdir::WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total = entries.len() as u32;
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();
    let mut new_image_ids: Vec<String> = Vec::new();

    for (i, path) in entries.iter().enumerate() {
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let _ = app.emit(
            "import-progress",
            ImportProgress {
                current: (i + 1) as u32,
                total,
                filename,
            },
        );

        match crate::db_core::import::import_file(db, path, app_data_dir) {
            Ok(Some(id)) => {
                new_image_ids.push(id);
                imported += 1;
            }
            Ok(None) => skipped += 1,
            Err(e) => errors.push(format!("{}: {}", path.display(), e)),
        }
    }

    if !new_image_ids.is_empty() {
        run_post_import_detection(app.clone(), new_image_ids);
    }

    Ok(ImportResponse {
        imported,
        skipped,
        errors,
    })
}

#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    state: State<'_, AppState>,
    file_paths: Vec<String>,
) -> Result<ImportResponse, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    let mut new_image_ids: Vec<String> = Vec::new();

    for path_str in file_paths {
        match crate::db_core::import::import_file(db, Path::new(&path_str), app_data_dir) {
            Ok(Some(id)) => {
                new_image_ids.push(id);
                imported += 1;
            }
            Ok(None) => skipped += 1,
            Err(e) => errors.push(format!("{}: {}", path_str, e)),
        }
    }

    if !new_image_ids.is_empty() {
        run_post_import_detection(app, new_image_ids);
    }

    Ok(ImportResponse { imported, skipped, errors })
}

#[derive(Clone, serde::Serialize)]
struct ThumbnailProgress {
    current: u32,
    total: u32,
}

#[tauri::command]
pub async fn regenerate_thumbnails(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let images = db.list_images(100000, 0).map_err(|e| e.to_string())?;
    let total = images.len() as u32;
    let mut regenerated = 0u32;

    for (i, img) in images.iter().enumerate() {
        let source_path = std::path::Path::new(&img.path);
        if source_path.exists() {
            match crate::db_core::thumbnails::generate_thumbnail(source_path, app_data_dir, &img.image.id) {
                Ok(_) => regenerated += 1,
                Err(e) => eprintln!("Thumbnail failed for {}: {}", img.path, e),
            }
        }
        let _ = app.emit("thumbnail-progress", ThumbnailProgress {
            current: (i + 1) as u32,
            total,
        });
    }

    Ok(regenerated)
}

fn run_post_import_detection(app: AppHandle, image_ids: Vec<String>) {
    tokio::spawn(async move {
        let state: State<'_, AppState> = app.state();

        // YOLO detection (if model available)
        let yolo_available = {
            let engine = state.detection_engine.lock().unwrap();
            engine.is_variant_available(crate::db_core::detection::YoloVariant::Medium)
        };

        if yolo_available {
            let _ = app.emit("auto-detection-start", serde_json::json!({
                "model": "yolov8m", "count": image_ids.len()
            }));

            {
                let mut engine = state.detection_engine.lock().unwrap();
                if engine.session.is_none() {
                    let _ = engine.load_yolo(crate::db_core::detection::YoloVariant::Medium);
                }
            }

            for (i, image_id) in image_ids.iter().enumerate() {
                let id_refs: Vec<&str> = vec![image_id.as_str()];
                if let Ok(images) = state.db.get_images_by_ids(&id_refs) {
                    if let Some(img) = images.first() {
                        let engine = state.detection_engine.lock().unwrap();
                        if let Ok(detections) = engine.detect(std::path::Path::new(&img.path)) {
                            drop(engine);
                            let _ = state.db.store_detections(image_id, "yolov8m", &detections);
                        }
                    }
                }

                let _ = app.emit("auto-detection-progress", serde_json::json!({
                    "current": i + 1, "total": image_ids.len(), "model": "yolov8m"
                }));
            }
        }

        // NudeNet safety check (if model available)
        let nudenet_available = {
            let engine = state.safety_engine.lock().unwrap();
            engine.is_nudenet_available()
        };

        if nudenet_available {
            let _ = app.emit("auto-detection-start", serde_json::json!({
                "model": "nudenet", "count": image_ids.len()
            }));

            {
                let mut engine = state.safety_engine.lock().unwrap();
                if engine.session.is_none() {
                    let _ = engine.load_nudenet();
                }
            }

            for (i, image_id) in image_ids.iter().enumerate() {
                let id_refs: Vec<&str> = vec![image_id.as_str()];
                if let Ok(images) = state.db.get_images_by_ids(&id_refs) {
                    if let Some(img) = images.first() {
                        let engine = state.safety_engine.lock().unwrap();
                        if let Ok(detections) = engine.detect(std::path::Path::new(&img.path)) {
                            drop(engine);
                            let _ = state.db.store_detections(image_id, "nudenet", &detections);
                        }
                    }
                }

                let _ = app.emit("auto-detection-progress", serde_json::json!({
                    "current": i + 1, "total": image_ids.len(), "model": "nudenet"
                }));
            }
        }

        let _ = app.emit("auto-detection-complete", serde_json::json!({
            "count": image_ids.len()
        }));
    });
}
