use serde::Deserialize;
use serde_json::Value;

use crate::db_core::embeddings::{embedding_model_spec, EmbeddingEngine};
use crate::services::model_pipeline::{run_embedding_model, EmbeddingRunRequest};

use super::HeadlessContext;

#[derive(Debug, Deserialize)]
struct EmbeddingModelParams {
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenerateEmbeddingsParams {
    image_ids: Vec<String>,
    model: Option<String>,
}

pub fn get_embedding_model_download_info(
    ctx: &HeadlessContext,
    params: Value,
) -> Result<Value, String> {
    let parsed: EmbeddingModelParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid get_embedding_model_download_info params: {}", e))?;
    let model = parsed.model.unwrap_or_else(|| "clip-vit-b32".to_string());
    let spec = embedding_model_spec(&model)
        .ok_or_else(|| format!("Unsupported embedding model '{}'", model))?;
    let engine = EmbeddingEngine::new(&ctx.app_data_dir.join("models"));
    let model_path = engine.model_path_for(spec.model_id)?;
    let part_path = crate::services::model_download::part_path_for(&model_path);

    Ok(serde_json::json!({
        "model_id": spec.model_id,
        "url": spec.url,
        "model_path": model_path,
        "part_path": part_path,
        "available": model_path.exists(),
    }))
}

pub fn download_embedding_model(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: EmbeddingModelParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid download_embedding_model params: {}", e))?;
    let model = parsed.model.unwrap_or_else(|| "clip-vit-b32".to_string());
    let spec = embedding_model_spec(&model)
        .ok_or_else(|| format!("Unsupported embedding model '{}'", model))?;
    let engine = EmbeddingEngine::new(&ctx.app_data_dir.join("models"));
    let model_path = engine.model_path_for(spec.model_id)?;
    if model_path.exists() {
        return Ok(serde_json::json!({
            "status": "already_downloaded",
            "model": spec.model_id,
            "model_path": model_path,
        }));
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create runtime: {}", e))?;
    let client = reqwest::Client::new();
    let outcome = runtime.block_on(crate::services::model_download::download_model_file(
        &client,
        spec.url,
        &model_path,
        |_progress| {},
    ))?;

    Ok(serde_json::json!({
        "status": if outcome.resumed { "resumed" } else { "downloaded" },
        "model": spec.model_id,
        "model_path": model_path,
        "downloaded": outcome.downloaded,
    }))
}

pub fn generate_embeddings(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: GenerateEmbeddingsParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid generate_embeddings params: {}", e))?;
    let model = parsed.model.unwrap_or_else(|| "clip-vit-b32".to_string());
    if parsed.image_ids.is_empty() {
        return Err("generate_embeddings requires at least one image_id".to_string());
    }

    let embedding_engine =
        parking_lot::Mutex::new(EmbeddingEngine::new(&ctx.app_data_dir.join("models")));
    crate::services::model_pipeline::ensure_embedding_model_loaded(&embedding_engine, &model)?;
    let result = run_embedding_model(EmbeddingRunRequest {
        db: &ctx.db,
        app_data_dir: &ctx.app_data_dir,
        embedding_engine: &embedding_engine,
        jobs: None,
        job_id: None,
        cancel: None,
        app: None,
        model_id: &model,
        image_ids: &parsed.image_ids,
    })?;

    serde_json::to_value(result).map_err(|e| e.to_string())
}
