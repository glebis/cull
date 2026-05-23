use crate::db_core::gemini::GeminiEmbeddingProvider;
use crate::AppState;
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

use crate::db_core::embeddings::{
    embedding_model_spec, embedding_provider_for_model, embedding_provider_specs, EmbeddingEngine,
    CLIP_MODEL_ID, GEMINI_EMBEDDING_MODEL_ID,
};
use crate::db_core::remote_embeddings::{
    build_text_embedding_input, check_ollama_embedding_available, cohere_storage_model_id,
    normalize_embedding, ollama_storage_model_id, openai_storage_model_id,
    CohereImageEmbeddingProvider, OllamaTextEmbeddingProvider, OpenAiTextEmbeddingProvider,
    COHERE_EMBEDDING_ENDPOINT, COHERE_EMBEDDING_MODEL, OLLAMA_EMBEDDING_MODEL,
    OLLAMA_EMBEDDING_URL, OPENAI_EMBEDDING_ENDPOINT, OPENAI_EMBEDDING_MODEL,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingProviderInfo {
    pub id: String,
    pub label: String,
    pub short_label: String,
    pub model_name: String,
    pub dimensions: usize,
    pub dimensions_label: String,
    pub scope: String,
    pub runtime: String,
    pub status: String,
    pub available: bool,
    pub downloadable: bool,
    pub download_label: Option<String>,
    pub api_key_provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddingModelDownloadInfo {
    pub model_id: String,
    pub url: String,
    pub model_path: String,
    pub part_path: String,
    pub curl_command: String,
}

pub type ClipModelDownloadInfo = EmbeddingModelDownloadInfo;

#[derive(Debug, Clone, Copy, Default)]
struct ProviderAvailability {
    google_key_exists: bool,
    cohere_key_exists: bool,
    openai_key_exists: bool,
    ollama_available: bool,
}

fn embedding_provider_infos(
    engine: &EmbeddingEngine,
    availability: ProviderAvailability,
) -> Vec<EmbeddingProviderInfo> {
    embedding_provider_specs()
        .iter()
        .map(|spec| {
            let (scope, available, status) = match spec.runtime {
                "local-onnx" => {
                    let available = engine
                        .is_model_available_for(spec.model_id)
                        .unwrap_or(false);
                    (
                        "local",
                        available,
                        if available { "ready" } else { "model" },
                    )
                }
                "cloud-api" => {
                    let available = match spec.api_key_provider {
                        Some("google") => availability.google_key_exists,
                        Some("cohere") => availability.cohere_key_exists,
                        Some("openai") => availability.openai_key_exists,
                        _ => false,
                    };
                    ("cloud", available, if available { "ready" } else { "key" })
                }
                "local-api" => (
                    "local",
                    availability.ollama_available,
                    if availability.ollama_available {
                        "ready"
                    } else {
                        "offline"
                    },
                ),
                _ => ("unknown", false, "unsupported"),
            };

            EmbeddingProviderInfo {
                id: spec.id.to_string(),
                label: spec.label.to_string(),
                short_label: spec.short_label.to_string(),
                model_name: spec.model_id.to_string(),
                dimensions: spec.dimensions,
                dimensions_label: if spec.dimensions == 0 {
                    "model".to_string()
                } else {
                    format!("{}d", spec.dimensions)
                },
                scope: scope.to_string(),
                runtime: spec.runtime.to_string(),
                status: status.to_string(),
                available,
                downloadable: spec.downloadable,
                download_label: spec.download_label.map(str::to_string),
                api_key_provider: spec.api_key_provider.map(str::to_string),
            }
        })
        .collect()
}

fn api_key_exists(state: &AppState, provider: &str) -> Result<bool, String> {
    let flag_key = format!("api_key_exists_{}", provider);
    match state.db.get_setting(&flag_key) {
        Ok(Some(value)) => Ok(value == "true"),
        Ok(None) => Ok(false),
        Err(err) => Err(err.to_string()),
    }
}

fn openai_embedding_model(state: &AppState) -> Result<String, String> {
    Ok(state
        .db
        .get_setting("openai_embedding_model")
        .map_err(|e| e.to_string())?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| OPENAI_EMBEDDING_MODEL.to_string()))
}

fn cohere_embedding_model(state: &AppState) -> Result<String, String> {
    Ok(state
        .db
        .get_setting("cohere_embedding_model")
        .map_err(|e| e.to_string())?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| COHERE_EMBEDDING_MODEL.to_string()))
}

