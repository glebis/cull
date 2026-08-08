use crate::db_core::db::Database;
use crate::db_core::models::NewSessionEvent;
use crate::services::jobs::JobRegistry;
use crate::AppState;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

#[derive(serde::Serialize)]
pub struct ImportResponse {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub batch_id: Option<String>,
    pub image_ids: Vec<String>,
    pub cancelled: bool,
}

#[derive(Clone, serde::Serialize)]
struct ImportProgress {
    job_id: String,
    progress_id: Option<String>,
    current: u32,
    total: u32,
    filename: String,
}

struct ImportWorkResult {
    response: ImportResponse,
    cancelled: bool,
}

struct ImportPathSummary {
    imported: u32,
    skipped: u32,
    errors: Vec<String>,
    image_ids: Vec<String>,
    cancelled: bool,
}

struct ImportDiscovery {
    entries: Vec<PathBuf>,
    cancelled: bool,
}

fn collect_cancellable_entries<I, F>(
    candidates: I,
    cancel: &CancellationToken,
    mut is_supported: F,
) -> ImportDiscovery
where
    I: IntoIterator<Item = PathBuf>,
    F: FnMut(&Path) -> bool,
{
    let mut entries = Vec::new();
    for path in candidates {
        if cancel.is_cancelled() {
            return ImportDiscovery {
                entries,
                cancelled: true,
            };
        }
        if is_supported(&path) {
            entries.push(path);
        }
    }
    ImportDiscovery {
        entries,
        cancelled: false,
    }
}

fn process_import_entries<P, F>(
    entries: &[PathBuf],
    cancel: &CancellationToken,
    mut on_progress: P,
    mut import_one: F,
) -> ImportPathSummary
where
    P: FnMut(u32, u32, &Path),
    F: FnMut(&Path) -> Result<Option<String>, String>,
{
    let total = entries.len() as u32;
    let mut summary = ImportPathSummary {
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
        image_ids: Vec::new(),
        cancelled: false,
    };

    for (index, path) in entries.iter().enumerate() {
        if cancel.is_cancelled() {
            summary.cancelled = true;
            break;
        }

        on_progress((index + 1) as u32, total, path);
        match import_one(path) {
            Ok(Some(id)) => {
                summary.image_ids.push(id);
                summary.imported += 1;
            }
            Ok(None) => summary.skipped += 1,
            Err(_) if cancel.is_cancelled() => {
                summary.cancelled = true;
                break;
            }
            Err(error) => summary
                .errors
                .push(format!("{}: {}", path.display(), error)),
        }
        if cancel.is_cancelled() {
            summary.cancelled = true;
            break;
        }
    }

    summary
}

fn emit_import_job_status(app: &AppHandle, job_id: &str, status: &str, current: u32, total: u32) {
    let _ = app.emit(
        "job-status-changed",
        serde_json::json!({
            "job_id": job_id,
            "kind": "import",
            "status": status,
            "current": current,
            "total": total,
        }),
    );
}

fn finish_import_job(
    app: &AppHandle,
    jobs: &JobRegistry,
    job_id: &str,
    work: &Result<ImportWorkResult, String>,
) {
    match work {
        Ok(result) if result.cancelled => jobs.mark_cancelled(job_id),
        Ok(_) => jobs.complete(job_id),
        Err(error) => jobs.fail(job_id, error),
    }
    if let Some(snapshot) = jobs.get(job_id) {
        emit_import_job_status(
            app,
            job_id,
            &snapshot.status,
            snapshot.current,
            snapshot.total,
        );
    }
}

fn seal_import_job(
    jobs: &JobRegistry,
    job_id: &str,
    cancel: &CancellationToken,
    already_cancelled: bool,
) -> bool {
    if already_cancelled || cancel.is_cancelled() {
        jobs.mark_cancelled(job_id);
    } else {
        // Completing here closes the cancellation window before audit logging
        // and automatic child-job dispatch. Later cancel requests are rejected.
        jobs.complete(job_id);
    }
    jobs.get(job_id)
        .map(|snapshot| snapshot.status == "cancelled")
        .unwrap_or(already_cancelled || cancel.is_cancelled())
}

