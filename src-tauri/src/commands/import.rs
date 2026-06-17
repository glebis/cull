use crate::db_core::db::Database;
use crate::db_core::models::NewSessionEvent;
use crate::AppState;
use std::collections::HashSet;
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
    let module_raw = crate::db_core::import::is_module_raw_enabled(&state.db);
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
    let auto_process_ids =
        filter_image_ids_for_auto_jobs(&db, &new_image_ids).map_err(|e| e.to_string())?;

    if !auto_process_ids.is_empty() {
        run_post_import_quality_analysis(app.clone(), auto_process_ids.clone());
        run_post_import_detection(app.clone(), auto_process_ids);
    }
    let _ = crate::tray::refresh_tray_menu(&app);

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
    let auto_process_ids =
        filter_image_ids_for_auto_jobs(&db, &new_image_ids).map_err(|e| e.to_string())?;

    if !new_image_ids.is_empty() {
        let _ = crate::tray::refresh_tray_menu(&app);
        if !auto_process_ids.is_empty() {
            run_post_import_quality_analysis(app.clone(), auto_process_ids.clone());
            run_post_import_detection(app, auto_process_ids);
        }
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
    let image_ids = db.list_image_ids().map_err(|e| e.to_string())?;
    let total = image_ids.len() as u32;
    let mut regenerated = 0u32;
    let mut processed = 0u32;

    for chunk in image_ids.chunks(250) {
        let id_refs: Vec<&str> = chunk.iter().map(|id| id.as_str()).collect();
        let images = db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        for img in images {
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
                                Err(e) => crate::safe_eprintln!(
                                    "RAW thumbnail failed for {}: {}",
                                    img.path,
                                    e
                                ),
                            }
                        }
                        Err(e) => {
                            crate::safe_eprintln!("RAW decode failed for {}: {}", img.path, e)
                        }
                    }
                } else if crate::extensions::is_document_extension(ext) {
                    match crate::db_core::thumbnails::generate_document_thumbnail(
                        source_path,
                        app_data_dir,
                        &img.image.id,
                    ) {
                        Ok(_) => regenerated += 1,
                        Err(e) => {
                            crate::safe_eprintln!(
                                "Document thumbnail failed for {}: {}",
                                img.path,
                                e
                            )
                        }
                    }
                } else {
                    match crate::db_core::thumbnails::generate_thumbnail(
                        source_path,
                        app_data_dir,
                        &img.image.id,
                    ) {
                        Ok(_) => regenerated += 1,
                        Err(e) => crate::safe_eprintln!("Thumbnail failed for {}: {}", img.path, e),
                    }
                }
            }
            processed += 1;
            let _ = app.emit(
                "thumbnail-progress",
                ThumbnailProgress {
                    current: processed,
                    total,
                },
            );
        }
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
                                        crate::safe_eprintln!(
                                            "RAW thumbnail failed for {}: {}",
                                            img.path,
                                            e
                                        )
                                    }
                                }
                            }
                            Err(e) => {
                                crate::safe_eprintln!("RAW decode failed for {}: {}", img.path, e)
                            }
                        }
                    } else if crate::extensions::is_document_extension(ext) {
                        match crate::db_core::thumbnails::generate_document_thumbnail(
                            source_path,
                            app_data_dir,
                            &img.image.id,
                        ) {
                            Ok(_) => regenerated += 1,
                            Err(e) => {
                                crate::safe_eprintln!(
                                    "Document thumbnail failed for {}: {}",
                                    img.path,
                                    e
                                )
                            }
                        }
                    } else {
                        match crate::db_core::thumbnails::generate_thumbnail(
                            source_path,
                            app_data_dir,
                            &img.image.id,
                        ) {
                            Ok(_) => regenerated += 1,
                            Err(e) => {
                                crate::safe_eprintln!("Thumbnail failed for {}: {}", img.path, e)
                            }
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
    } else if crate::extensions::is_document_extension(ext) {
        crate::db_core::thumbnails::generate_document_thumbnail(
            source_path,
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
    let image_ids = db.list_image_ids().map_err(|e| e.to_string())?;
    let total = image_ids.len() as u32;
    let mut updated = 0u32;
    let mut processed = 0u32;

    for chunk in image_ids.chunks(250) {
        let id_refs: Vec<&str> = chunk.iter().map(|id| id.as_str()).collect();
        let images = db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        for img in images {
            let path = std::path::Path::new(&img.path);
            if !path.exists() {
                processed += 1;
                let _ = app.emit(
                    "rescan-progress",
                    serde_json::json!({
                        "current": processed, "total": total
                    }),
                );
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

            processed += 1;
            let _ = app.emit(
                "rescan-progress",
                serde_json::json!({
                    "current": processed, "total": total
                }),
            );
        }
    }

    Ok(updated)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImportQualitySummary {
    analyzed: u32,
    failed: u32,
    cancelled: bool,
}

fn run_post_import_quality_analysis(app: AppHandle, image_ids: Vec<String>) {
    if image_ids.is_empty() {
        return;
    }

    let app_clone = app.clone();
    crate::spawn_guarded(app_clone, "post-import-quality", move || async move {
        let state: State<'_, AppState> = app.state();
        let total = image_ids.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("quality", total);
        let progress_job_id = job_id.clone();
        let progress_app = app.clone();
        let cancel_for_loop = cancel_token.clone();

        let _ = app.emit(
            "job-status-changed",
            serde_json::json!({
                "job_id": &job_id,
                "kind": "quality",
                "status": "running",
                "current": 0,
                "total": total,
            }),
        );

        let summary = analyze_quality_for_imported_images(
            &state.db,
            &state.app_data_dir,
            &image_ids,
            |current, total| {
                state.jobs.update_progress(&progress_job_id, current, None);
                let _ = progress_app.emit(
                    "quality-progress",
                    serde_json::json!({
                        "job_id": &progress_job_id,
                        "current": current,
                        "total": total,
                        "analyzer": crate::db_core::quality::QUALITY_ANALYZER_VERSION,
                    }),
                );
            },
            move || cancel_for_loop.is_cancelled(),
        );

        let status = if summary.cancelled {
            state.jobs.mark_cancelled(&job_id);
            "cancelled"
        } else {
            state.jobs.complete(&job_id);
            "completed"
        };
        state.jobs.persist_terminal(&job_id, &state.db);
        let _ = app.emit(
            "job-status-changed",
            serde_json::json!({
                "job_id": &job_id,
                "kind": "quality",
                "status": status,
                "current": if summary.cancelled { summary.analyzed + summary.failed } else { total },
                "total": total,
                "message": format!("{} analyzed, {} skipped", summary.analyzed, summary.failed),
            }),
        );
    });
}

fn analyze_quality_for_imported_images<F, C>(
    db: &Database,
    app_data_dir: &Path,
    image_ids: &[String],
    mut on_progress: F,
    mut should_cancel: C,
) -> ImportQualitySummary
where
    F: FnMut(u32, u32),
    C: FnMut() -> bool,
{
    let total = image_ids.len() as u32;
    let mut summary = ImportQualitySummary {
        analyzed: 0,
        failed: 0,
        cancelled: false,
    };

    for (index, image_id) in image_ids.iter().enumerate() {
        if should_cancel() {
            summary.cancelled = true;
            break;
        }

        match analyze_quality_for_imported_image(db, app_data_dir, image_id) {
            Ok(()) => summary.analyzed += 1,
            Err(e) => {
                summary.failed += 1;
                crate::safe_eprintln!("Quality analysis error for {}: {}", image_id, e);
            }
        }
        on_progress((index + 1) as u32, total);
    }

    summary
}

fn analyze_quality_for_imported_image(
    db: &Database,
    app_data_dir: &Path,
    image_id: &str,
) -> Result<(), String> {
    let images = db
        .get_images_by_ids(&[image_id])
        .map_err(|e| e.to_string())?;
    let image = images
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_id))?;
    let ml_path = crate::commands::resolve_image_path_for_ml(image, app_data_dir);
    let metrics = crate::db_core::quality::analyze_image_quality(image_id, &ml_path)?;
    db.store_image_quality_metrics(&metrics)
        .map_err(|e| e.to_string())
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
                        let detect_path =
                            crate::commands::resolve_image_path_for_ml(img, &state.app_data_dir);
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
                        let detect_path =
                            crate::commands::resolve_image_path_for_ml(img, &state.app_data_dir);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;

    #[test]
    fn post_import_quality_analysis_stores_metrics_without_inline_import_work() {
        let tmp = tempfile::tempdir().unwrap();
        let app_data_dir = tmp.path().join("app-data");
        std::fs::create_dir(&app_data_dir).unwrap();
        let image_path = tmp.path().join("checker.png");
        let image = image::ImageBuffer::from_fn(32, 32, |x, y| {
            let value: u8 = if (x + y) % 2 == 0 { 255 } else { 0 };
            image::Rgba([value, value, value, 255])
        });
        image.save(&image_path).unwrap();

        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image_id = crate::db_core::import::import_file(&db, &image_path, &app_data_dir)
            .unwrap()
            .unwrap();

        assert!(db.get_image_quality_metrics(&image_id).unwrap().is_none());

        let summary = analyze_quality_for_imported_images(
            &db,
            &app_data_dir,
            &[image_id.clone()],
            |_current, _total| {},
            || false,
        );

        assert_eq!(summary.analyzed, 1);
        assert_eq!(summary.failed, 0);
        assert!(!summary.cancelled);
        assert!(db.get_image_quality_metrics(&image_id).unwrap().is_some());
    }

    #[test]
    fn imports_bmp_with_dimensions_and_thumbnail() {
        let tmp = tempfile::tempdir().unwrap();
        let app_data_dir = tmp.path().join("app-data");
        std::fs::create_dir(&app_data_dir).unwrap();
        let image_path = tmp.path().join("sample.bmp");
        let image = image::ImageBuffer::from_fn(24, 16, |x, y| {
            let red = (x * 10) as u8;
            let green = (y * 12) as u8;
            image::Rgb([red, green, 128])
        });
        image.save(&image_path).unwrap();

        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image_id = crate::db_core::import::import_file(&db, &image_path, &app_data_dir)
            .unwrap()
            .unwrap();
        let images = db.get_images_by_ids(&[&image_id]).unwrap();
        let imported = images.first().unwrap();

        assert_eq!(imported.image.width, 24);
        assert_eq!(imported.image.height, 16);
        assert!(crate::db_core::thumbnails::thumbnail_path(&app_data_dir, &image_id).exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn imports_svg_with_platform_decoder_thumbnail_and_quality() {
        let tmp = tempfile::tempdir().unwrap();
        let app_data_dir = tmp.path().join("app-data");
        std::fs::create_dir(&app_data_dir).unwrap();
        let image_path = tmp.path().join("poster.svg");
        std::fs::write(
            &image_path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48"><rect width="64" height="48" fill="#1a1a2e"/><circle cx="32" cy="24" r="16" fill="#bb9af7"/></svg>"##,
        )
        .unwrap();

        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image_id = crate::db_core::import::import_file(&db, &image_path, &app_data_dir)
            .unwrap()
            .unwrap();
        let images = db.get_images_by_ids(&[&image_id]).unwrap();
        let imported = images.first().unwrap();

        assert_eq!(imported.image.width, 64);
        assert_eq!(imported.image.height, 48);
        assert!(crate::db_core::thumbnails::thumbnail_path(&app_data_dir, &image_id).exists());

        let summary = analyze_quality_for_imported_images(
            &db,
            &app_data_dir,
            &[image_id.clone()],
            |_current, _total| {},
            || false,
        );

        assert_eq!(summary.analyzed, 1);
        assert_eq!(summary.failed, 0);
        assert!(db.get_image_quality_metrics(&image_id).unwrap().is_some());
    }

    #[test]
    fn auto_job_filter_skips_pdf_assets() {
        let tmp = tempfile::tempdir().unwrap();
        let filter_db_path = tmp.path().join("filter.db");
        let db = Database::open(&filter_db_path).unwrap();
        let app_data_dir = tmp.path().join("app-data");
        std::fs::create_dir(&app_data_dir).unwrap();

        let png_path = tmp.path().join("image.png");
        image::RgbImage::from_fn(2, 2, |_, _| image::Rgb([0, 0, 0]))
            .save(&png_path)
            .unwrap();

        let pdf_path = tmp.path().join("doc.pdf");
        std::fs::write(&pdf_path, b"%PDF-1.4 sample").unwrap();

        let png_id = crate::db_core::import::import_file(&db, &png_path, &app_data_dir)
            .unwrap()
            .unwrap();
        let pdf_id = crate::db_core::import::import_file(&db, &pdf_path, &app_data_dir)
            .unwrap()
            .unwrap();

        let ids = filter_image_ids_for_auto_jobs(&db, &[png_id.clone(), pdf_id.clone()]).unwrap();
        assert_eq!(ids, vec![png_id.clone()]);
        assert!(!ids.contains(&pdf_id));
    }
}

fn filter_image_ids_for_auto_jobs(
    db: &Database,
    image_ids: &[String],
) -> Result<Vec<String>, String> {
    if image_ids.is_empty() {
        return Ok(vec![]);
    }

    let refs: Vec<&str> = image_ids.iter().map(|id| id.as_str()).collect();
    let images = db.get_images_by_ids(&refs).map_err(|e| e.to_string())?;
    let mut document_ids: HashSet<String> = HashSet::new();
    let mut existing_ids: HashSet<String> = HashSet::new();

    for img in images {
        existing_ids.insert(img.image.id.clone());
        if crate::extensions::is_document_extension(&img.image.format) {
            document_ids.insert(img.image.id);
        }
    }

    Ok(image_ids
        .iter()
        .filter(|id| existing_ids.contains(*id))
        .filter(|id| !document_ids.contains(*id))
        .cloned()
        .collect())
}
