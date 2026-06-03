use crate::services::ocr::{OcrBatchRequest, OcrBatchResult};
use crate::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Serialize)]
pub struct OcrBatchStartResponse {
    pub job_id: String,
}

#[tauri::command]
pub async fn start_ocr_batch(
    app: AppHandle,
    state: State<'_, AppState>,
    request: OcrBatchRequest,
) -> Result<OcrBatchStartResponse, String> {
    if request.image_ids.is_empty() {
        return Err("No images selected".to_string());
    }

    let db = state.db.clone();
    let jobs = state.jobs.clone();
    let app_data_dir = state.app_data_dir.clone();
    let app_clone = app.clone();
    let (job_id, cancel) = state.jobs.create_job("ocr", request.image_ids.len() as u32);
    let job_id_for_task = job_id.clone();

    crate::spawn_guarded(app.clone(), "ocr-batch", move || async move {
        let result: Result<OcrBatchResult, String> = crate::services::ocr::run_ocr_batch(
            request,
            crate::services::ocr::OcrRunContext {
                db: &db,
                app_data_dir: &app_data_dir,
                jobs: &jobs,
                job_id: &job_id_for_task,
                cancel: &cancel,
                app: &app_clone,
            },
        );

        if let Err(error) = result {
            jobs.fail(&job_id_for_task, &error);
            let _ = app_clone.emit(
                "job-status-changed",
                serde_json::json!({
                    "job_id": job_id_for_task,
                    "kind": "ocr",
                    "status": "failed",
                    "current": 0,
                    "total": 0,
                    "error": error,
                }),
            );
        }
        jobs.persist_terminal(&job_id_for_task, &db);
    });

    Ok(OcrBatchStartResponse { job_id })
}
