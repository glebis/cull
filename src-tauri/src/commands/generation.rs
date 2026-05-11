use tauri::State;
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::services::generation;

#[derive(Debug, Deserialize)]
pub struct ResubmitRequest {
    pub source_image_id: Option<String>,
    pub prompt: String,
    pub n: u8,
    pub model: String,
    pub size: String,
    pub quality: String,
}

#[derive(Debug, Serialize)]
pub struct ResubmitResponse {
    pub job_id: String,
}

#[derive(Debug, Serialize)]
pub struct CostEstimate {
    pub estimated_cost: f64,
    pub model: String,
    pub size: String,
    pub quality: String,
    pub n: u8,
}

#[tauri::command]
pub async fn resubmit_prompt(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ResubmitRequest,
) -> Result<ResubmitResponse, String> {
    let api_key = state.secrets.get("api_key_openai")?
        .ok_or("OpenAI API key not set. Go to Settings to add it.")?;

    if request.prompt.trim().is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }
    if request.n < 1 || request.n > 4 {
        return Err("n must be between 1 and 4".to_string());
    }

    let gen_request = generation::GenerationRequest {
        source_image_id: request.source_image_id,
        prompt: request.prompt,
        n: request.n,
        model: request.model,
        size: request.size,
        quality: request.quality,
    };

    let db = state.db.clone();
    let jobs = state.jobs.clone();
    let app_data_dir = state.app_data_dir.clone();
    let app_clone = app.clone();

    let (job_id, cancel) = state.jobs.create_job("generation", gen_request.n as u32);
    let job_id_for_task = job_id.clone();

    tokio::spawn(async move {
        let _ = generation::generate_images(
            &gen_request,
            &api_key,
            &app_data_dir,
            &db,
            &jobs,
            &job_id_for_task,
            &cancel,
            &app_clone,
        ).await;
    });

    Ok(ResubmitResponse { job_id })
}

#[tauri::command]
pub async fn estimate_generation_cost(
    model: String,
    size: String,
    quality: String,
    n: u8,
) -> Result<CostEstimate, String> {
    Ok(CostEstimate {
        estimated_cost: generation::estimate_cost(&model, &size, &quality, n),
        model,
        size,
        quality,
        n,
    })
}
