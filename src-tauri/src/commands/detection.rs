use crate::db_core::detection::{Detection, YoloVariant};
use crate::db_core::models::ImageWithFile;
use crate::services::Pagination;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

/// Expected SHA-256 hashes for ONNX model files.
/// These guard against corrupted downloads and MITM attacks.
fn expected_sha256(variant: &YoloVariant) -> &'static str {
    match variant {
        // TODO: populate on first verified download of nano and small models
        YoloVariant::Nano => "PLACEHOLDER_HASH_YOLO11N",
        YoloVariant::Small => "PLACEHOLDER_HASH_YOLO11S",
        YoloVariant::Medium => "8a37b5c53ff642831aa454156b548ec2cf2537827445385c3e1c1b276cb666a3",
    }
}

const NUDENET_EXPECTED_SHA256: &str =
    "9832f15515bdb06bcb5a77beb60bc8ea54439bd7ecbaac46dac3b760b3dd13cc";

/// Verify that a downloaded file matches the expected SHA-256 hash.
/// On mismatch the file is deleted and an error is returned.
fn verify_sha256(path: &std::path::Path, expected: &str) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file =
        std::fs::File::open(path).map_err(|e| format!("Cannot open for hash check: {}", e))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("Read error during hash check: {}", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = format!("{:x}", hasher.finalize());

    if hash != expected {
        // Delete the compromised/corrupt file before returning the error.
        let _ = std::fs::remove_file(path);
        return Err(format!(
            "SHA-256 mismatch for {}: expected {}, got {}",
            path.display(),
            expected,
            hash
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn download_yolo_model(
    app: AppHandle,
    state: State<'_, AppState>,
    variant: String,
) -> Result<String, String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let variant =
        YoloVariant::from_str(&variant).ok_or("Invalid variant. Use: nano, small, medium")?;

    let model_path = {
        let engine = state.detection_engine.lock();
        engine.model_path_for_variant(variant)
    };

    if model_path.exists() {
        return Ok("already_downloaded".to_string());
    }

    let url = variant.download_url();
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;
    let total_size = response.content_length().unwrap_or(0);

    let _ = app.emit("yolo-download-progress", serde_json::json!({
        "downloaded": 0u64, "total": total_size, "variant": variant.model_name(), "status": "downloading"
    }));

    let mut file =
        std::fs::File::create(&model_path).map_err(|e| format!("File create error: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;
        if downloaded % (512 * 1024) < chunk.len() as u64 || downloaded == total_size {
            let _ = app.emit("yolo-download-progress", serde_json::json!({
                "downloaded": downloaded, "total": total_size, "variant": variant.model_name(), "status": "downloading"
            }));
        }
    }

    let _ = app.emit("yolo-download-progress", serde_json::json!({
        "downloaded": total_size, "total": total_size, "variant": variant.model_name(), "status": "verifying"
    }));

    verify_sha256(&model_path, expected_sha256(&variant))?;

    let _ = app.emit("yolo-download-progress", serde_json::json!({
        "downloaded": total_size, "total": total_size, "variant": variant.model_name(), "status": "complete"
    }));

    {
        let mut engine = state.detection_engine.lock();
        engine.load_yolo(variant)?;
    }

    Ok("downloaded".to_string())
}

#[tauri::command]
pub async fn download_nudenet_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let model_path = {
        let engine = state.safety_engine.lock();
        engine.nudenet_model_path()
    };

    if model_path.exists() {
        return Ok("already_downloaded".to_string());
    }

    let url = "https://huggingface.co/vladmandic/nudenet/resolve/main/nudenet.onnx";
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;
    let total_size = response.content_length().unwrap_or(0);

    let _ = app.emit(
        "nudenet-download-progress",
        serde_json::json!({
            "downloaded": 0u64, "total": total_size, "status": "downloading"
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
        if downloaded % (512 * 1024) < chunk.len() as u64 || downloaded == total_size {
            let _ = app.emit(
                "nudenet-download-progress",
                serde_json::json!({
                    "downloaded": downloaded, "total": total_size, "status": "downloading"
                }),
            );
        }
    }

    let _ = app.emit(
        "nudenet-download-progress",
        serde_json::json!({
            "downloaded": total_size, "total": total_size, "status": "verifying"
        }),
    );

    verify_sha256(&model_path, NUDENET_EXPECTED_SHA256)?;

    let _ = app.emit(
        "nudenet-download-progress",
        serde_json::json!({
            "downloaded": total_size, "total": total_size, "status": "complete"
        }),
    );

    {
        let mut engine = state.safety_engine.lock();
        engine.load_nudenet()?;
    }

    Ok("downloaded".to_string())
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