fn import_audit_event_type(cancelled: bool) -> &'static str {
    if cancelled {
        "import_cancelled"
    } else {
        "import_completed"
    }
}

async fn persist_import_job(db: Database, jobs: JobRegistry, job_id: String) {
    let _ = tauri::async_runtime::spawn_blocking(move || jobs.persist_terminal(&job_id, &db)).await;
}

#[tauri::command]
pub async fn import_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
    session_id: Option<String>,
    progress_id: Option<String>,
) -> Result<ImportResponse, String> {
    let db = state.db.clone();
    let app_data_dir = state.app_data_dir.clone();
    let jobs = state.jobs.clone();
    let (job_id, cancel) = jobs.create_job("import", 0);
    emit_import_job_status(&app, &job_id, "running", 0, 0);

    let worker_app = app.clone();
    let worker_jobs = jobs.clone();
    let worker_job_id = job_id.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        import_folder_blocking(
            worker_app,
            db,
            app_data_dir,
            worker_jobs,
            worker_job_id,
            cancel,
            folder_path,
            session_id,
            progress_id,
        )
    })
    .await;
    let work = match joined {
        Ok(work) => work,
        Err(error) => Err(format!("Import worker failed: {error}")),
    };

    finish_import_job(&app, &jobs, &job_id, &work);
    persist_import_job(state.db.clone(), jobs, job_id).await;
    work.map(|result| result.response)
}

#[allow(clippy::too_many_arguments)]
fn import_folder_blocking(
    app: AppHandle,
    db: Database,
    app_data_dir: PathBuf,
    jobs: JobRegistry,
    job_id: String,
    cancel: CancellationToken,
    folder_path: String,
    session_id: Option<String>,
    progress_id: Option<String>,
) -> Result<ImportWorkResult, String> {
    // Collect all image files first so we know the total
    let module_raw = crate::db_core::import::is_module_raw_enabled(&db);
    let extensions = crate::extensions::supported_extensions(module_raw);
    let candidates = walkdir::WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf());
    let discovery = collect_cancellable_entries(candidates, &cancel, |path| {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| extensions.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    });
    if discovery.cancelled {
        return Ok(ImportWorkResult {
            response: ImportResponse {
                imported: 0,
                skipped: 0,
                errors: Vec::new(),
                batch_id: None,
                image_ids: Vec::new(),
                cancelled: true,
            },
            cancelled: true,
        });
    }
    let entries = discovery.entries;

    let total = entries.len() as u32;
    jobs.update_total(&job_id, total);
    let summary = process_import_entries(
        &entries,
        &cancel,
        |current, total, path| {
            let filename = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            let _ = app.emit(
                "import-progress",
                ImportProgress {
                    job_id: job_id.clone(),
                    progress_id: progress_id.clone(),
                    current,
                    total,
                    filename: filename.clone(),
                },
            );
            jobs.update_progress(&job_id, current, Some(&filename));
        },
        |path| {
            crate::db_core::import::import_file_cancellable(&db, path, &app_data_dir, &|| {
                cancel.is_cancelled()
            })
        },
    );
    let imported = summary.imported;
    let skipped = summary.skipped;
    let errors = summary.errors;
    let new_image_ids = summary.image_ids;

    let _ = db.add_library_root(&folder_path);

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
        Some(batch)
    } else {
        None
    };

    let auto_process_ids = filter_image_ids_for_auto_jobs(&db, &new_image_ids)?;
    let cancelled = seal_import_job(&jobs, &job_id, &cancel, summary.cancelled);
    if let Some(batch) = batch_id.as_ref() {
        let _ = db.log_session_event(&NewSessionEvent {
            session_id: session_id.clone(),
            event_type: import_audit_event_type(cancelled).to_string(),
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
                "cancelled": cancelled,
            })
            .to_string(),
        });
    }
    let image_ids_out = new_image_ids.clone();

    if !cancelled && !auto_process_ids.is_empty() {
        run_post_import_quality_analysis(app.clone(), auto_process_ids.clone());
        run_post_import_detection(app.clone(), auto_process_ids);
    }
    let _ = crate::tray::refresh_tray_menu(&app);

    Ok(ImportWorkResult {
        response: ImportResponse {
            imported,
            skipped,
            errors,
            batch_id,
            image_ids: image_ids_out,
            cancelled,
        },
        cancelled,
    })
}

