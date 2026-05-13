use crate::services::generation;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct ResubmitRequest {
    pub provider: String,
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
    pub provider: String,
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
    let provider_cfg = generation::provider_config(&request.provider)?;
    let api_key = state.secrets.get(provider_cfg.key_name)?.ok_or_else(|| {
        format!(
            "{} API key not set. Go to Settings to add it.",
            request.provider
        )
    })?;
    let base_url = provider_cfg.base_url.to_string();
    let api_style = provider_cfg.api_style;

    if request.prompt.trim().is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }
    if request.n < 1 || request.n > 4 {
        return Err("n must be between 1 and 4".to_string());
    }

    let gen_request = generation::GenerationRequest {
        provider: request.provider,
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
            &base_url,
            api_style,
            &app_data_dir,
            &db,
            &jobs,
            &job_id_for_task,
            &cancel,
            &app_clone,
        )
        .await;
    });

    Ok(ResubmitResponse { job_id })
}

#[tauri::command]
pub async fn estimate_generation_cost(
    provider: String,
    model: String,
    size: String,
    quality: String,
    n: u8,
) -> Result<CostEstimate, String> {
    Ok(CostEstimate {
        estimated_cost: generation::estimate_cost(&provider, &model, &size, &quality, n),
        provider,
        model,
        size,
        quality,
        n,
    })
}
