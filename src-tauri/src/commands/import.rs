use crate::db_core::models::NewSessionEvent;
use crate::AppState;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(serde::Serialize)]
pub struct ImportResponse {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub batch_id: Option<String>,
    pub image_ids: Vec<String>,
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
    session_id: Option<String>,
) -> Result<ImportResponse, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;

    // Collect all image files first so we know the total
    let module_raw = state
        .db
        .get_setting("module_raw")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);
    let extensions = crate::extensions::supported_extensions(module_raw);
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

    let _ = state.db.add_library_root(&folder_path);

    let batch_id = if !new_image_ids.is_empty() {
        let batch = db
            .create_import_batch("folder", new_image_ids.len() as u32, session_id.as_deref())
            .map_err(|e| e.to_string())?;
        for id in &new_image_ids {
            let _ = db.set_image_batch(id, &batch);
        }
        if let Some(active_session_id) = session_id.as_deref() {
            let refs: Vec<&str> = new_image_ids.iter().map(|id| id.as_str()).collect();
            let _ = db.add_to_collection(active_session_id, &refs);
        }
        let _ = db.detect_lineage_for_batch(&new_image_ids);
        let _ = db.log_session_event(&NewSessionEvent {
            session_id: session_id.clone(),
            event_type: "import_completed".to_string(),
            actor_type: "user".to_string(),
            actor_id: None,
            subject_type: Some("import_batch".to_string()),
            subject_id: Some(batch.clone()),
            payload_json: serde_json::json!({
                "source": "folder",
                "source_path": folder_path,
                "imported": imported,
                "skipped": skipped,
                "error_count": errors.len(),
                "image_count": new_image_ids.len(),
            })
            .to_string(),
        });
        Some(batch)
    } else {
        None
    };

    let image_ids_out = new_image_ids.clone();

    if !new_image_ids.is_empty() {
        run_post_import_detection(app.clone(), new_image_ids);
    }

    Ok(ImportResponse {
        imported,
        skipped,
        errors,
        batch_id,
        image_ids: image_ids_out,
    })
}

#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    state: State<'_, AppState>,
    file_paths: Vec<String>,
    session_id: Option<String>,
) -> Result<ImportResponse, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    let mut new_image_ids: Vec<String> = Vec::new();
    let total = file_paths.len() as u32;

    for (i, path_str) in file_paths.iter().enumerate() {
        let filename = Path::new(path_str.as_str())
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());

        let _ = app.emit(
            "import-progress",
            ImportProgress {
                current: (i + 1) as u32,
                total,
                filename,
            },
        );

        match crate::db_core::import::import_file(db, Path::new(path_str.as_str()), app_data_dir) {
            Ok(Some(id)) => {
                new_image_ids.push(id);
                imported += 1;
            }
            Ok(None) => skipped += 1,
            Err(e) => errors.push(format!("{}: {}", path_str, e)),
        }
    }

    let batch_id = if !new_image_ids.is_empty() {
        let batch = db
            .create_import_batch("cli", new_image_ids.len() as u32, session_id.as_deref())
            .map_err(|e| e.to_string())?;
        for id in &new_image_ids {
            let _ = db.set_image_batch(id, &batch);
        }
        if let Some(active_session_id) = session_id.as_deref() {
            let refs: Vec<&str> = new_image_ids.iter().map(|id| id.as_str()).collect();
            let _ = db.add_to_collection(active_session_id, &refs);
        }
        let _ = db.detect_lineage_for_batch(&new_image_ids);
        let _ = db.log_session_event(&NewSessionEvent {
            session_id: session_id.clone(),
            event_type: "import_completed".to_string(),
            actor_type: "user".to_string(),
            actor_id: None,
            subject_type: Some("import_batch".to_string()),
            subject_id: Some(batch.clone()),
            payload_json: serde_json::json!({
                "source": "files",
                "file_count": file_paths.len(),
                "imported": imported,
                "skipped": skipped,
                "error_count": errors.len(),
                "image_count": new_image_ids.len(),
            })
            .to_string(),
        });
        Some(batch)
    } else {
        None
    };

    let image_ids_out = new_image_ids.clone();

    if !new_image_ids.is_empty() {
        run_post_import_detection(app, new_image_ids);
    }

    Ok(ImportResponse {
        imported,
        skipped,
        errors,
        batch_id,
        image_ids: image_ids_out,
    })
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
            let ext = source_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if crate::extensions::is_raw_extension(ext) {
                match crate::raw::decode_raw_preview(source_path) {
                    Ok(preview) => {
                        match crate::db_core::thumbnails::generate_thumbnail_from_image(
                            &preview.image,
                            app_data_dir,
                            &img.image.id,
                        ) {
                            Ok(_) => regenerated += 1,
                            Err(e) => eprintln!("RAW thumbnail failed for {}: {}", img.path, e),
                        }
                    }
                    Err(e) => eprintln!("RAW decode failed for {}: {}", img.path, e),
                }
            } else {
                match crate::db_core::thumbnails::generate_thumbnail(
                    source_path,
                    app_data_dir,
                    &img.image.id,
                ) {
                    Ok(_) => regenerated += 1,
                    Err(e) => eprintln!("Thumbnail failed for {}: {}", img.path, e),
                }
            }
        }
        let _ = app.emit(
            "thumbnail-progress",
            ThumbnailProgress {
                current: (i + 1) as u32,
                total,
            },
        );
    }

    Ok(regenerated)
}