#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    state: State<'_, AppState>,
    file_paths: Vec<String>,
    session_id: Option<String>,
    progress_id: Option<String>,
) -> Result<ImportResponse, String> {
    let db = state.db.clone();
    let app_data_dir = state.app_data_dir.clone();
    let jobs = state.jobs.clone();
    let total = file_paths.len() as u32;
    let (job_id, cancel) = jobs.create_job("import", total);
    emit_import_job_status(&app, &job_id, "running", 0, total);

    let worker_app = app.clone();
    let worker_jobs = jobs.clone();
    let worker_job_id = job_id.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        import_files_blocking(
            worker_app,
            db,
            app_data_dir,
            worker_jobs,
            worker_job_id,
            cancel,
            file_paths,
            session_id,
            progress_id,
        )
    })
    .await;
    let work = match joined {
        Ok(work) => work,
        Err(error) => Err(format!("Import worker failed: {error}")),
    };

    finish_import_job(&app, &jobs, &job_id, &work);
    persist_import_job(state.db.clone(), jobs, job_id).await;
    work.map(|result| result.response)
}

#[allow(clippy::too_many_arguments)]
fn import_files_blocking(
    app: AppHandle,
    db: Database,
    app_data_dir: PathBuf,
    jobs: JobRegistry,
    job_id: String,
    cancel: CancellationToken,
    file_paths: Vec<String>,
    session_id: Option<String>,
    progress_id: Option<String>,
) -> Result<ImportWorkResult, String> {
    let entries: Vec<PathBuf> = file_paths.into_iter().map(PathBuf::from).collect();
    let summary = process_import_entries(
        &entries,
        &cancel,
        |current, total, path| {
            let filename = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let _ = app.emit(
                "import-progress",
                ImportProgress {
                    job_id: job_id.clone(),
                    progress_id: progress_id.clone(),
                    current,
                    total,
                    filename: filename.clone(),
                },
            );
            jobs.update_progress(&job_id, current, Some(&filename));
        },
        |path| {
            crate::db_core::import::import_file_cancellable(&db, path, &app_data_dir, &|| {
                cancel.is_cancelled()
            })
        },
    );
    let imported = summary.imported;
    let skipped = summary.skipped;
    let errors = summary.errors;
    let new_image_ids = summary.image_ids;

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
        Some(batch)
    } else {
        None
    };

    let auto_process_ids = filter_image_ids_for_auto_jobs(&db, &new_image_ids)?;
    let cancelled = seal_import_job(&jobs, &job_id, &cancel, summary.cancelled);
    if let Some(batch) = batch_id.as_ref() {
        let _ = db.log_session_event(&NewSessionEvent {
            session_id: session_id.clone(),
            event_type: import_audit_event_type(cancelled).to_string(),
            actor_type: "user".to_string(),
            actor_id: None,
            subject_type: Some("import_batch".to_string()),
            subject_id: Some(batch.clone()),
            payload_json: serde_json::json!({
                "source": "files",
                "file_count": entries.len(),
                "imported": imported,
                "skipped": skipped,
                "error_count": errors.len(),
                "image_count": new_image_ids.len(),
                "cancelled": cancelled,
            })
            .to_string(),
        });
    }
    let image_ids_out = new_image_ids.clone();

    if !new_image_ids.is_empty() {
        let _ = crate::tray::refresh_tray_menu(&app);
        if !cancelled && !auto_process_ids.is_empty() {
            run_post_import_quality_analysis(app.clone(), auto_process_ids.clone());
            run_post_import_detection(app, auto_process_ids);
        }
    }

    Ok(ImportWorkResult {
        response: ImportResponse {
            imported,
            skipped,
            errors,
            batch_id,
            image_ids: image_ids_out,
            cancelled,
        },
        cancelled,
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
        let blocking_app = app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let state: State<'_, AppState> = blocking_app.state();
            let total = image_ids.len() as u32;
            let (job_id, cancel_token) = state.jobs.create_job("quality", total);
            let progress_job_id = job_id.clone();
            let progress_app = blocking_app.clone();
            let cancel_for_loop = cancel_token.clone();

            let _ = blocking_app.emit(
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
            let _ = blocking_app.emit(
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
            state.jobs.persist_terminal(&job_id, &state.db);
        })
        .await;
        if let Err(error) = result {
            let _ = app.emit(
                "background-task-failed",
                serde_json::json!({
                    "task": "post-import-quality",
                    "message": error.to_string(),
                    "recoverable": true,
                }),
            );
        }
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
        let blocking_app = app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            run_post_import_detection_blocking(blocking_app, image_ids)
        })
        .await;
        if let Err(error) = result {
            let _ = app.emit(
                "background-task-failed",
                serde_json::json!({
                    "task": "post-import-detection",
                    "message": error.to_string(),
                    "recoverable": true,
                }),
            );
        }
    });
}

