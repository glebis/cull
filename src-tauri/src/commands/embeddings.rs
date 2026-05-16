use crate::db_core::gemini::GeminiEmbeddingProvider;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

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
pub async fn download_clip_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let model_path = {
        let engine = state.embedding_engine.lock();
        engine.model_path()
    };

    if model_path.exists() {
        return Ok("already_downloaded".to_string());
    }

    let url = "https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx";

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    let total_size = response.content_length().unwrap_or(0);

    // Emit initial progress
    let _ = app.emit(
        "model-download-progress",
        serde_json::json!({
            "downloaded": 0u64,
            "total": total_size,
            "status": "downloading"
        }),
    );

    let mut file =
        std::fs::File::create(&model_path).map_err(|e| format!("File create error: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;

        // Emit progress every ~500KB to avoid flooding
        if downloaded % (512 * 1024) < chunk.len() as u64 || downloaded == total_size {
            let _ = app.emit(
                "model-download-progress",
                serde_json::json!({
                    "downloaded": downloaded,
                    "total": total_size,
                    "status": "downloading"
                }),
            );
        }
    }

    let _ = app.emit(
        "model-download-progress",
        serde_json::json!({
            "downloaded": total_size,
            "total": total_size,
            "status": "complete"
        }),
    );

    // Load the model after download
    {
        let mut engine = state.embedding_engine.lock();
        engine.load_model()?;
    }

    Ok("downloaded".to_string())
}

#[tauri::command]
pub async fn is_model_available(state: State<'_, AppState>) -> Result<bool, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::is_clip_available(&ctx).map_err(|e| e.to_string())
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

#[tauri::command]
pub async fn generate_gemini_embeddings(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
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
                    .store_embedding(image_id, "gemini-embedding-2", &embedding)
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
                eprintln!("Gemini embedding error for {}: {}", image_id, e);
            }
        }

        let _ = app.emit(
            "embedding-progress",
            serde_json::json!({
                "current": i + 1,
                "total": total,
                "provider": "gemini"
            }),
        );

        // Rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(generated)
}