#[tauri::command]
pub async fn regenerate_thumbnails_by_ids(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let total = image_ids.len() as u32;
    let mut regenerated = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        if let Ok(found) = db.get_images_by_ids(&id_refs) {
            if let Some(img) = found.first() {
                let source_path = std::path::Path::new(&img.path);
                if source_path.exists() {
                    let ext = source_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if crate::extensions::is_raw_extension(ext) {
                        match crate::raw::decode_raw_preview(source_path) {
                            Ok(preview) => {
                                match crate::db_core::thumbnails::generate_thumbnail_from_image(
                                    &preview.image,
                                    app_data_dir,
                                    &img.image.id,
                                ) {
                                    Ok(_) => regenerated += 1,
                                    Err(e) => {
                                        eprintln!("RAW thumbnail failed for {}: {}", img.path, e)
                                    }
                                }
                            }
                            Err(e) => eprintln!("RAW decode failed for {}: {}", img.path, e),
                        }
                    } else {
                        match crate::db_core::thumbnails::generate_thumbnail(
                            source_path,
                            app_data_dir,
                            &img.image.id,
                        ) {
                            Ok(_) => regenerated += 1,
                            Err(e) => eprintln!("Thumbnail failed for {}: {}", img.path, e),
                        }
                    }
                }
            }
        }
        let _ = app.emit(
            "thumbnail-progress",
            ThumbnailProgress {
                current: (i + 1) as u32,
                total,
            },
        );
    }

    Ok(regenerated)
}

#[tauri::command]
pub async fn regenerate_single_thumbnail(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<String, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let id_refs: Vec<&str> = vec![image_id.as_str()];
    let found = db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
    let img = found
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_id))?;
    let source_path = std::path::Path::new(&img.path);
    if !source_path.exists() {
        return Err(format!("Source file missing: {}", img.path));
    }
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let thumb_path = if crate::extensions::is_raw_extension(ext) {
        let preview = crate::raw::decode_raw_preview(source_path)
            .map_err(|e| format!("RAW decode failed: {}", e))?;
        crate::db_core::thumbnails::generate_thumbnail_from_image(
            &preview.image,
            app_data_dir,
            &image_id,
        )?
    } else {
        crate::db_core::thumbnails::generate_thumbnail(source_path, app_data_dir, &image_id)?
    };
    Ok(thumb_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn rescan_sources(app: AppHandle, state: State<'_, AppState>) -> Result<u32, String> {
    let db = &state.db;
    let images = db.list_images(100000, 0).map_err(|e| e.to_string())?;
    let total = images.len() as u32;
    let mut updated = 0u32;

    for (i, img) in images.iter().enumerate() {
        let path = std::path::Path::new(&img.path);
        if !path.exists() {
            continue;
        }

        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        let png_chunks = if ext == "png" {
            crate::db_core::source_detection::read_png_text_chunks(path).unwrap_or_default()
        } else {
            vec![]
        };

        let detection =
            crate::db_core::source_detection::detect_source(filename, &png_chunks, path);

        if detection.source_label.is_some() {
            let aspect_ratio = img.image.width as f64 / img.image.height.max(1) as f64;
            let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
                "square"
            } else if aspect_ratio > 1.0 {
                "landscape"
            } else {
                "portrait"
            };
            let megapixels = (img.image.width as f64 * img.image.height as f64) / 1_000_000.0;

            let _ = db.update_source_detection(
                &img.image.id,
                detection.source_label.as_deref(),
                detection.confidence,
                &detection.to_evidence_json(),
                detection.is_ai_generated,
                detection.ai_prompt.as_deref(),
                aspect_ratio,
                orientation,
                megapixels,
            );
            updated += 1;
        }

        let _ = app.emit(
            "rescan-progress",
            serde_json::json!({
                "current": i + 1, "total": total
            }),
        );
    }

    Ok(updated)
}

