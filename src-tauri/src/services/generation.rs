// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::future::Future;
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::Emitter;
use uuid::Uuid;

use crate::db_core::db::Database;
use crate::db_core::models::GenerationRun;
use crate::services::jobs::JobRegistry;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiStyle {
    OpenAi,
    Gemini,
}

pub struct ProviderConfig {
    pub base_url: &'static str,
    pub key_name: &'static str,
    pub api_style: ApiStyle,
}

pub fn provider_config(provider: &str) -> Result<ProviderConfig, String> {
    match provider {
        "openai" => Ok(ProviderConfig {
            base_url: "https://api.openai.com/v1",
            key_name: "api_key_openai",
            api_style: ApiStyle::OpenAi,
        }),
        "openrouter" => Ok(ProviderConfig {
            base_url: "https://openrouter.ai/api/v1",
            key_name: "api_key_openrouter",
            api_style: ApiStyle::OpenAi,
        }),
        "google" => Ok(ProviderConfig {
            base_url: "https://generativelanguage.googleapis.com/v1beta",
            key_name: "api_key_google",
            api_style: ApiStyle::Gemini,
        }),
        _ => Err(format!("Unknown provider: {}", provider)),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub provider: String,
    pub source_image_id: Option<String>,
    pub prompt: String,
    pub n: u8,
    pub model: String,
    pub size: String,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub job_id: String,
    pub image_ids: Vec<String>,
    pub generation_run_ids: Vec<String>,
    pub lineage_group_id: Option<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageResponse {
    data: Vec<OpenAiImageData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageData {
    b64_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}
#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}
#[derive(Debug, Deserialize)]
struct GeminiContent {
    parts: Option<Vec<GeminiPart>>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiPart {
    inline_data: Option<GeminiInlineData>,
    #[serde(default)]
    #[allow(dead_code)]
    text: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineData {
    data: String,
    #[allow(dead_code)]
    mime_type: String,
}

const PRICING: &[(&str, &str, &str, f64)] = &[
    // OpenAI direct
    ("openai", "gpt-image-2", "1024x1024", 0.040),
    ("openai", "gpt-image-2", "1024x1536", 0.060),
    ("openai", "gpt-image-2", "1536x1024", 0.060),
    ("openai", "gpt-image-2", "auto", 0.040),
    // OpenRouter (OpenAI-compatible models only)
    ("openrouter", "openai/gpt-image-2", "1024x1024", 0.040),
    ("openrouter", "openai/gpt-image-2", "auto", 0.040),
    ("openrouter", "openai/gpt-5-image", "auto", 0.040),
    ("openrouter", "openai/gpt-5-image-mini", "auto", 0.020),
    // Google direct
    ("google", "gemini-2.5-flash-image", "auto", 0.039),
    ("google", "gemini-3-pro-image-preview", "auto", 0.039),
];

pub fn estimate_cost(provider: &str, model: &str, size: &str, quality: &str, n: u8) -> f64 {
    let base = PRICING
        .iter()
        .find(|(p, m, s, _)| *p == provider && *m == model && *s == size)
        .or_else(|| {
            PRICING
                .iter()
                .find(|(p, m, _, _)| *p == provider && *m == model)
        })
        .map(|(_, _, _, price)| *price)
        .unwrap_or(0.040);
    let multiplier = if provider == "openai" && quality == "high" {
        2.0
    } else {
        1.0
    };
    base * multiplier * n as f64
}

fn generation_provider_label(provider: &str) -> &'static str {
    match provider {
        "openai" => "OpenAI",
        "openrouter" => "OpenRouter",
        "google" => "Google",
        _ => "provider",
    }
}

fn generation_wait_message(provider: &str, elapsed_secs: u64) -> String {
    let label = generation_provider_label(provider);
    if elapsed_secs == 0 {
        format!("Waiting for {}", label)
    } else {
        format!("Waiting for {} ({}s)", label, elapsed_secs)
    }
}

fn generation_progress_payload(
    job_id: &str,
    current: u32,
    total: u8,
    message: &str,
) -> serde_json::Value {
    serde_json::json!({
        "job_id": job_id,
        "current": current,
        "total": total,
        "message": message,
    })
}

fn set_generation_progress(
    jobs: &JobRegistry,
    app_handle: &tauri::AppHandle,
    job_id: &str,
    current: u32,
    total: u8,
    message: &str,
) {
    jobs.update_progress(job_id, current, Some(message));
    let _ = app_handle.emit(
        "generation-progress",
        generation_progress_payload(job_id, current, total, message),
    );
}

async fn with_generation_heartbeat<F, T>(
    future: F,
    jobs: &JobRegistry,
    app_handle: &tauri::AppHandle,
    job_id: &str,
    request: &GenerationRequest,
    current: u32,
) -> T
where
    F: Future<Output = T>,
{
    let mut future = std::pin::pin!(future);
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;
    let started_at = Instant::now();

    loop {
        tokio::select! {
            result = &mut future => return result,
            _ = interval.tick() => {
                set_generation_progress(
                    jobs,
                    app_handle,
                    job_id,
                    current,
                    request.n,
                    &generation_wait_message(&request.provider, started_at.elapsed().as_secs()),
                );
            }
        }
    }
}

fn extract_images_openai(resp_body: &str) -> Result<Vec<Vec<u8>>, String> {
    let api_resp: OpenAiImageResponse =
        serde_json::from_str(resp_body).map_err(|e| format!("Parse error: {}", e))?;
    let mut images = Vec::new();
    for item in &api_resp.data {
        let b64 = item.b64_json.as_deref().ok_or("No b64_json in response")?;
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .map_err(|e| format!("Base64 decode error: {}", e))?;
        images.push(bytes);
    }
    Ok(images)
}

fn extract_image_gemini(resp_body: &str) -> Result<Vec<u8>, String> {
    let resp: GeminiResponse =
        serde_json::from_str(resp_body).map_err(|e| format!("Parse error: {}", e))?;
    let candidates = resp.candidates.ok_or("No candidates in response")?;
    let candidate = candidates.first().ok_or("Empty candidates")?;
    let content = candidate.content.as_ref().ok_or("No content")?;
    let parts = content.parts.as_ref().ok_or("No parts")?;
    for part in parts {
        if let Some(ref data) = part.inline_data {
            let bytes =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data.data)
                    .map_err(|e| format!("Base64 decode: {}", e))?;
            return Ok(bytes);
        }
    }
    Err("No image data in Gemini response".to_string())
}

#[expect(
    clippy::too_many_arguments,
    reason = "generation orchestration depends on explicit service handles and request context"
)]
pub async fn generate_images(
    request: &GenerationRequest,
    api_key: &str,
    base_url: &str,
    api_style: ApiStyle,
    app_data_dir: &Path,
    db: &Database,
    jobs: &JobRegistry,
    job_id: &str,
    cancel: &tokio_util::sync::CancellationToken,
    app_handle: &tauri::AppHandle,
) -> Result<GenerationResult, String> {
    let initial_message = format!(
        "Preparing {} request",
        generation_provider_label(&request.provider)
    );
    jobs.update_progress(job_id, 0, Some(&initial_message));
    let _ = app_handle.emit(
        "job-status-changed",
        serde_json::json!({
            "job_id": job_id,
            "kind": "generation",
            "status": "running",
            "current": 0,
            "total": request.n,
            "message": initial_message,
        }),
    );

    let generated_dir = app_data_dir.join("generated");
    std::fs::create_dir_all(&generated_dir).map_err(|e| format!("Dir create error: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let parent_run_id = if let Some(ref src_id) = request.source_image_id {
        db.get_generation_run_for_image(src_id)
            .ok()
            .flatten()
            .map(|r| r.id)
    } else {
        None
    };

    let mut image_ids = Vec::new();
    let mut run_ids = Vec::new();
    let mut errors = Vec::new();

    match api_style {
        ApiStyle::OpenAi => {
            set_generation_progress(
                jobs,
                app_handle,
                job_id,
                0,
                request.n,
                &generation_wait_message(&request.provider, 0),
            );
            let resp = with_generation_heartbeat(
                client
                    .post(format!("{}/images/generations", base_url))
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&serde_json::json!({
                        "model": &request.model,
                        "prompt": &request.prompt,
                        "n": request.n,
                        "size": &request.size,
                        "quality": &request.quality,
                    }))
                    .send(),
                jobs,
                app_handle,
                job_id,
                request,
                0,
            )
            .await
            .map_err(|e| {
                jobs.fail(job_id, &e.to_string());
                let _ = app_handle.emit(
                    "job-status-changed",
                    serde_json::json!({
                        "job_id": job_id, "kind": "generation", "status": "failed",
                    }),
                );
                format!("API request failed: {}", e)
            })?;

            let resp_status = resp.status();
            let jurisdiction = match request.provider.as_str() {
                "openai" => "US - OpenAI Inc",
                "openrouter" => "US - OpenRouter (proxy)",
                _ => "Unknown",
            };

            if !resp_status.is_success() {
                let _ = crate::services::audit::log_api_call(
                    db,
                    &request.provider,
                    &format!("{}/images/generations", base_url),
                    if request.source_image_id.is_some() {
                        "prompt+image"
                    } else {
                        "prompt"
                    },
                    request.prompt.len() as i64,
                    Some(&request.prompt),
                    None,
                    Some(&request.model),
                    resp_status.as_u16() as i32,
                    jurisdiction,
                );
                let body = resp.text().await.unwrap_or_default();
                let msg = format!("API error {}: {}", resp_status, body);
                jobs.fail(job_id, &msg);
                let _ = app_handle.emit(
                    "job-status-changed",
                    serde_json::json!({
                        "job_id": job_id, "kind": "generation", "status": "failed",
                    }),
                );
                return Err(msg);
            }

            set_generation_progress(
                jobs,
                app_handle,
                job_id,
                0,
                request.n,
                "Receiving generation response",
            );
            let _ = crate::services::audit::log_api_call(
                db,
                &request.provider,
                &format!("{}/images/generations", base_url),
                if request.source_image_id.is_some() {
                    "prompt+image"
                } else {
                    "prompt"
                },
                request.prompt.len() as i64,
                Some(&request.prompt),
                None,
                Some(&request.model),
                200,
                jurisdiction,
            );

            let resp_body =
                with_generation_heartbeat(resp.text(), jobs, app_handle, job_id, request, 0)
                    .await
                    .map_err(|e| {
                        let msg = format!("Read error: {}", e);
                        jobs.fail(job_id, &msg);
                        let _ = app_handle.emit(
                            "job-status-changed",
                            serde_json::json!({
                                "job_id": job_id, "kind": "generation", "status": "failed",
                            }),
                        );
                        msg
                    })?;

            set_generation_progress(
                jobs,
                app_handle,
                job_id,
                0,
                request.n,
                "Decoding generated images",
            );
            let decoded_images = extract_images_openai(&resp_body).inspect_err(|e| {
                jobs.fail(job_id, e);
                let _ = app_handle.emit(
                    "job-status-changed",
                    serde_json::json!({
                        "job_id": job_id, "kind": "generation", "status": "failed",
                    }),
                );
            })?;

            for (i, bytes) in decoded_images.iter().enumerate() {
                if cancel.is_cancelled() {
                    jobs.mark_cancelled(job_id);
                    let _ = app_handle.emit(
                        "job-status-changed",
                        serde_json::json!({
                            "job_id": job_id, "kind": "generation", "status": "cancelled",
                        }),
                    );
                    break;
                }

                set_generation_progress(
                    jobs,
                    app_handle,
                    job_id,
                    i as u32,
                    request.n,
                    &format!("Saving image {}/{}", i + 1, request.n),
                );
                match save_image_bytes(
                    bytes,
                    i,
                    request,
                    &generated_dir,
                    db,
                    parent_run_id.as_deref(),
                    &resp_body,
                ) {
                    Ok((image_id, run_id)) => {
                        image_ids.push(image_id);
                        run_ids.push(run_id);
                    }
                    Err(e) => errors.push(format!("Image {}: {}", i, e)),
                }

                set_generation_progress(
                    jobs,
                    app_handle,
                    job_id,
                    (i + 1) as u32,
                    request.n,
                    &format!("Saved image {}/{}", i + 1, request.n),
                );
            }
        }
        ApiStyle::Gemini => {
            for i in 0..request.n as usize {
                if cancel.is_cancelled() {
                    jobs.mark_cancelled(job_id);
                    let _ = app_handle.emit(
                        "job-status-changed",
                        serde_json::json!({
                            "job_id": job_id, "kind": "generation", "status": "cancelled",
                        }),
                    );
                    break;
                }

                set_generation_progress(
                    jobs,
                    app_handle,
                    job_id,
                    i as u32,
                    request.n,
                    &format!(
                        "Waiting for {} image {}/{}",
                        generation_provider_label(&request.provider),
                        i + 1,
                        request.n
                    ),
                );
                let url = format!("{}/models/{}:generateContent", base_url, request.model);
                let mut parts = vec![serde_json::json!({"text": &request.prompt})];
                if let Some(ref src_id) = request.source_image_id {
                    if let Ok(images) = db.get_images_by_ids(&[src_id.as_str()]) {
                        if let Some(src_img) = images.first() {
                            if let Ok(img_bytes) = std::fs::read(&src_img.path) {
                                let b64 = base64::Engine::encode(
                                    &base64::engine::general_purpose::STANDARD,
                                    &img_bytes,
                                );
                                let mime = if src_img.path.ends_with(".png") {
                                    "image/png"
                                } else {
                                    "image/jpeg"
                                };
                                parts.push(serde_json::json!({
                                    "inlineData": {"mimeType": mime, "data": b64}
                                }));
                            }
                        }
                    }
                }
                let payload = serde_json::json!({
                    "contents": [{"parts": parts}],
                    "generationConfig": {"responseModalities": ["TEXT", "IMAGE"]},
                });

                let resp = with_generation_heartbeat(
                    client
                        .post(&url)
                        .header("Content-Type", "application/json")
                        .header("x-goog-api-key", api_key)
                        .json(&payload)
                        .send(),
                    jobs,
                    app_handle,
                    job_id,
                    request,
                    i as u32,
                )
                .await;

                match resp {
                    Ok(r) => {
                        let gemini_status = r.status();
                        if !gemini_status.is_success() {
                            let _ = crate::services::audit::log_api_call(
                                db,
                                "google",
                                &url,
                                if request.source_image_id.is_some() {
                                    "prompt+image"
                                } else {
                                    "prompt"
                                },
                                request.prompt.len() as i64,
                                Some(&request.prompt),
                                None,
                                Some(&request.model),
                                gemini_status.as_u16() as i32,
                                "US - Google LLC",
                            );
                            let body = r.text().await.unwrap_or_default();
                            errors.push(format!(
                                "Image {}: Gemini API error {}: {}",
                                i, gemini_status, body
                            ));
                            continue;
                        }
                        set_generation_progress(
                            jobs,
                            app_handle,
                            job_id,
                            i as u32,
                            request.n,
                            &format!("Receiving image {}/{}", i + 1, request.n),
                        );
                        let _ = crate::services::audit::log_api_call(
                            db,
                            "google",
                            &url,
                            if request.source_image_id.is_some() {
                                "prompt+image"
                            } else {
                                "prompt"
                            },
                            request.prompt.len() as i64,
                            Some(&request.prompt),
                            None,
                            Some(&request.model),
                            200,
                            "US - Google LLC",
                        );
                        match with_generation_heartbeat(
                            r.text(),
                            jobs,
                            app_handle,
                            job_id,
                            request,
                            i as u32,
                        )
                        .await
                        {
                            Ok(resp_body) => match extract_image_gemini(&resp_body) {
                                Ok(bytes) => {
                                    set_generation_progress(
                                        jobs,
                                        app_handle,
                                        job_id,
                                        i as u32,
                                        request.n,
                                        &format!("Saving image {}/{}", i + 1, request.n),
                                    );
                                    match save_image_bytes(
                                        &bytes,
                                        i,
                                        request,
                                        &generated_dir,
                                        db,
                                        parent_run_id.as_deref(),
                                        &resp_body,
                                    ) {
                                        Ok((image_id, run_id)) => {
                                            image_ids.push(image_id);
                                            run_ids.push(run_id);
                                        }
                                        Err(e) => errors.push(format!("Image {}: {}", i, e)),
                                    }
                                }
                                Err(e) => errors.push(format!("Image {}: {}", i, e)),
                            },
                            Err(e) => errors.push(format!("Image {}: Read error: {}", i, e)),
                        }
                    }
                    Err(e) => errors.push(format!("Image {}: Request failed: {}", i, e)),
                }

                set_generation_progress(
                    jobs,
                    app_handle,
                    job_id,
                    (i + 1) as u32,
                    request.n,
                    &format!("Saved image {}/{}", i + 1, request.n),
                );
            }
        }
    }

    let lineage_group_id = if image_ids.len() > 1 || request.source_image_id.is_some() {
        create_generation_lineage(
            db,
            &image_ids,
            request.source_image_id.as_deref(),
            &request.prompt,
        )
        .ok()
    } else {
        None
    };

    if errors.is_empty() {
        jobs.complete(job_id);
    } else if image_ids.is_empty() {
        jobs.fail(job_id, &errors.join("; "));
    } else {
        jobs.complete(job_id);
    }

    let status_str = if errors.is_empty() {
        "completed"
    } else if image_ids.is_empty() {
        "failed"
    } else {
        "completed"
    };
    let _ = app_handle.emit(
        "job-status-changed",
        serde_json::json!({
            "job_id": job_id,
            "kind": "generation",
            "status": status_str,
            "current": image_ids.len(),
            "total": request.n,
        }),
    );

    let _ = app_handle.emit(
        "generation-complete",
        serde_json::json!({
            "job_id": job_id,
            "image_ids": &image_ids,
            "lineage_group_id": &lineage_group_id,
        }),
    );

    Ok(GenerationResult {
        job_id: job_id.to_string(),
        image_ids,
        generation_run_ids: run_ids,
        lineage_group_id,
        errors,
    })
}

fn save_image_bytes(
    bytes: &[u8],
    index: usize,
    request: &GenerationRequest,
    generated_dir: &Path,
    db: &Database,
    parent_run_id: Option<&str>,
    raw_api_response: &str,
) -> Result<(String, String), String> {
    let image_id = Uuid::new_v4().to_string();
    let filename = format!("{}_{}.png", &image_id[..8], index);
    let file_path = generated_dir.join(&filename);

    std::fs::write(&file_path, bytes).map_err(|e| format!("Write error: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash = format!("{:x}", hasher.finalize());

    let img = image::open(&file_path).map_err(|e| format!("Image decode error: {}", e))?;
    let (width, height) = (img.width(), img.height());

    let now = Utc::now().to_rfc3339();
    let image_record = crate::db_core::models::Image {
        id: image_id.clone(),
        sha256_hash: hash,
        width,
        height,
        format: "png".to_string(),
        file_size: bytes.len() as u64,
        created_at: now.clone(),
        imported_at: now.clone(),
        ai_prompt: Some(request.prompt.clone()),
        raw_metadata: None,
    };
    db.insert_image(&image_record).map_err(|e| e.to_string())?;

    let file_record = crate::db_core::models::ImageFile {
        id: Uuid::new_v4().to_string(),
        image_id: image_id.clone(),
        path: file_path.to_string_lossy().to_string(),
        last_seen_at: now.clone(),
        missing_at: None,
        last_seen_size: None,
        last_seen_mtime: None,
    };
    db.insert_image_file(&file_record)
        .map_err(|e| e.to_string())?;

    let aspect = width as f64 / height.max(1) as f64;
    let orientation = if (aspect - 1.0).abs() < 0.05 {
        "square"
    } else if aspect > 1.0 {
        "landscape"
    } else {
        "portrait"
    };
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;
    db.update_source_detection(
        &image_id,
        Some(&request.provider),
        100.0,
        &format!("{{\"source\":\"{}_api_generation\"}}", request.provider),
        Some(true),
        Some(&request.prompt),
        aspect,
        orientation,
        megapixels,
    )
    .map_err(|e| e.to_string())?;

    let run_id = Uuid::new_v4().to_string();
    let settings = serde_json::json!({
        "n": request.n,
        "size": &request.size,
        "quality": &request.quality,
        "variation_index": index,
        "estimated_cost": estimate_cost(&request.provider, &request.model, &request.size, &request.quality, 1),
    });
    let run = GenerationRun {
        id: run_id.clone(),
        prompt: Some(request.prompt.clone()),
        negative_prompt: None,
        provider: Some(request.provider.clone()),
        model: Some(request.model.clone()),
        settings_json: settings.to_string(),
        seed: None,
        parent_run_id: parent_run_id.map(|s| s.to_string()),
        source_type: format!("{}_api", request.provider),
        source_path: Some(file_path.to_string_lossy().to_string()),
        raw_metadata_json: Some(raw_api_response.to_string()),
        created_at: Some(now.clone()),
        imported_at: now,
    };
    db.insert_generation_run(&run).map_err(|e| e.to_string())?;
    db.link_image_to_run(&image_id, &run_id)
        .map_err(|e| e.to_string())?;

    let _ = crate::db_core::thumbnails::generate_thumbnail(
        &file_path,
        generated_dir.parent().unwrap_or(generated_dir),
        &image_id,
    );

    Ok((image_id, run_id))
}

fn create_generation_lineage(
    db: &Database,
    new_image_ids: &[String],
    source_image_id: Option<&str>,
    prompt: &str,
) -> Result<String, String> {
    let truncated: &str = if prompt.chars().count() > 40 {
        let end = prompt
            .char_indices()
            .nth(40)
            .map(|(i, _)| i)
            .unwrap_or(prompt.len());
        &prompt[..end]
    } else {
        prompt
    };
    let name = format!("Gen: {}", truncated);
    let group_id = db
        .create_lineage_group(&name, "generation", 100.0)
        .map_err(|e| e.to_string())?;

    let mut order = 0;
    if let Some(src_id) = source_image_id {
        db.assign_to_lineage_group(src_id, &group_id, order)
            .map_err(|e| e.to_string())?;
        order += 1;
    }

    for id in new_image_ids {
        db.assign_to_lineage_group(id, &group_id, order)
            .map_err(|e| e.to_string())?;
        order += 1;
    }

    Ok(group_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_wait_message_uses_provider_name_and_elapsed_time() {
        assert_eq!(generation_wait_message("openai", 0), "Waiting for OpenAI");
        assert_eq!(
            generation_wait_message("openrouter", 12),
            "Waiting for OpenRouter (12s)"
        );
        assert_eq!(
            generation_wait_message("google", 65),
            "Waiting for Google (65s)"
        );
    }

    #[test]
    fn generation_progress_payload_includes_stage_message() {
        let payload = generation_progress_payload("job_test", 0, 2, "Waiting for OpenAI");

        assert_eq!(payload["job_id"], "job_test");
        assert_eq!(payload["current"], 0);
        assert_eq!(payload["total"], 2);
        assert_eq!(payload["message"], "Waiting for OpenAI");
    }
}
