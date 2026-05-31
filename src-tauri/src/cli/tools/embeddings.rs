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
        "expected_sha256": spec.expected_sha256,
        "expected_size_bytes": spec.expected_size_bytes,
        "spdx_license": spec.spdx_license,
        "source_repo": spec.source_repo,
        "model_card_url": spec.model_card_url,
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
    let verification = spec.download_verification();
    let outcome = runtime.block_on(
        crate::services::model_download::download_model_file_verified_controlled(
            &client,
            spec.url,
            &model_path,
            Some(&verification),
            &crate::services::model_download::DownloadControl::default(),
            |_progress| {},
        ),
    )?;

    Ok(serde_json::json!({
        "status": if outcome.resumed { "resumed" } else { "downloaded" },
        "model": spec.model_id,
        "model_path": model_path,
        "downloaded": outcome.downloaded,
        "expected_sha256": spec.expected_sha256,
        "expected_size_bytes": spec.expected_size_bytes,
        "spdx_license": spec.spdx_license,
        "source_repo": spec.source_repo,
        "model_card_url": spec.model_card_url,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::embeddings::CLIP_MODEL_SPEC;

    fn test_context() -> (tempfile::TempDir, HeadlessContext) {
        let tmp = tempfile::tempdir().unwrap();
        let app_data_dir = tmp.path().join("app-data");
        std::fs::create_dir_all(&app_data_dir).unwrap();
        let db = Database::open(&app_data_dir.join("cull.db")).unwrap();
        (tmp, HeadlessContext { db, app_data_dir })
    }

    #[test]
    fn download_info_includes_checksum_size_and_provenance() {
        let (_tmp, ctx) = test_context();

        let info =
            get_embedding_model_download_info(&ctx, serde_json::json!({ "model": "clip-vit-b32" }))
                .unwrap();

        assert_eq!(info["model_id"], CLIP_MODEL_SPEC.model_id);
        assert_eq!(info["url"], CLIP_MODEL_SPEC.url);
        assert_eq!(info["expected_sha256"], CLIP_MODEL_SPEC.expected_sha256);
        assert_eq!(
            info["expected_size_bytes"],
            CLIP_MODEL_SPEC.expected_size_bytes
        );
        assert_eq!(info["spdx_license"], CLIP_MODEL_SPEC.spdx_license);
        assert_eq!(info["source_repo"], CLIP_MODEL_SPEC.source_repo);
        assert_eq!(info["model_card_url"], CLIP_MODEL_SPEC.model_card_url);
    }

    #[test]
    fn headless_cli_download_uses_verified_downloader() {
        let source = include_str!("embeddings.rs");
        let download_body = source
            .split("pub fn download_embedding_model")
            .nth(1)
            .and_then(|rest| rest.split("pub fn generate_embeddings").next())
            .expect("download_embedding_model should be followed by generate_embeddings");

        assert!(
            download_body.contains("download_model_file_verified_controlled"),
            "{download_body}"
        );
        assert!(
            !download_body.contains("download_model_file("),
            "{download_body}"
        );
    }
}