fn run_post_import_detection_blocking(app: AppHandle, image_ids: Vec<String>) {
    let state: State<'_, AppState> = app.state();
    let yolo_variant = crate::db_core::detection::YoloVariant::Medium;
    let yolo_model_name = yolo_variant.model_name();
    let yolo_available = {
        let engine = state.detection_engine.lock();
        engine.is_variant_available(yolo_variant)
    };
    let nudenet_available = {
        let engine = state.safety_engine.lock();
        engine.is_nudenet_available()
    };
    let stage_size = image_ids.len() as u32;
    let enabled_stages = u32::from(yolo_available) + u32::from(nudenet_available);
    let total = stage_size.saturating_mul(enabled_stages);
    let (job_id, cancel) = state.jobs.create_job("detection", total);
    let _ = app.emit(
        "job-status-changed",
        serde_json::json!({
            "job_id": &job_id,
            "kind": "detection",
            "status": "running",
            "current": 0,
            "total": total,
        }),
    );
    let mut completed_stages = 0u32;

    if yolo_available && !cancel.is_cancelled() {
        let _ = app.emit(
            "auto-detection-start",
            serde_json::json!({
                "job_id": &job_id,
                "model": yolo_model_name,
                "current": completed_stages,
                "total": total,
            }),
        );
        {
            let mut engine = state.detection_engine.lock();
            if engine.session.is_none() {
                let _ = engine.load_yolo(yolo_variant);
            }
        }
        for (index, image_id) in image_ids.iter().enumerate() {
            if cancel.is_cancelled() {
                break;
            }
            if let Ok(images) = state.db.get_images_by_ids(&[image_id.as_str()]) {
                if let Some(image) = images.first() {
                    let path =
                        crate::commands::resolve_image_path_for_ml(image, &state.app_data_dir);
                    let engine = state.detection_engine.lock();
                    if let Ok(detections) = engine.detect(&path) {
                        drop(engine);
                        let _ = state
                            .db
                            .store_detections(image_id, yolo_model_name, &detections);
                    }
                }
            }
            let current = completed_stages + (index + 1) as u32;
            state.jobs.update_progress(&job_id, current, None);
            let _ = app.emit(
                "auto-detection-progress",
                serde_json::json!({
                    "job_id": &job_id,
                    "current": current,
                    "total": total,
                    "model": yolo_model_name,
                }),
            );
        }
        if !cancel.is_cancelled() {
            completed_stages = completed_stages.saturating_add(stage_size);
        }
    }

    if nudenet_available && !cancel.is_cancelled() {
        let _ = app.emit(
            "auto-detection-start",
            serde_json::json!({
                "job_id": &job_id,
                "model": "nudenet",
                "current": completed_stages,
                "total": total,
            }),
        );
        {
            let mut engine = state.safety_engine.lock();
            if engine.session.is_none() {
                let _ = engine.load_nudenet();
            }
        }
        for (index, image_id) in image_ids.iter().enumerate() {
            if cancel.is_cancelled() {
                break;
            }
            if let Ok(images) = state.db.get_images_by_ids(&[image_id.as_str()]) {
                if let Some(image) = images.first() {
                    let path =
                        crate::commands::resolve_image_path_for_ml(image, &state.app_data_dir);
                    let engine = state.safety_engine.lock();
                    if let Ok(detections) = engine.detect(&path) {
                        drop(engine);
                        let _ = state.db.store_detections(image_id, "nudenet", &detections);
                    }
                }
            }
            let current = completed_stages + (index + 1) as u32;
            state.jobs.update_progress(&job_id, current, None);
            let _ = app.emit(
                "auto-detection-progress",
                serde_json::json!({
                    "job_id": &job_id,
                    "current": current,
                    "total": total,
                    "model": "nudenet",
                }),
            );
        }
        if !cancel.is_cancelled() {
            completed_stages = completed_stages.saturating_add(stage_size);
        }
    }

    let status = if cancel.is_cancelled() {
        state.jobs.mark_cancelled(&job_id);
        "cancelled"
    } else {
        state.jobs.complete(&job_id);
        "completed"
    };
    let _ = app.emit(
        "job-status-changed",
        serde_json::json!({
            "job_id": &job_id,
            "kind": "detection",
            "status": status,
            "current": state.jobs.get(&job_id).map(|job| job.current).unwrap_or(0),
            "total": total,
        }),
    );
    let _ = app.emit(
        "auto-detection-complete",
        serde_json::json!({
            "job_id": &job_id,
            "current": state.jobs.get(&job_id).map(|job| job.current).unwrap_or(completed_stages),
            "total": total,
            "cancelled": cancel.is_cancelled(),
        }),
    );
    state.jobs.persist_terminal(&job_id, &state.db);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;

    #[test]
    fn synthetic_10k_import_work_is_bounded_and_cancellable_between_files() {
        let entries: Vec<PathBuf> = (0..10_000)
            .map(|index| PathBuf::from(format!("synthetic-{index}.png")))
            .collect();
        let cancel = CancellationToken::new();
        let cancel_after_progress = cancel.clone();
        let mut max_total = 0;

        let summary = process_import_entries(
            &entries,
            &cancel,
            |current, total, _path| {
                max_total = total;
                if current == 128 {
                    cancel_after_progress.cancel();
                }
            },
            |path| Ok(Some(path.to_string_lossy().to_string())),
        );

        assert_eq!(max_total, 10_000);
        assert_eq!(summary.imported, 128);
        assert!(summary.cancelled);
        assert_eq!(summary.image_ids.len(), 128);
    }

    #[test]
    fn synthetic_10k_folder_discovery_stops_when_cancelled() {
        let cancel = CancellationToken::new();
        let cancel_during_walk = cancel.clone();
        let candidates = (0..10_000).map(move |index| {
            if index == 128 {
                cancel_during_walk.cancel();
            }
            PathBuf::from(format!("synthetic-{index}.png"))
        });

        let discovery = collect_cancellable_entries(candidates, &cancel, |_| true);

        assert!(discovery.cancelled);
        assert_eq!(discovery.entries.len(), 128);
    }

    #[test]
    fn synthetic_10k_import_keeps_browse_and_job_listing_responsive() {
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&tmp.path().join("responsive-import.db")).unwrap();
        let jobs = JobRegistry::default();
        let (job_id, cancel) = jobs.create_job("import", 10_000);
        let entries: Vec<PathBuf> = (0..10_000)
            .map(|index| PathBuf::from(format!("synthetic-{index}.png")))
            .collect();
        let worker_db = db.clone();
        let worker_cancel = cancel.clone();
        let (writer_started_tx, writer_started_rx) = mpsc::channel();
        let (release_writer_tx, release_writer_rx) = mpsc::channel();

        let worker = std::thread::spawn(move || {
            process_import_entries(
                &entries,
                &worker_cancel,
                |_current, _total, _path| {},
                |_path| {
                    let writer = worker_db.conn.lock();
                    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
                    writer_started_tx.send(()).unwrap();
                    release_writer_rx.recv().unwrap();
                    writer.execute_batch("ROLLBACK").unwrap();
                    worker_cancel.cancel();
                    Ok(None)
                },
            )
        });
        writer_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("synthetic import should enter its database stage");

        let started = Instant::now();
        assert_eq!(db.image_count().unwrap(), 0);
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "browse reads must not wait for the import writer"
        );
        let started = Instant::now();
        assert_eq!(jobs.list().first().unwrap().job_id, job_id);
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "list_jobs must not wait for import work"
        );

        release_writer_tx.send(()).unwrap();
        let summary = worker.join().unwrap();
        assert!(summary.cancelled);
    }

    #[test]
    fn tauri_import_commands_dispatch_blocking_work_to_the_blocking_pool() {
        let source = include_str!("import.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(
            production
                .matches("tauri::async_runtime::spawn_blocking")
                .count(),
            5
        );
        assert!(production.contains("process_import_entries("));
        assert!(production.contains("cancel.is_cancelled()"));
    }

    #[test]
    fn import_outcome_is_sealed_only_after_fallible_child_job_preparation() {
        let source = include_str!("import.rs");
        let folder = &source[source.find("fn import_folder_blocking(").unwrap()
            ..source.find("pub async fn import_files(").unwrap()];
        let files = &source[source.find("fn import_files_blocking(").unwrap()
            ..source.find("struct ThumbnailProgress").unwrap()];

        for worker in [folder, files] {
            let prepare = worker.find("filter_image_ids_for_auto_jobs").unwrap();
            let seal = worker.find("seal_import_job").unwrap();
            let audit = worker.find("log_session_event").unwrap();
            assert!(prepare < seal && seal < audit);
        }
    }

    #[test]
    fn child_jobs_emit_terminal_status_before_database_persistence() {
        let source = include_str!("import.rs");
        let quality = &source[source.find("fn run_post_import_quality_analysis").unwrap()
            ..source
                .find("fn analyze_quality_for_imported_images")
                .unwrap()];
        let detection = &source[source
            .find("fn run_post_import_detection_blocking")
            .unwrap()
            ..source.find("#[cfg(test)]\nmod tests").unwrap()];

        for child in [quality, detection] {
            let terminal_event = child.rfind("job-status-changed").unwrap();
            let persistence = child.rfind("persist_terminal").unwrap();
            assert!(terminal_event < persistence);
        }
    }

    #[test]
    fn import_progress_uses_the_registered_job_identity() {
        let payload = serde_json::to_value(ImportProgress {
            job_id: "job_import".to_string(),
            progress_id: Some("sidebar-import".to_string()),
            current: 3,
            total: 10,
            filename: "image.png".to_string(),
        })
        .unwrap();

        assert_eq!(payload["job_id"], "job_import");
        assert_eq!(payload["progress_id"], "sidebar-import");
        assert_eq!(payload["current"], 3);
        assert_eq!(payload["total"], 10);
    }

    #[test]
    fn cancellation_is_sealed_before_child_jobs_and_audit_logging() {
        let jobs = JobRegistry::default();
        let (cancelled_id, cancelled_token) = jobs.create_job("import", 1);
        cancelled_token.cancel();
        assert!(seal_import_job(
            &jobs,
            &cancelled_id,
            &cancelled_token,
            false
        ));
        assert_eq!(jobs.get(&cancelled_id).unwrap().status, "cancelled");
        assert_eq!(import_audit_event_type(true), "import_cancelled");

        let (completed_id, completed_token) = jobs.create_job("import", 1);
        assert!(!seal_import_job(
            &jobs,
            &completed_id,
            &completed_token,
            false
        ));
        assert_eq!(jobs.get(&completed_id).unwrap().status, "completed");
        assert!(jobs.cancel(&completed_id).is_err());
        assert_eq!(import_audit_event_type(false), "import_completed");
    }

    #[test]
    fn post_import_detection_persists_and_emits_canonical_medium_yolo_model_name() {
        let canonical = crate::db_core::detection::YoloVariant::Medium.model_name();
        assert_eq!(canonical, "yolo11m");

        let source = include_str!("import.rs");
        let legacy = ["yolo", "v8m"].concat();
        assert!(!source.contains(&format!("\"{legacy}\"")));
        let compact = source.split_whitespace().collect::<String>();
        let store_call = [
            "store_detections(",
            "image_id,",
            "yolo_model_name,",
            "&detections",
        ]
        .concat();
        let event_model = ["\"model\":", "yolo_model_name"].concat();
        assert!(compact.contains(&store_call));
        assert_eq!(compact.matches(&event_model).count(), 2);
    }

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
