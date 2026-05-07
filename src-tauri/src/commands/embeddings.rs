use tauri::{AppHandle, Emitter, State};
use crate::AppState;
use crate::db_core::gemini::GeminiEmbeddingProvider;

#[tauri::command]
pub async fn generate_embeddings(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    // Ensure model is loaded
    {
        let mut engine = state.embedding_engine.lock().unwrap();
        if engine.session.is_none() {
            if !engine.is_model_available() {
                return Err("Model not downloaded. Run download_clip_model first.".to_string());
            }
            engine.load_model()?;
        }
    }

    let total = image_ids.len() as u32;
    let mut generated = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        // Get image path from DB
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let images = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        let img = images.first().ok_or("Image not found")?;

        // Generate embedding
        let engine = state.embedding_engine.lock().unwrap();
        match engine.generate_embedding(std::path::Path::new(&img.path)) {
            Ok(embedding) => {
                drop(engine); // Release lock before DB write
                state.db.store_embedding(image_id, "clip-vit-b32", &embedding).map_err(|e| e.to_string())?;
                generated += 1;
            }
            Err(e) => {
                drop(engine);
                eprintln!("Embedding error for {}: {}", image_id, e);
            }
        }

        // Emit progress
        let _ = app.emit("embedding-progress", serde_json::json!({
            "current": i + 1,
            "total": total,
        }));
    }

    Ok(generated)
}

#[tauri::command]
pub async fn get_all_embeddings(state: State<'_, AppState>, model: Option<String>) -> Result<Vec<(String, Vec<f32>)>, String> {
    let model_name = model.as_deref().unwrap_or("clip-vit-b32");
    state.db.get_all_embeddings(model_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_similar_images(
    state: State<'_, AppState>,
    image_id: String,
    top_k: u32,
    model: Option<String>,
) -> Result<Vec<(String, f32)>, String> {
    let model_name = model.as_deref().unwrap_or("clip-vit-b32");
    let all = state.db.get_all_embeddings(model_name).map_err(|e| e.to_string())?;
    let query = all.iter().find(|(id, _)| id == &image_id)
        .ok_or("Image has no embedding")?;
    state.db.find_similar(&query.1, model_name, top_k as usize).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_clip_model(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let model_path = {
        let engine = state.embedding_engine.lock().unwrap();
        engine.model_path()
    };

    if model_path.exists() {
        return Ok("already_downloaded".to_string());
    }

    let url = "https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx";

    let client = reqwest::Client::new();
    let response = client.get(url).send().await.map_err(|e| format!("Request error: {}", e))?;

    let total_size = response.content_length().unwrap_or(0);

    // Emit initial progress
    let _ = app.emit("model-download-progress", serde_json::json!({
        "downloaded": 0u64,
        "total": total_size,
        "status": "downloading"
    }));

    let mut file = std::fs::File::create(&model_path).map_err(|e| format!("File create error: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk).map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;

        // Emit progress every ~500KB to avoid flooding
        if downloaded % (512 * 1024) < chunk.len() as u64 || downloaded == total_size {
            let _ = app.emit("model-download-progress", serde_json::json!({
                "downloaded": downloaded,
                "total": total_size,
                "status": "downloading"
            }));
        }
    }

    let _ = app.emit("model-download-progress", serde_json::json!({
        "downloaded": total_size,
        "total": total_size,
        "status": "complete"
    }));

    // Load the model after download
    {
        let mut engine = state.embedding_engine.lock().unwrap();
        engine.load_model()?;
    }

    Ok("downloaded".to_string())
}

#[tauri::command]
pub async fn is_model_available(state: State<'_, AppState>) -> Result<bool, String> {
    let engine = state.embedding_engine.lock().unwrap();
    Ok(engine.is_model_available())
}

#[tauri::command]
pub async fn get_embedding_count(state: State<'_, AppState>, model: Option<String>) -> Result<u32, String> {
    let model_name = model.as_deref().unwrap_or("clip-vit-b32");
    state.db.embedding_count(model_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_api_key(state: State<'_, AppState>, provider: String, key: String) -> Result<(), String> {
    let setting_key = format!("api_key_{}", provider);
    state.db.set_setting(&setting_key, &key).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_api_key(state: State<'_, AppState>, provider: String) -> Result<Option<String>, String> {
    let setting_key = format!("api_key_{}", provider);
    state.db.get_setting(&setting_key).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_api_key(provider: String, key: String) -> Result<bool, String> {
    if provider == "google" {
        let client = reqwest::Client::new();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}", key
        );
        let resp = client.get(&url).send().await.map_err(|e| format!("{}", e))?;
        Ok(resp.status().is_success())
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn generate_gemini_embeddings(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let api_key = state.db.get_setting("api_key_google")
        .map_err(|e| e.to_string())?
        .ok_or("Google API key not set")?;

    let provider = GeminiEmbeddingProvider::new(&api_key);
    let total = image_ids.len() as u32;
    let mut generated = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let images = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        let img = images.first().ok_or("Image not found")?;

        match provider.generate_embedding(std::path::Path::new(&img.path)).await {
            Ok(embedding) => {
                state.db.store_embedding(image_id, "gemini-embedding-2", &embedding)
                    .map_err(|e| e.to_string())?;
                generated += 1;
            }
            Err(e) => {
                eprintln!("Gemini embedding error for {}: {}", image_id, e);
            }
        }

        let _ = app.emit("embedding-progress", serde_json::json!({
            "current": i + 1,
            "total": total,
            "provider": "gemini"
        }));

        // Rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(generated)
}