fn run_post_import_detection(app: AppHandle, image_ids: Vec<String>) {
    let app_clone = app.clone();
    crate::spawn_guarded(app_clone, "post-import-detection", move || async move {
        let state: State<'_, AppState> = app.state();

        // YOLO detection (if model available)
        let yolo_available = {
            let engine = state.detection_engine.lock();
            engine.is_variant_available(crate::db_core::detection::YoloVariant::Medium)
        };

        if yolo_available {
            let _ = app.emit(
                "auto-detection-start",
                serde_json::json!({
                    "model": "yolov8m", "count": image_ids.len()
                }),
            );

            {
                let mut engine = state.detection_engine.lock();
                if engine.session.is_none() {
                    let _ = engine.load_yolo(crate::db_core::detection::YoloVariant::Medium);
                }
            }

            for (i, image_id) in image_ids.iter().enumerate() {
                let id_refs: Vec<&str> = vec![image_id.as_str()];
                if let Ok(images) = state.db.get_images_by_ids(&id_refs) {
                    if let Some(img) = images.first() {
                        let detect_path = if crate::extensions::is_raw_extension(
                            std::path::Path::new(&img.path)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or(""),
                        ) {
                            crate::db_core::thumbnails::thumbnail_path(
                                &app.state::<AppState>().app_data_dir,
                                image_id,
                            )
                        } else {
                            std::path::PathBuf::from(&img.path)
                        };
                        let engine = state.detection_engine.lock();
                        if let Ok(detections) = engine.detect(&detect_path) {
                            drop(engine);
                            let _ = state.db.store_detections(image_id, "yolov8m", &detections);
                        }
                    }
                }

                let _ = app.emit(
                    "auto-detection-progress",
                    serde_json::json!({
                        "current": i + 1, "total": image_ids.len(), "model": "yolov8m"
                    }),
                );
            }
        }

        // NudeNet safety check (if model available)
        let nudenet_available = {
            let engine = state.safety_engine.lock();
            engine.is_nudenet_available()
        };

        if nudenet_available {
            let _ = app.emit(
                "auto-detection-start",
                serde_json::json!({
                    "model": "nudenet", "count": image_ids.len()
                }),
            );

            {
                let mut engine = state.safety_engine.lock();
                if engine.session.is_none() {
                    let _ = engine.load_nudenet();
                }
            }

            for (i, image_id) in image_ids.iter().enumerate() {
                let id_refs: Vec<&str> = vec![image_id.as_str()];
                if let Ok(images) = state.db.get_images_by_ids(&id_refs) {
                    if let Some(img) = images.first() {
                        let detect_path = if crate::extensions::is_raw_extension(
                            std::path::Path::new(&img.path)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or(""),
                        ) {
                            crate::db_core::thumbnails::thumbnail_path(
                                &app.state::<AppState>().app_data_dir,
                                image_id,
                            )
                        } else {
                            std::path::PathBuf::from(&img.path)
                        };
                        let engine = state.safety_engine.lock();
                        if let Ok(detections) = engine.detect(&detect_path) {
                            drop(engine);
                            let _ = state.db.store_detections(image_id, "nudenet", &detections);
                        }
                    }
                }

                let _ = app.emit(
                    "auto-detection-progress",
                    serde_json::json!({
                        "current": i + 1, "total": image_ids.len(), "model": "nudenet"
                    }),
                );
            }
        }

        let _ = app.emit(
            "auto-detection-complete",
            serde_json::json!({
                "count": image_ids.len()
            }),
        );
    });
}
