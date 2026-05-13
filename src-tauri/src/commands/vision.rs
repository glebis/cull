use crate::db_core::vision;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn check_ollama(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let url = state
        .db
        .get_setting("ollama_url")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| vision::default_ollama_url().to_string());
    vision::check_ollama_available(&url).await
}

#[tauri::command]
pub async fn set_ollama_config(
    state: State<'_, AppState>,
    url: Option<String>,
    model: Option<String>,
) -> Result<(), String> {
    if let Some(u) = url {
        state
            .db
            .set_setting("ollama_url", &u)
            .map_err(|e| e.to_string())?;
    }
    if let Some(m) = model {
        state
            .db
            .set_setting("ollama_model", &m)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_ollama_config(state: State<'_, AppState>) -> Result<(String, String), String> {
    let url = state
        .db
        .get_setting("ollama_url")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| vision::default_ollama_url().to_string());
    let model = state
        .db
        .get_setting("ollama_model")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "minicpm-v".to_string());
    Ok((url, model))
}

#[tauri::command]
pub async fn analyze_images(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let url = state
        .db
        .get_setting("ollama_url")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| vision::default_ollama_url().to_string());
    let model = state
        .db
        .get_setting("ollama_model")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "minicpm-v".to_string());

    let total = image_ids.len() as u32;
    let mut analyzed = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let images = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;
        let img = match images.first() {
            Some(img) => img,
            None => continue,
        };

        let ollama_jurisdiction = if url.contains("localhost") || url.contains("127.0.0.1") {
            "Local"
        } else {
            "Remote"
        };
        match vision::analyze_image(std::path::Path::new(&img.path), &url, &model).await {
            Ok(fields) => {
                let _ = crate::services::audit::log_api_call(
                    &state.db,
                    "ollama",
                    &url,
                    "image",
                    std::fs::metadata(&img.path)
                        .map(|m| m.len() as i64)
                        .unwrap_or(0),
                    None,
                    None,
                    Some(&model),
                    200,
                    ollama_jurisdiction,
                );
                state
                    .db
                    .store_vision_metadata(image_id, &model, &fields)
                    .map_err(|e| e.to_string())?;
                analyzed += 1;
            }
            Err(e) => {
                let _ = crate::services::audit::log_api_call(
                    &state.db,
                    "ollama",
                    &url,
                    "image",
                    std::fs::metadata(&img.path)
                        .map(|m| m.len() as i64)
                        .unwrap_or(0),
                    None,
                    None,
                    Some(&model),
                    500,
                    ollama_jurisdiction,
                );
                eprintln!("Vision error for {}: {}", image_id, e);
            }
        }

        let _ = app.emit(
            "vision-progress",
            serde_json::json!({
                "current": i + 1, "total": total, "model": model
            }),
        );
    }

    Ok(analyzed)
}

#[tauri::command]
pub async fn get_vision_metadata(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Vec<(String, String, String)>, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_vision_metadata(&ctx, &image_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_vision_count(
    state: State<'_, AppState>,
    source: Option<String>,
) -> Result<u32, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_vision_count(&ctx, source.as_deref()).map_err(|e| e.to_string())
}
