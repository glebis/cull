use crate::db_core::detection::{Detection, YoloVariant};
use crate::db_core::models::ImageWithFile;
use crate::services::Pagination;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn download_yolo_model(
    _app: AppHandle,
    _state: State<'_, AppState>,
    variant: String,
) -> Result<String, String> {
    let variant =
        YoloVariant::from_str(&variant).ok_or("Invalid variant. Use: nano, small, medium")?;
    Err(format!(
        "Built-in YOLO downloads are disabled for the Apache-2.0 release because {} weights are licensed separately by Ultralytics. Place a compatible ONNX model named {} in Cull's models directory to use local detection.",
        variant.model_name(),
        variant.filename()
    ))
}

#[tauri::command]
pub async fn download_nudenet_model(
    _app: AppHandle,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    Err(
        "Built-in NudeNet downloads are disabled for the Apache-2.0 release because the referenced ONNX weight license is not verified. Place a compatible ONNX model named nudenet.onnx in Cull's models directory to use local safety detection."
            .to_string(),
    )
}

#[tauri::command]
pub async fn detect_objects(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
    variant: Option<String>,
) -> Result<u32, String> {
    let variant = variant
        .as_deref()
        .map(|v| YoloVariant::from_str(v).ok_or("Invalid variant"))
        .transpose()?
        .unwrap_or(YoloVariant::Medium);

    {
        let mut engine = state.detection_engine.lock();
        let needs_load = engine.session.is_none() || engine.loaded_variant != Some(variant);
        if needs_load {
            if !engine.is_variant_available(variant) {
                return Err(format!("Model {} not downloaded", variant.model_name()));
            }
            engine.load_yolo(variant)?;
        }
    }

    let total = image_ids.len() as u32;
    let mut detected = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let images = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;
        let img = images.first().ok_or("Image not found")?;

        let detect_path = crate::commands::resolve_image_path_for_ml(img, &state.app_data_dir);
        let engine = state.detection_engine.lock();
        match engine.detect(&detect_path) {
            Ok(detections) => {
                drop(engine);
                state
                    .db
                    .store_detections(image_id, variant.model_name(), &detections)
                    .map_err(|e| e.to_string())?;
                detected += 1;
            }
            Err(e) => {
                drop(engine);
                crate::safe_eprintln!("Detection error for {}: {}", image_id, e);
            }
        }

        let _ = app.emit(
            "detection-progress",
            serde_json::json!({
                "current": i + 1, "total": total, "model": variant.model_name()
            }),
        );
    }

    Ok(detected)
}

#[tauri::command]
pub async fn detect_nsfw(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    {
        let mut engine = state.safety_engine.lock();
        if engine.session.is_none() {
            if !engine.is_nudenet_available() {
                return Err("NudeNet model not downloaded".to_string());
            }
            engine.load_nudenet()?;
        }
    }

    let total = image_ids.len() as u32;
    let mut detected = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let images = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;
        let img = images.first().ok_or("Image not found")?;

        let detect_path = crate::commands::resolve_image_path_for_ml(img, &state.app_data_dir);
        let engine = state.safety_engine.lock();
        match engine.detect(&detect_path) {
            Ok(detections) => {
                drop(engine);
                state
                    .db
                    .store_detections(image_id, "nudenet", &detections)
                    .map_err(|e| e.to_string())?;
                detected += 1;
            }
            Err(e) => {
                drop(engine);
                crate::safe_eprintln!("NudeNet error for {}: {}", image_id, e);
            }
        }

        let _ = app.emit(
            "nsfw-progress",
            serde_json::json!({
                "current": i + 1, "total": total
            }),
        );
    }

    Ok(detected)
}

#[tauri::command]
pub async fn get_detections(
    state: State<'_, AppState>,
    image_id: String,
    model: Option<String>,
) -> Result<Vec<Detection>, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_detections(&ctx, &image_id, model.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_by_detected_class(
    state: State<'_, AppState>,
    class_name: String,
    limit: Option<u32>,
) -> Result<Vec<(String, f32)>, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::search_by_detected_class(&ctx, &class_name, limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn count_by_detected_class(
    state: State<'_, AppState>,
    class_name: String,
) -> Result<u32, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::count_by_detected_class(&ctx, &class_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images_by_detected_class(
    state: State<'_, AppState>,
    class_name: String,
    limit: u32,
    offset: u32,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::list_images_by_detected_class(
        &ctx,
        &class_name,
        Pagination::clamped(offset, limit),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_yolo_available(
    state: State<'_, AppState>,
    variant: Option<String>,
) -> Result<bool, String> {
    let variant = variant
        .as_deref()
        .map(|v| YoloVariant::from_str(v).ok_or("Invalid variant"))
        .transpose()?
        .unwrap_or(YoloVariant::Medium);
    let engine = state.detection_engine.lock();
    Ok(engine.is_variant_available(variant))
}

#[tauri::command]
pub async fn is_nudenet_available(state: State<'_, AppState>) -> Result<bool, String> {
    let engine = state.safety_engine.lock();
    Ok(engine.is_nudenet_available())
}

#[tauri::command]
pub async fn get_detection_count(state: State<'_, AppState>, model: String) -> Result<u32, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_detection_count(&ctx, &model).map_err(|e| e.to_string())
}
