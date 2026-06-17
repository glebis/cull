use crate::db_core::db::Database;
use crate::db_core::models::{ImageWithFile, NewModelRun, NewModelRunItem};
use crate::services::jobs::JobRegistry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

const OCR_SOURCE: &str = "ocr";
const OCR_MODEL_ID: &str = "apple-vision-text-recognition";

#[derive(Debug, Clone, Deserialize)]
pub struct OcrBatchRequest {
    pub image_ids: Vec<String>,
    pub skip_existing: bool,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct OcrBatchResult {
    pub model_run_id: String,
    pub processed: u32,
    pub skipped: u32,
    pub failed: u32,
    pub total: u32,
    pub status: String,
}

pub struct OcrRunContext<'a> {
    pub db: &'a Database,
    pub app_data_dir: &'a PathBuf,
    pub jobs: &'a JobRegistry,
    pub job_id: &'a str,
    pub cancel: &'a CancellationToken,
    pub app: &'a AppHandle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrResult {
    pub lines: Vec<OcrLine>,
}

impl OcrResult {
    fn full_text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.text.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn average_confidence(&self) -> Option<f32> {
        if self.lines.is_empty() {
            return None;
        }
        Some(self.lines.iter().map(|line| line.confidence).sum::<f32>() / self.lines.len() as f32)
    }
}

pub fn run_ocr_batch(
    request: OcrBatchRequest,
    ctx: OcrRunContext<'_>,
) -> Result<OcrBatchResult, String> {
    let total = request.image_ids.len() as u32;
    let model_run_id = insert_run(&request, &ctx)?;
    emit_model_run_status(ctx.app, &model_run_id, ctx.job_id, "running", 0, total);

    let mut processed = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;

    for (i, image_id) in request.image_ids.iter().enumerate() {
        if ctx.cancel.is_cancelled() {
            let summary = serde_json::json!({
                "processed": processed,
                "skipped": skipped,
                "failed": failed,
                "total": total,
                "cancelled_at": i,
            })
            .to_string();
            ctx.db
                .update_model_run_terminal(&model_run_id, "cancelled", &summary, None)
                .map_err(|e| e.to_string())?;
            ctx.jobs.mark_cancelled(ctx.job_id);
            emit_model_run_status(
                ctx.app,
                &model_run_id,
                ctx.job_id,
                "cancelled",
                i as u32,
                total,
            );
            return Ok(OcrBatchResult {
                model_run_id,
                processed,
                skipped,
                failed,
                total,
                status: "cancelled".to_string(),
            });
        }

        let current = (i + 1) as u32;
        let image = match load_image(ctx.db, image_id) {
            Ok(Some(image)) => image,
            Ok(None) => {
                failed += 1;
                insert_item(
                    ctx.db,
                    &model_run_id,
                    None,
                    image_id,
                    None,
                    "failed",
                    None,
                    None,
                    Some("Image not found"),
                )?;
                update_progress(&ctx, &model_run_id, current, total, image_id);
                continue;
            }
            Err(error) => {
                failed += 1;
                insert_item(
                    ctx.db,
                    &model_run_id,
                    None,
                    image_id,
                    None,
                    "failed",
                    None,
                    None,
                    Some(&error),
                )?;
                update_progress(&ctx, &model_run_id, current, total, image_id);
                continue;
            }
        };

        if request.skip_existing
            && !request.overwrite
            && ctx
                .db
                .image_has_metadata_source(image_id, OCR_SOURCE)
                .map_err(|e| e.to_string())?
        {
            skipped += 1;
            let audit = serde_json::json!({ "reason": "ocr source already exists" }).to_string();
            insert_item(
                ctx.db,
                &model_run_id,
                Some(image_id),
                image_id,
                Some(&image.image.sha256_hash),
                "skipped",
                None,
                Some(audit),
                None,
            )?;
            update_progress(&ctx, &model_run_id, current, total, image_id);
            continue;
        }

        let image_path = crate::commands::resolve_image_path_for_ml(&image, ctx.app_data_dir);
        match recognize_text(&image_path) {
            Ok(ocr) => {
                if request.overwrite {
                    ctx.db
                        .delete_image_metadata_source(image_id, OCR_SOURCE)
                        .map_err(|e| e.to_string())?;
                }
                let fields = fields_from_ocr(&ocr);
                ctx.db
                    .store_vision_metadata(image_id, OCR_SOURCE, &fields)
                    .map_err(|e| e.to_string())?;

                let full_text = ocr.full_text();
                let audit = serde_json::json!({
                    "line_count": ocr.lines.len(),
                    "char_count": full_text.chars().count(),
                    "average_confidence": ocr.average_confidence(),
                    "engine": OCR_MODEL_ID,
                })
                .to_string();
                insert_item(
                    ctx.db,
                    &model_run_id,
                    Some(image_id),
                    image_id,
                    Some(&image.image.sha256_hash),
                    "completed",
                    Some(OCR_SOURCE),
                    Some(audit),
                    None,
                )?;
                processed += 1;
            }
            Err(error) => {
                failed += 1;
                insert_item(
                    ctx.db,
                    &model_run_id,
                    Some(image_id),
                    image_id,
                    Some(&image.image.sha256_hash),
                    "failed",
                    None,
                    None,
                    Some(&error),
                )?;
                eprintln!("OCR error for {}: {}", image_id, error);
            }
        }

        update_progress(&ctx, &model_run_id, current, total, image_id);
    }

    let status = if failed > 0 && processed == 0 && skipped == 0 {
        "failed"
    } else {
        "completed"
    };
    let summary = serde_json::json!({
        "processed": processed,
        "skipped": skipped,
        "failed": failed,
        "total": total,
        "source_label": OCR_SOURCE,
    })
    .to_string();
    ctx.db
        .update_model_run_terminal(
            &model_run_id,
            status,
            &summary,
            (status == "failed").then_some("All OCR items failed"),
        )
        .map_err(|e| e.to_string())?;
    if status == "failed" {
        ctx.jobs.fail(ctx.job_id, "All OCR items failed");
    } else {
        ctx.jobs.complete(ctx.job_id);
    }
    emit_model_run_status(ctx.app, &model_run_id, ctx.job_id, status, total, total);
    let _ = ctx.app.emit(
        "ocr-complete",
        serde_json::json!({
            "job_id": ctx.job_id,
            "model_run_id": model_run_id,
            "processed": processed,
            "skipped": skipped,
            "failed": failed,
            "total": total,
        }),
    );

    Ok(OcrBatchResult {
        model_run_id,
        processed,
        skipped,
        failed,
        total,
        status: status.to_string(),
    })
}

