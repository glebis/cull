use crate::db_core::db::Database;
use crate::db_core::embeddings::EmbeddingEngine;
use crate::db_core::models::{ImageWithFile, NewModelRun, NewModelRunItem};
use crate::services::jobs::JobRegistry;
use parking_lot::Mutex;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

const CLIP_MODEL_ID: &str = "clip-vit-b32";

pub struct ClipEmbeddingRunRequest<'a> {
    pub db: &'a Database,
    pub app_data_dir: &'a PathBuf,
    pub embedding_engine: &'a Mutex<EmbeddingEngine>,
    pub jobs: Option<&'a JobRegistry>,
    pub job_id: Option<&'a str>,
    pub cancel: Option<&'a CancellationToken>,
    pub app: Option<&'a AppHandle>,
    pub image_ids: &'a [String],
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipEmbeddingRunResult {
    pub model_run_id: String,
    pub generated: u32,
    pub failed: u32,
    pub total: u32,
    pub status: String,
}

pub fn ensure_clip_model_loaded(embedding_engine: &Mutex<EmbeddingEngine>) -> Result<(), String> {
    let mut engine = embedding_engine.lock();
    if engine.session.is_none() {
        if !engine.is_model_available() {
            return Err("Model not downloaded. Run download_clip_model first.".to_string());
        }
        engine.load_model()?;
    }
    Ok(())
}

