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
pub async fn download_clip_model(state: State<'_, AppState>) -> Result<String, String> {
    let engine = state.embedding_engine.lock().unwrap();
    let model_path = engine.model_path();
    drop(engine);

    if model_path.exists() {
        return Ok("already_downloaded".to_string());
    }

    let url = "https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx";
    let response = reqwest::blocking::get(url).map_err(|e| format!("Download: {}", e))?;
    let bytes = response.bytes().map_err(|e| format!("Read: {}", e))?;
    std::fs::write(&model_path, &bytes).map_err(|e| format!("Save: {}", e))?;

    // Load the model after download
    let mut engine = state.embedding_engine.lock().unwrap();
    engine.load_model()?;

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