fn ollama_embedding_config(state: &AppState) -> Result<(String, String), String> {
    let url = state
        .db
        .get_setting("ollama_embedding_url")
        .map_err(|e| e.to_string())?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| OLLAMA_EMBEDDING_URL.to_string());
    let model = state
        .db
        .get_setting("ollama_embedding_model")
        .map_err(|e| e.to_string())?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| OLLAMA_EMBEDDING_MODEL.to_string());
    Ok((url, model))
}

fn ollama_model_available(models: &[String], configured_model: &str) -> bool {
    let configured = configured_model.trim();
    if configured.is_empty() {
        return false;
    }
    models.iter().any(|model| {
        model == configured
            || model
                .strip_suffix(":latest")
                .map(|base| base == configured)
                .unwrap_or(false)
            || (!configured.contains(':')
                && model
                    .split_once(':')
                    .map(|(base, _)| base == configured)
                    .unwrap_or(false))
    })
}

#[cfg(test)]
fn clip_model_download_info_for_path(model_path: &Path) -> ClipModelDownloadInfo {
    embedding_model_download_info_for_path(CLIP_MODEL_ID, model_path)
        .expect("CLIP model spec should exist")
}

fn embedding_model_download_info_for_path(
    model_id: &str,
    model_path: &Path,
) -> Result<EmbeddingModelDownloadInfo, String> {
    let spec = embedding_model_spec(model_id)
        .ok_or_else(|| format!("Unsupported embedding model '{}'", model_id))?;
    let part_path = crate::services::model_download::part_path_for(model_path);
    let model_path = model_path.to_string_lossy().to_string();
    let part_path = part_path.to_string_lossy().to_string();
    let quoted_part_path = shell_quote(&part_path);
    let quoted_model_path = shell_quote(&model_path);
    let quoted_url = shell_quote(spec.url);

    Ok(EmbeddingModelDownloadInfo {
        model_id: spec.model_id.to_string(),
        url: spec.url.to_string(),
        model_path: model_path.clone(),
        part_path: part_path.clone(),
        curl_command: format!(
            "mkdir -p {} && curl -L -C - -o {} {} && mv {} {}",
            shell_quote(
                Path::new(&model_path)
                    .parent()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string())
                    .as_str()
            ),
            quoted_part_path,
            quoted_url,
            shell_quote(&part_path),
            quoted_model_path
        ),
    })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[tauri::command]
pub async fn generate_embeddings(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    crate::services::model_pipeline::ensure_clip_model_loaded(&state.embedding_engine)?;
    let result = crate::services::model_pipeline::run_clip_embeddings(
        crate::services::model_pipeline::ClipEmbeddingRunRequest {
            db: &state.db,
            app_data_dir: &state.app_data_dir,
            embedding_engine: &state.embedding_engine,
            jobs: None,
            job_id: None,
            cancel: None,
            app: Some(&app),
            image_ids: &image_ids,
        },
    )?;
    Ok(result.generated)
}

#[tauri::command]
pub async fn generate_model_embeddings(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    generate_embeddings_for_model(&app, &state, &model, &image_ids).await
}