pub fn run_clip_embeddings(
    request: ClipEmbeddingRunRequest<'_>,
) -> Result<ClipEmbeddingRunResult, String> {
    let total = request.image_ids.len() as u32;
    let model_run_id = format!("mr_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let now = chrono::Utc::now().to_rfc3339();
    let input_scope_json = serde_json::json!({
        "type": "image_ids",
        "image_ids": request.image_ids,
    })
    .to_string();
    let params_json = serde_json::json!({
        "runtime": "onnx",
        "source": "hf_hub",
        "projection": "embeddings",
        "normalized": true,
    })
    .to_string();

    request
        .db
        .insert_model_run(&NewModelRun {
            id: model_run_id.clone(),
            job_id: request.job_id.map(|id| id.to_string()),
            parent_run_id: None,
            profile_id: None,
            task: "embedding".to_string(),
            provider: "local".to_string(),
            model_id: CLIP_MODEL_ID.to_string(),
            model_revision: None,
            status: "running".to_string(),
            input_scope_json,
            params_json,
            output_summary_json: "{}".to_string(),
            cost_estimate_usd: None,
            cost_actual_usd: None,
            error: None,
            created_at: now.clone(),
            started_at: Some(now),
            completed_at: None,
        })
        .map_err(|e| e.to_string())?;

    emit_model_run_event(
        request.app,
        &model_run_id,
        request.job_id,
        "running",
        0,
        total,
    );

    let mut generated = 0u32;
    let mut failed = 0u32;

    for (i, image_id) in request.image_ids.iter().enumerate() {
        if request.cancel.map(|c| c.is_cancelled()).unwrap_or(false) {
            let summary = serde_json::json!({
                "generated": generated,
                "failed": failed,
                "total": total,
                "cancelled_at": i,
            })
            .to_string();
            request
                .db
                .update_model_run_terminal(&model_run_id, "cancelled", &summary, None)
                .map_err(|e| e.to_string())?;
            emit_model_run_event(
                request.app,
                &model_run_id,
                request.job_id,
                "cancelled",
                i as u32,
                total,
            );
            return Ok(ClipEmbeddingRunResult {
                model_run_id,
                generated,
                failed,
                total,
                status: "cancelled".to_string(),
            });
        }

        let current = (i + 1) as u32;
        let image = match load_image(request.db, image_id) {
            Ok(Some(image)) => image,
            Ok(None) => {
                failed += 1;
                insert_embedding_item(
                    request.db,
                    &model_run_id,
                    None,
                    image_id,
                    None,
                    "failed",
                    None,
                    Some("Image not found"),
                )
                .map_err(|e| fail_run(request.db, &model_run_id, e))?;
                update_progress(&request, &model_run_id, current, total, image_id);
                continue;
            }
            Err(e) => {
                failed += 1;
                insert_embedding_item(
                    request.db,
                    &model_run_id,
                    None,
                    image_id,
                    None,
                    "failed",
                    None,
                    Some(&e),
                )
                .map_err(|e| fail_run(request.db, &model_run_id, e))?;
                update_progress(&request, &model_run_id, current, total, image_id);
                continue;
            }
        };

        let ml_path = resolve_image_path_for_ml(&image, request.app_data_dir);
        let embedding = {
            let engine = request.embedding_engine.lock();
            engine.generate_embedding(&ml_path)
        };

        match embedding {
            Ok(vector) => {
                let embedding_id = request
                    .db
                    .store_embedding_with_model_run(
                        image_id,
                        CLIP_MODEL_ID,
                        &vector,
                        Some(&model_run_id),
                    )
                    .map_err(|e| fail_run(request.db, &model_run_id, e.to_string()))?;
                insert_embedding_item(
                    request.db,
                    &model_run_id,
                    Some(image_id),
                    image_id,
                    Some(&image.image.sha256_hash),
                    "completed",
                    Some(&embedding_id),
                    None,
                )
                .map_err(|e| fail_run(request.db, &model_run_id, e))?;
                generated += 1;
            }
            Err(e) => {
                failed += 1;
                insert_embedding_item(
                    request.db,
                    &model_run_id,
                    Some(image_id),
                    image_id,
                    Some(&image.image.sha256_hash),
                    "failed",
                    None,
                    Some(&e),
                )
                .map_err(|err| fail_run(request.db, &model_run_id, err))?;
                eprintln!("Embedding error for {}: {}", image_id, e);
            }
        }

        update_progress(&request, &model_run_id, current, total, image_id);
    }

    let status = "completed";
    let summary = serde_json::json!({
        "generated": generated,
        "failed": failed,
        "total": total,
    })
    .to_string();
    request
        .db
        .update_model_run_terminal(&model_run_id, status, &summary, None)
        .map_err(|e| e.to_string())?;
    emit_model_run_event(
        request.app,
        &model_run_id,
        request.job_id,
        status,
        total,
        total,
    );

    Ok(ClipEmbeddingRunResult {
        model_run_id,
        generated,
        failed,
        total,
        status: status.to_string(),
    })
}

fn load_image(db: &Database, image_id: &str) -> Result<Option<ImageWithFile>, String> {
    let id_refs = vec![image_id];
    let images = db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
    Ok(images.into_iter().next())
}

fn insert_embedding_item(
    db: &Database,
    model_run_id: &str,
    image_id: Option<&str>,
    input_asset_image_id: &str,
    input_hash: Option<&str>,
    status: &str,
    output_ref_id: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    db.insert_model_run_item(&NewModelRunItem {
        id: format!("mri_{}", uuid::Uuid::new_v4().to_string().replace('-', "")),
        run_id: model_run_id.to_string(),
        image_id: image_id.map(|id| id.to_string()),
        input_asset_uri: format!("cull://images/{}/ml-input", input_asset_image_id),
        input_hash: input_hash.map(|h| h.to_string()),
        status: status.to_string(),
        output_ref_kind: output_ref_id.map(|_| "embedding".to_string()),
        output_ref_id: output_ref_id.map(|id| id.to_string()),
        audit_payload_json: None,
        cost_usd: None,
        attempt_count: 1,
        error: error.map(|e| e.to_string()),
        started_at: Some(now.clone()),
        completed_at: Some(now),
    })
    .map_err(|e| e.to_string())
}

fn update_progress(
    request: &ClipEmbeddingRunRequest<'_>,
    model_run_id: &str,
    current: u32,
    total: u32,
    image_id: &str,
) {
    if let (Some(jobs), Some(job_id)) = (request.jobs, request.job_id) {
        jobs.update_progress(job_id, current, Some(image_id));
    }
    if let Some(app) = request.app {
        let _ = app.emit(
            "embedding-progress",
            serde_json::json!({
                "current": current,
                "total": total,
                "model": CLIP_MODEL_ID,
                "model_run_id": model_run_id,
            }),
        );
    }
}

fn emit_model_run_event(
    app: Option<&AppHandle>,
    model_run_id: &str,
    job_id: Option<&str>,
    status: &str,
    current: u32,
    total: u32,
) {
    if let Some(app) = app {
        let _ = app.emit(
            "model-run-status-changed",
            serde_json::json!({
                "model_run_id": model_run_id,
                "job_id": job_id,
                "status": status,
                "current": current,
                "total": total,
            }),
        );
    }
}

fn fail_run(db: &Database, model_run_id: &str, error: String) -> String {
    let summary = serde_json::json!({ "error": &error }).to_string();
    let _ = db.update_model_run_terminal(model_run_id, "failed", &summary, Some(&error));
    error
}

fn resolve_image_path_for_ml(img: &ImageWithFile, app_data_dir: &Path) -> PathBuf {
    let ext = Path::new(&img.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if crate::extensions::is_raw_extension(ext) {
        crate::db_core::thumbnails::thumbnail_path(app_data_dir, &img.image.id)
    } else {
        PathBuf::from(&img.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;

    #[test]
    fn test_clip_pipeline_records_failed_item_for_missing_image() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let app_data_dir = tmp.path().to_path_buf();
        let model_dir = tmp.path().join("models");
        let embedding_engine = Mutex::new(EmbeddingEngine::new(&model_dir));
        let image_ids = vec!["missing-image".to_string()];

        let result = run_clip_embeddings(ClipEmbeddingRunRequest {
            db: &db,
            app_data_dir: &app_data_dir,
            embedding_engine: &embedding_engine,
            jobs: None,
            job_id: None,
            cancel: None,
            app: None,
            image_ids: &image_ids,
        })
        .unwrap();

        assert_eq!(result.generated, 0);
        assert_eq!(result.failed, 1);
        assert_eq!(result.status, "completed");

        let run = db.get_model_run(&result.model_run_id).unwrap().unwrap();
        assert_eq!(run.status, "completed");
        assert!(run.output_summary_json.contains("\"failed\":1"));

        let conn = db.conn.lock();
        let failed_items: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM model_run_items WHERE run_id = ?1 AND status = 'failed'",
                rusqlite::params![result.model_run_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(failed_items, 1);
    }
}