fn fields_from_ocr(ocr: &OcrResult) -> HashMap<String, String> {
    let full_text = ocr.full_text();
    let mut fields = HashMap::new();
    fields.insert(
        "ocr_status".to_string(),
        if full_text.is_empty() {
            "no_text"
        } else {
            "recognized"
        }
        .to_string(),
    );
    fields.insert("line_count".to_string(), ocr.lines.len().to_string());
    fields.insert("ocr_engine".to_string(), OCR_MODEL_ID.to_string());
    if let Some(confidence) = ocr.average_confidence() {
        fields.insert(
            "average_confidence".to_string(),
            format!("{:.3}", confidence),
        );
    }
    if !full_text.is_empty() {
        fields.insert("ocr_text".to_string(), full_text);
    }
    fields
}

fn insert_run(request: &OcrBatchRequest, ctx: &OcrRunContext<'_>) -> Result<String, String> {
    let model_run_id = format!("mr_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let now = chrono::Utc::now().to_rfc3339();
    let input_scope_json = serde_json::json!({
        "type": "image_ids",
        "image_ids": request.image_ids,
    })
    .to_string();
    let params_json = serde_json::json!({
        "source_label": OCR_SOURCE,
        "skip_existing": request.skip_existing,
        "overwrite": request.overwrite,
    })
    .to_string();

    ctx.db
        .insert_model_run(&NewModelRun {
            id: model_run_id.clone(),
            job_id: Some(ctx.job_id.to_string()),
            parent_run_id: None,
            profile_id: None,
            task: "ocr".to_string(),
            provider: "local".to_string(),
            model_id: OCR_MODEL_ID.to_string(),
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

    Ok(model_run_id)
}

fn load_image(db: &Database, image_id: &str) -> Result<Option<ImageWithFile>, String> {
    let id_refs = vec![image_id];
    let images = db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
    Ok(images.into_iter().next())
}

#[expect(
    clippy::too_many_arguments,
    reason = "OCR item inserts mirror the model_run_items schema columns"
)]
fn insert_item(
    db: &Database,
    model_run_id: &str,
    image_id: Option<&str>,
    input_asset_image_id: &str,
    input_hash: Option<&str>,
    status: &str,
    output_ref_id: Option<&str>,
    audit_payload_json: Option<String>,
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
        output_ref_kind: output_ref_id.map(|_| "image_metadata_source".to_string()),
        output_ref_id: output_ref_id.map(|id| id.to_string()),
        audit_payload_json,
        cost_usd: None,
        attempt_count: 1,
        error: error.map(|e| e.to_string()),
        started_at: Some(now.clone()),
        completed_at: Some(now),
    })
    .map_err(|e| e.to_string())
}

