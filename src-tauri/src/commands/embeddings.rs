use tauri::{AppHandle, Emitter, State};
use crate::AppState;

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
pub async fn get_all_embeddings(state: State<'_, AppState>) -> Result<Vec<(String, Vec<f32>)>, String> {
    state.db.get_all_embeddings("clip-vit-b32").map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_similar_images(
    state: State<'_, AppState>,
    image_id: String,
    top_k: u32,
) -> Result<Vec<(String, f32)>, String> {
    let all = state.db.get_all_embeddings("clip-vit-b32").map_err(|e| e.to_string())?;
    let query = all.iter().find(|(id, _)| id == &image_id)
        .ok_or("Image has no embedding")?;
    state.db.find_similar(&query.1, "clip-vit-b32", top_k as usize).map_err(|e| e.to_string())
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
pub async fn get_embedding_count(state: State<'_, AppState>) -> Result<u32, String> {
    state.db.embedding_count("clip-vit-b32").map_err(|e| e.to_string())
}