async fn generate_embeddings_for_model(
    app: &AppHandle,
    state: &AppState,
    model_id: &str,
    image_ids: &[String],
) -> Result<u32, String> {
    if model_id == GEMINI_EMBEDDING_MODEL_ID {
        return generate_gemini_embeddings_for(app, state, image_ids).await;
    }
    if let Some(model) = model_id.strip_prefix("openai:") {
        let model = if model.trim().is_empty() {
            openai_embedding_model(state)?
        } else {
            model.to_string()
        };
        return generate_openai_text_embeddings(app, state, &model, image_ids).await;
    }
    if let Some(model) = model_id.strip_prefix("cohere:") {
        let model = if model.trim().is_empty() {
            cohere_embedding_model(state)?
        } else {
            model.to_string()
        };
        return generate_cohere_image_embeddings(app, state, &model, image_ids).await;
    }
    if let Some(model) = model_id.strip_prefix("ollama:") {
        let (_, configured_model) = ollama_embedding_config(state)?;
        let model = if model.trim().is_empty() {
            configured_model
        } else {
            model.to_string()
        };
        return generate_ollama_text_embeddings(app, state, &model, image_ids).await;
    }
    if embedding_provider_for_model(model_id).is_none() {
        return Err(format!("Unsupported embedding model '{}'", model_id));
    }

    crate::services::model_pipeline::ensure_embedding_model_loaded(
        &state.embedding_engine,
        model_id,
    )?;
    let result = crate::services::model_pipeline::run_embedding_model(
        crate::services::model_pipeline::EmbeddingRunRequest {
            db: &state.db,
            app_data_dir: &state.app_data_dir,
            embedding_engine: &state.embedding_engine,
            jobs: None,
            job_id: None,
            cancel: None,
            app: Some(app),
            model_id,
            image_ids,
        },
    )?;
    Ok(result.generated)
}