fn update_progress(
    ctx: &OcrRunContext<'_>,
    model_run_id: &str,
    current: u32,
    total: u32,
    image_id: &str,
) {
    ctx.jobs
        .update_progress(ctx.job_id, current, Some(image_id));
    let _ = ctx.app.emit(
        "ocr-progress",
        serde_json::json!({
            "job_id": ctx.job_id,
            "current": current,
            "total": total,
            "model": OCR_MODEL_ID,
            "model_run_id": model_run_id,
        }),
    );
    emit_model_run_status(ctx.app, model_run_id, ctx.job_id, "running", current, total);
}

fn emit_model_run_status(
    app: &AppHandle,
    model_run_id: &str,
    job_id: &str,
    status: &str,
    current: u32,
    total: u32,
) {
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
    let _ = app.emit(
        "job-status-changed",
        serde_json::json!({
            "job_id": job_id,
            "kind": "ocr",
            "status": status,
            "current": current,
            "total": total,
        }),
    );
}

#[cfg(target_os = "macos")]
pub fn recognize_text(path: &std::path::Path) -> Result<OcrResult, String> {
    macos_vision::recognize_text(path)
}

#[cfg(not(target_os = "macos"))]
pub fn recognize_text(_path: &std::path::Path) -> Result<OcrResult, String> {
    Err("OCR is currently available only on macOS via Apple Vision".to_string())
}

#[cfg(target_os = "macos")]
mod macos_vision {
    use super::{OcrLine, OcrResult};
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::AnyObject;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSError};
    use objc2_vision::{
        VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
        VNRequestTextRecognitionLevel,
    };
    use std::path::Path;

    pub fn recognize_text(path: &Path) -> Result<OcrResult, String> {
        let data = std::fs::read(path).map_err(|e| format!("Read error: {}", e))?;
        autoreleasepool(|_| recognize_from_bytes(&data))
    }

    fn recognize_from_bytes(data: &[u8]) -> Result<OcrResult, String> {
        let image_data = NSData::with_bytes(data);
        let options = NSDictionary::<VNImageOption, AnyObject>::new();
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &image_data,
            &options,
        );
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        request.setAutomaticallyDetectsLanguage(true);

        let request_as_base: Retained<VNRequest> = Retained::from(&*request);
        let requests = NSArray::from_slice(&[&*request_as_base]);
        handler
            .performRequests_error(&requests)
            .map_err(|e| ns_error_to_string(&e))?;

        let Some(observations) = request.results() else {
            return Ok(OcrResult { lines: Vec::new() });
        };

        let mut lines = Vec::with_capacity(observations.len());
        for index in 0..observations.len() {
            let observation = observations.objectAtIndex(index);
            let candidates = observation.topCandidates(1);
            if candidates.is_empty() {
                continue;
            }
            let candidate = candidates.objectAtIndex(0);
            let text = candidate.string().to_string();
            let text = text.trim().to_string();
            if text.is_empty() {
                continue;
            }
            lines.push(OcrLine {
                text,
                confidence: candidate.confidence(),
            });
        }

        Ok(OcrResult { lines })
    }

    fn ns_error_to_string(error: &NSError) -> String {
        error.localizedDescription().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_result_full_text_joins_non_empty_lines() {
        let ocr = OcrResult {
            lines: vec![
                OcrLine {
                    text: " First ".to_string(),
                    confidence: 0.9,
                },
                OcrLine {
                    text: "".to_string(),
                    confidence: 0.7,
                },
                OcrLine {
                    text: "Second".to_string(),
                    confidence: 0.8,
                },
            ],
        };
        assert_eq!(ocr.full_text(), "First\nSecond");
        let confidence = ocr.average_confidence().unwrap();
        assert!((confidence - 0.8).abs() < 0.0001);
    }

    #[test]
    fn fields_mark_empty_ocr_as_processed() {
        let fields = fields_from_ocr(&OcrResult { lines: Vec::new() });
        assert_eq!(fields.get("ocr_status"), Some(&"no_text".to_string()));
        assert_eq!(fields.get("line_count"), Some(&"0".to_string()));
        assert!(!fields.contains_key("ocr_text"));
    }
}