#[tauri::command]
pub async fn get_embedding_page(
    state: State<'_, AppState>,
    model: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<crate::db_core::models::EmbeddingPage, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_embedding_page(
        &ctx,
        model.as_deref(),
        crate::services::Pagination { offset, limit },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_similar_images(
    state: State<'_, AppState>,
    image_id: String,
    top_k: u32,
    model: Option<String>,
) -> Result<Vec<(String, f32)>, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::find_similar_images(&ctx, &image_id, top_k as usize, model.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_similarity_groups(
    state: State<'_, AppState>,
    model: Option<String>,
    threshold: Option<f64>,
    min_group_size: Option<u32>,
) -> Result<crate::db_core::models::SimilarityGroupingResult, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::generate_similarity_groups(
        &ctx,
        model.as_deref(),
        threshold.unwrap_or(0.88),
        min_group_size.unwrap_or(2),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_similarity_groups(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<crate::db_core::models::SimilarityGroupSummary>, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::list_similarity_groups(
        &ctx,
        crate::services::Pagination::clamped(offset.unwrap_or(0), limit.unwrap_or(100)),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_similarity_group_images(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<Vec<crate::db_core::models::ImageWithFile>, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::list_similarity_group_images(&ctx, &group_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_clip_model_download_info(
    state: State<'_, AppState>,
) -> Result<ClipModelDownloadInfo, String> {
    get_embedding_model_download_info(state, CLIP_MODEL_ID.to_string()).await
}

#[tauri::command]
pub async fn get_embedding_model_download_info(
    state: State<'_, AppState>,
    model: String,
) -> Result<EmbeddingModelDownloadInfo, String> {
    let model_path = {
        let engine = state.embedding_engine.lock();
        engine.model_path_for(&model)?
    };
    embedding_model_download_info_for_path(&model, &model_path)
}

#[tauri::command]
pub async fn list_embedding_providers(
    state: State<'_, AppState>,
) -> Result<Vec<EmbeddingProviderInfo>, String> {
    let openai_model = openai_embedding_model(&state)?;
    let cohere_model = cohere_embedding_model(&state)?;
    let (ollama_url, ollama_model) = ollama_embedding_config(&state)?;
    let ollama_models = check_ollama_embedding_available(&ollama_url)
        .await
        .unwrap_or_default();
    let availability = ProviderAvailability {
        google_key_exists: api_key_exists(&state, "google")?,
        cohere_key_exists: api_key_exists(&state, "cohere")?,
        openai_key_exists: api_key_exists(&state, "openai")?,
        ollama_available: ollama_model_available(&ollama_models, &ollama_model),
    };
    let engine = state.embedding_engine.lock();
    let mut providers = embedding_provider_infos(&engine, availability);
    for provider in &mut providers {
        if provider.id == "openai" {
            provider.model_name = openai_storage_model_id(&openai_model);
        } else if provider.id == "cohere" {
            provider.model_name = cohere_storage_model_id(&cohere_model);
        } else if provider.id == "ollama" {
            provider.model_name = ollama_storage_model_id(&ollama_model);
        }
    }
    Ok(providers)
}

#[tauri::command]
pub async fn check_ollama_embedding(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let (url, _) = ollama_embedding_config(&state)?;
    check_ollama_embedding_available(&url).await
}

#[tauri::command]
pub async fn get_ollama_embedding_config(
    state: State<'_, AppState>,
) -> Result<(String, String), String> {
    ollama_embedding_config(&state)
}

#[tauri::command]
pub async fn set_ollama_embedding_config(
    state: State<'_, AppState>,
    url: Option<String>,
    model: Option<String>,
) -> Result<(), String> {
    if let Some(url) = url {
        state
            .db
            .set_setting("ollama_embedding_url", &url)
            .map_err(|e| e.to_string())?;
    }
    if let Some(model) = model {
        state
            .db
            .set_setting("ollama_embedding_model", &model)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_model_download_info_includes_real_model_path_and_curl_command() {
        let info = clip_model_download_info_for_path(Path::new(
            "/tmp/cull-models/clip-vit-b32-vision.onnx",
        ));

        assert_eq!(info.model_path, "/tmp/cull-models/clip-vit-b32-vision.onnx");
        assert_eq!(
            info.part_path,
            "/tmp/cull-models/clip-vit-b32-vision.onnx.part"
        );
        assert_eq!(info.url, crate::db_core::embeddings::CLIP_MODEL_SPEC.url);
        assert!(info
            .curl_command
            .contains("'/tmp/cull-models/clip-vit-b32-vision.onnx.part'"));
        assert!(info
            .curl_command
            .contains("'/tmp/cull-models/clip-vit-b32-vision.onnx'"));
    }

    #[test]
    fn dinov2_model_download_info_uses_model_specific_url_and_path() {
        let info = embedding_model_download_info_for_path(
            "dinov2-vits14",
            Path::new("/tmp/cull-models/dinov2-vits14.onnx"),
        )
        .unwrap();

        assert_eq!(info.model_id, "dinov2-vits14");
        assert_eq!(
            info.url,
            "https://huggingface.co/sefaburak/dinov2-small-onnx/resolve/main/dinov2_vits14.onnx"
        );
        assert_eq!(info.model_path, "/tmp/cull-models/dinov2-vits14.onnx");
        assert_eq!(info.part_path, "/tmp/cull-models/dinov2-vits14.onnx.part");
        assert!(info
            .curl_command
            .contains("'/tmp/cull-models/dinov2-vits14.onnx.part'"));
        assert!(info.curl_command.contains("'https://huggingface.co/"));
    }

    #[test]
    fn provider_infos_report_local_model_and_cloud_key_availability() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = crate::db_core::embeddings::EmbeddingEngine::new(tmp.path());

        let missing = embedding_provider_infos(&engine, ProviderAvailability::default());
        assert_eq!(missing.len(), 6);
        assert_eq!(missing[0].id, "clip");
        assert_eq!(missing[0].scope, "local");
        assert_eq!(missing[0].status, "model");
        assert!(!missing[0].available);
        assert_eq!(missing[1].id, "dinov2");
        assert_eq!(missing[1].status, "model");
        assert_eq!(missing[2].id, "gemini");
        assert_eq!(missing[2].scope, "cloud");
        assert_eq!(missing[2].status, "key");
        assert_eq!(missing[2].api_key_provider.as_deref(), Some("google"));
        assert_eq!(missing[3].id, "cohere");
        assert_eq!(missing[3].status, "key");
        assert_eq!(missing[3].api_key_provider.as_deref(), Some("cohere"));
        assert_eq!(missing[4].id, "openai");
        assert_eq!(missing[4].status, "key");
        assert_eq!(missing[4].api_key_provider.as_deref(), Some("openai"));
        assert_eq!(missing[5].id, "ollama");
        assert_eq!(missing[5].scope, "local");
        assert_eq!(missing[5].status, "offline");
        assert_eq!(missing[5].dimensions_label, "model");

        std::fs::write(tmp.path().join("clip-vit-b32-vision.onnx"), b"model").unwrap();

        let with_ready_providers = embedding_provider_infos(
            &engine,
            ProviderAvailability {
                google_key_exists: true,
                cohere_key_exists: true,
                openai_key_exists: true,
                ollama_available: true,
            },
        );
        assert_eq!(with_ready_providers[0].status, "ready");
        assert!(with_ready_providers[0].available);
        assert_eq!(with_ready_providers[1].status, "model");
        assert_eq!(with_ready_providers[2].status, "ready");
        assert!(with_ready_providers[2].available);
        assert_eq!(with_ready_providers[3].status, "ready");
        assert!(with_ready_providers[3].available);
        assert_eq!(with_ready_providers[4].status, "ready");
        assert!(with_ready_providers[4].available);
        assert_eq!(with_ready_providers[5].status, "ready");
        assert!(with_ready_providers[5].available);
    }

    #[test]
    fn ollama_model_available_accepts_latest_tag_alias() {
        let models = vec![
            "embeddinggemma:latest".to_string(),
            "nomic-embed-text:v1.5".to_string(),
        ];

        assert!(ollama_model_available(&models, "embeddinggemma"));
        assert!(ollama_model_available(&models, "embeddinggemma:latest"));
        assert!(ollama_model_available(&models, "nomic-embed-text"));
        assert!(!ollama_model_available(&models, "mxbai-embed-large"));
    }
}

#[tauri::command]
pub async fn download_clip_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    download_embedding_model_for(&app, &state, CLIP_MODEL_ID).await
}

#[tauri::command]
pub async fn download_embedding_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
) -> Result<String, String> {
    download_embedding_model_for(&app, &state, &model).await
}

async fn download_embedding_model_for(
    app: &AppHandle,
    state: &AppState,
    model_id: &str,
) -> Result<String, String> {
    let spec = embedding_model_spec(model_id)
        .ok_or_else(|| format!("Unsupported embedding model '{}'", model_id))?;
    let model_path = {
        let engine = state.embedding_engine.lock();
        engine.model_path_for(spec.model_id)?
    };

    if model_path.exists() {
        return Ok("already_downloaded".to_string());
    }

    let client = reqwest::Client::new();
    let (job_id, _cancel) = state
        .jobs
        .create_job(&format!("{}-download", spec.model_id), 0);
    let control = state
        .jobs
        .control_for(&job_id)
        .ok_or_else(|| format!("Download job '{}' not found", job_id))?;
    let outcome = crate::services::model_download::download_model_file_controlled(
        &client,
        spec.url,
        &model_path,
        &control,
        |progress| {
            state.jobs.update_progress(
                &job_id,
                progress.downloaded.min(u32::MAX as u64) as u32,
                Some(&format!("Downloading {}", spec.display_name)),
            );
            let _ = app.emit(
                "model-download-progress",
                serde_json::json!({
                    "job_id": job_id,
                    "model": spec.model_id,
                    "downloaded": progress.downloaded,
                    "total": progress.total,
                    "status": progress.status,
                    "resumable": progress.resumable,
                }),
            );
        },
    )
    .await
    .map_err(|err| {
        if control.cancellation_token().is_cancelled() {
            state.jobs.mark_cancelled(&job_id);
        } else {
            state.jobs.fail(&job_id, &err);
        }
        state.jobs.persist_terminal(&job_id, &state.db);
        let _ = app.emit(
            "model-download-progress",
            serde_json::json!({
                "job_id": job_id,
                "model": spec.model_id,
                "downloaded": 0u64,
                "total": 0u64,
                "status": if control.cancellation_token().is_cancelled() { "cancelled" } else { "failed" },
                "resumable": true,
                "error": err,
            }),
        );
        err
    })?;

    // Load the model after download
    {
        let mut engine = state.embedding_engine.lock();
        engine.load_model_for(spec.model_id).map_err(|err| {
            state.jobs.fail(&job_id, &err);
            state.jobs.persist_terminal(&job_id, &state.db);
            err
        })?;
    }

    state.jobs.complete(&job_id);
    state.jobs.persist_terminal(&job_id, &state.db);
    Ok(if outcome.resumed {
        "resumed".to_string()
    } else {
        "downloaded".to_string()
    })
}

#[tauri::command]
pub async fn is_model_available(state: State<'_, AppState>) -> Result<bool, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::is_clip_available(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_embedding_model_available(
    state: State<'_, AppState>,
    model: String,
) -> Result<bool, String> {
    let engine = state.embedding_engine.lock();
    engine.is_model_available_for(&model)
}

#[tauri::command]
pub async fn get_embedding_count(
    state: State<'_, AppState>,
    model: Option<String>,
) -> Result<u32, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_embedding_count(&ctx, model.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_api_key(
    state: State<'_, AppState>,
    provider: String,
    key: String,
) -> Result<(), String> {
    let secret_key = format!("api_key_{}", provider);
    state.secrets.set(&secret_key, &key)?;
    let flag_key = format!("api_key_exists_{}", provider);
    state
        .db
        .set_setting(&flag_key, "true")
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn validate_api_key(provider: String, key: String) -> Result<bool, String> {
    let client = reqwest::Client::new();
    match provider.as_str() {
        "google" => {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                key
            );
            let resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("{}", e))?;
            Ok(resp.status().is_success())
        }
        "openai" => {
            let resp = client
                .get("https://api.openai.com/v1/models")
                .header("Authorization", format!("Bearer {}", key))
                .send()
                .await
                .map_err(|e| format!("{}", e))?;
            Ok(resp.status().is_success())
        }
        "cohere" => {
            let resp = client
                .get("https://api.cohere.com/v1/models")
                .bearer_auth(key)
                .send()
                .await
                .map_err(|e| format!("{}", e))?;
            Ok(resp.status().is_success())
        }
        "openrouter" => {
            let resp = client
                .get("https://openrouter.ai/api/v1/models")
                .header("Authorization", format!("Bearer {}", key))
                .send()
                .await
                .map_err(|e| format!("{}", e))?;
            Ok(resp.status().is_success())
        }
        _ => Ok(false),
    }
}

#[tauri::command]
pub async fn delete_api_key(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    let secret_key = format!("api_key_{}", provider);
    state.secrets.delete(&secret_key)?;
    let flag_key = format!("api_key_exists_{}", provider);
    state
        .db
        .set_setting(&flag_key, "false")
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn has_api_key(state: State<'_, AppState>, provider: String) -> Result<bool, String> {
    let flag_key = format!("api_key_exists_{}", provider);
    match state.db.get_setting(&flag_key) {
        Ok(Some(v)) => Ok(v == "true"),
        Ok(None) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

enum TextEmbeddingProvider {
    OpenAi(OpenAiTextEmbeddingProvider),
    Ollama(OllamaTextEmbeddingProvider),
}

impl TextEmbeddingProvider {
    async fn generate_embedding(&self, input: &str) -> Result<Vec<f32>, String> {
        match self {
            TextEmbeddingProvider::OpenAi(provider) => provider.generate_embedding(input).await,
            TextEmbeddingProvider::Ollama(provider) => provider.generate_embedding(input).await,
        }
    }
}

async fn generate_openai_text_embeddings(
    app: &AppHandle,
    state: &AppState,
    model: &str,
    image_ids: &[String],
) -> Result<u32, String> {
    let api_key = state
        .secrets
        .get("api_key_openai")?
        .ok_or("OpenAI API key not set")?;
    let _ = state.db.set_setting("api_key_exists_openai", "true");
    generate_text_embeddings_for(
        app,
        state,
        image_ids,
        TextEmbeddingRun {
            provider: TextEmbeddingProvider::OpenAi(OpenAiTextEmbeddingProvider::new(
                &api_key, model,
            )),
            provider_name: "openai",
            endpoint: OPENAI_EMBEDDING_ENDPOINT.to_string(),
            model,
            storage_model_id: openai_storage_model_id(model),
            jurisdiction: "US - OpenAI Inc",
        },
    )
    .await
}

async fn generate_cohere_image_embeddings(
    app: &AppHandle,
    state: &AppState,
    model: &str,
    image_ids: &[String],
) -> Result<u32, String> {
    let api_key = state
        .secrets
        .get("api_key_cohere")?
        .ok_or("Cohere API key not set")?;
    let _ = state.db.set_setting("api_key_exists_cohere", "true");
    let provider = CohereImageEmbeddingProvider::new(&api_key, model);
    let storage_model_id = cohere_storage_model_id(model);
    let total = image_ids.len() as u32;
    let mut generated = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let images = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;
        let img = images.first().ok_or("Image not found")?;
        let ml_path = crate::commands::resolve_image_path_for_ml(img, &state.app_data_dir);
        let file_size = std::fs::metadata(&ml_path)
            .map(|metadata| metadata.len() as i64)
            .unwrap_or(0);
        let dims = format!("{}x{}", img.image.width, img.image.height);

        match provider.generate_embedding(&ml_path).await {
            Ok(embedding) => {
                let embedding = normalize_embedding(embedding);
                let _ = crate::services::audit::log_api_call(
                    &state.db,
                    "cohere",
                    COHERE_EMBEDDING_ENDPOINT,
                    "image",
                    file_size,
                    None,
                    Some(&dims),
                    Some(model),
                    200,
                    "CA/US - Cohere",
                );
                state
                    .db
                    .store_embedding(image_id, &storage_model_id, &embedding)
                    .map_err(|e| e.to_string())?;
                generated += 1;
            }
            Err(e) => {
                let _ = crate::services::audit::log_api_call(
                    &state.db,
                    "cohere",
                    COHERE_EMBEDDING_ENDPOINT,
                    "image",
                    file_size,
                    None,
                    Some(&dims),
                    Some(model),
                    500,
                    "CA/US - Cohere",
                );
                crate::safe_eprintln!("Cohere embedding error for {}: {}", image_id, e);
            }
        }

        let _ = app.emit(
            "embedding-progress",
            serde_json::json!({
                "current": i + 1,
                "total": total,
                "provider": "cohere",
                "model": storage_model_id,
            }),
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(generated)
}

async fn generate_ollama_text_embeddings(
    app: &AppHandle,
    state: &AppState,
    model: &str,
    image_ids: &[String],
) -> Result<u32, String> {
    let (url, _) = ollama_embedding_config(state)?;
    let endpoint = crate::db_core::remote_embeddings::normalize_ollama_embed_url(&url);
    let jurisdiction = if endpoint.contains("localhost") || endpoint.contains("127.0.0.1") {
        "Local"
    } else {
        "Remote"
    };
    generate_text_embeddings_for(
        app,
        state,
        image_ids,
        TextEmbeddingRun {
            provider: TextEmbeddingProvider::Ollama(OllamaTextEmbeddingProvider::new(
                &endpoint, model,
            )),
            provider_name: "ollama",
            endpoint,
            model,
            storage_model_id: ollama_storage_model_id(model),
            jurisdiction,
        },
    )
    .await
}

struct TextEmbeddingRun<'a> {
    provider: TextEmbeddingProvider,
    provider_name: &'static str,
    endpoint: String,
    model: &'a str,
    storage_model_id: String,
    jurisdiction: &'static str,
}

async fn generate_text_embeddings_for(
    app: &AppHandle,
    state: &AppState,
    image_ids: &[String],
    run: TextEmbeddingRun<'_>,
) -> Result<u32, String> {
    let total = image_ids.len() as u32;
    let mut generated = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let images = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;
        let img = images.first().ok_or("Image not found")?;
        let generation_run = state
            .db
            .get_generation_run_for_image(image_id)
            .map_err(|e| e.to_string())?;
        let vision_metadata = state
            .db
            .get_vision_metadata(image_id)
            .map_err(|e| e.to_string())?;
        let input = build_text_embedding_input(img, generation_run.as_ref(), &vision_metadata);
        let dims = format!("{}x{}", img.image.width, img.image.height);

        match run.provider.generate_embedding(&input).await {
            Ok(embedding) => {
                let embedding = normalize_embedding(embedding);
                let _ = crate::services::audit::log_api_call(
                    &state.db,
                    run.provider_name,
                    &run.endpoint,
                    "text",
                    input.len() as i64,
                    Some(&input),
                    Some(&dims),
                    Some(run.model),
                    200,
                    run.jurisdiction,
                );
                state
                    .db
                    .store_embedding(image_id, &run.storage_model_id, &embedding)
                    .map_err(|e| e.to_string())?;
                generated += 1;
            }
            Err(e) => {
                let _ = crate::services::audit::log_api_call(
                    &state.db,
                    run.provider_name,
                    &run.endpoint,
                    "text",
                    input.len() as i64,
                    Some(&input),
                    Some(&dims),
                    Some(run.model),
                    500,
                    run.jurisdiction,
                );
                crate::safe_eprintln!(
                    "{} embedding error for {}: {}",
                    run.provider_name,
                    image_id,
                    e
                );
            }
        }

        let _ = app.emit(
            "embedding-progress",
            serde_json::json!({
                "current": i + 1,
                "total": total,
                "provider": run.provider_name,
                "model": run.storage_model_id,
            }),
        );

        if run.provider_name == "openai" {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    Ok(generated)
}

#[tauri::command]
pub async fn generate_gemini_embeddings(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    generate_gemini_embeddings_for(&app, &state, &image_ids).await
}

async fn generate_gemini_embeddings_for(
    app: &AppHandle,
    state: &AppState,
    image_ids: &[String],
) -> Result<u32, String> {
    let api_key = state
        .secrets
        .get("api_key_google")?
        .ok_or("Google API key not set")?;

    // Backfill presence flag for existing keys migrated before this feature
    let _ = state.db.set_setting("api_key_exists_google", "true");

    let provider = GeminiEmbeddingProvider::new(&api_key);
    let total = image_ids.len() as u32;
    let mut generated = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let images = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;
        let img = images.first().ok_or("Image not found")?;

        let ml_path = crate::commands::resolve_image_path_for_ml(img, &state.app_data_dir);
        let file_size = std::fs::metadata(&ml_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
        let dims = format!("{}x{}", img.image.width, img.image.height);
        match provider.generate_embedding(&ml_path).await {
            Ok(embedding) => {
                let _ = crate::services::audit::log_api_call(
                    &state.db, "gemini",
                    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-exp-03-07:embedContent",
                    "image", file_size, None, Some(&dims),
                    Some("gemini-embedding-exp-03-07"), 200, "US - Google LLC",
                );
                state
                    .db
                    .store_embedding(image_id, GEMINI_EMBEDDING_MODEL_ID, &embedding)
                    .map_err(|e| e.to_string())?;
                generated += 1;
            }
            Err(e) => {
                let _ = crate::services::audit::log_api_call(
                    &state.db, "gemini",
                    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-exp-03-07:embedContent",
                    "image", file_size, None, Some(&dims),
                    Some("gemini-embedding-exp-03-07"), 500, "US - Google LLC",
                );
                crate::safe_eprintln!("Gemini embedding error for {}: {}", image_id, e);
            }
        }

        let _ = app.emit(
            "embedding-progress",
            serde_json::json!({
                "current": i + 1,
                "total": total,
                "provider": "gemini",
                "model": GEMINI_EMBEDDING_MODEL_ID,
            }),
        );

        // Rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(generated)
}
