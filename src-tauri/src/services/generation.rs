use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;
use std::path::Path;
use sha2::{Sha256, Digest};
use tauri::Emitter;

use crate::db_core::db::Database;
use crate::db_core::models::GenerationRun;
use crate::services::jobs::JobRegistry;

pub struct ProviderConfig {
    pub base_url: &'static str,
    pub key_name: &'static str,
}

pub fn provider_config(provider: &str) -> Result<ProviderConfig, String> {
    match provider {
        "openai" => Ok(ProviderConfig {
            base_url: "https://api.openai.com/v1",
            key_name: "api_key_openai",
        }),
        "openrouter" => Ok(ProviderConfig {
            base_url: "https://openrouter.ai/api/v1",
            key_name: "api_key_openrouter",
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

const PRICING: &[(&str, &str, &str, f64)] = &[
    ("openai", "gpt-image-2", "1024x1024", 0.040),
    ("openai", "gpt-image-2", "1024x1536", 0.060),
    ("openai", "gpt-image-2", "1536x1024", 0.060),
    ("openai", "gpt-image-2", "auto", 0.040),
    ("openrouter", "openai/gpt-image-2", "1024x1024", 0.040),
    ("openrouter", "openai/gpt-image-2", "1024x1536", 0.060),
    ("openrouter", "openai/gpt-image-2", "1536x1024", 0.060),
    ("openrouter", "openai/gpt-image-2", "auto", 0.040),
];

pub fn estimate_cost(provider: &str, model: &str, size: &str, quality: &str, n: u8) -> f64 {
    let base = PRICING.iter()
        .find(|(p, m, s, _)| *p == provider && *m == model && *s == size)
        .map(|(_, _, _, price)| *price)
        .unwrap_or(0.040);
    let multiplier = if quality == "high" { 2.0 } else { 1.0 };
    base * multiplier * n as f64
}

pub async fn generate_images(
    request: &GenerationRequest,
    api_key: &str,
    base_url: &str,
    app_data_dir: &Path,
    db: &Database,
    jobs: &JobRegistry,
    job_id: &str,
    cancel: &tokio_util::sync::CancellationToken,
    app_handle: &tauri::AppHandle,
) -> Result<GenerationResult, String> {
    let _ = app_handle.emit("job-status-changed", serde_json::json!({
        "job_id": job_id,
        "kind": "generation",
        "status": "running",
        "current": 0,
        "total": request.n,
    }));

    let generated_dir = app_data_dir.join("generated");
    std::fs::create_dir_all(&generated_dir).map_err(|e| format!("Dir create error: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;
    let resp = client
        .post(&format!("{}/images/generations", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": &request.model,
            "prompt": &request.prompt,
            "n": request.n,
            "size": &request.size,
            "quality": &request.quality,
        }))
        .send()
        .await
        .map_err(|e| {
            jobs.fail(job_id, &e.to_string());
            let _ = app_handle.emit("job-status-changed", serde_json::json!({
                "job_id": job_id, "kind": "generation", "status": "failed",
            }));
            format!("API request failed: {}", e)
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let msg = format!("OpenAI API error {}: {}", status, body);
        jobs.fail(job_id, &msg);
        let _ = app_handle.emit("job-status-changed", serde_json::json!({
            "job_id": job_id, "kind": "generation", "status": "failed",
        }));
        return Err(msg);
    }

    let resp_body = resp.text().await
        .map_err(|e| {
            let msg = format!("Read error: {}", e);
            jobs.fail(job_id, &msg);
            let _ = app_handle.emit("job-status-changed", serde_json::json!({
                "job_id": job_id, "kind": "generation", "status": "failed",
            }));
            msg
        })?;
    let api_resp: OpenAiImageResponse = serde_json::from_str(&resp_body)
        .map_err(|e| {
            let msg = format!("Parse error: {}", e);
            jobs.fail(job_id, &msg);
            let _ = app_handle.emit("job-status-changed", serde_json::json!({
                "job_id": job_id, "kind": "generation", "status": "failed",
            }));
            msg
        })?;

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

    for (i, item) in api_resp.data.iter().enumerate() {
        if cancel.is_cancelled() {
            jobs.mark_cancelled(job_id);
            let _ = app_handle.emit("job-status-changed", serde_json::json!({
                "job_id": job_id, "kind": "generation", "status": "cancelled",
            }));
            break;
        }

        match save_generated_image(item, i, request, &generated_dir, db, parent_run_id.as_deref(), &resp_body) {
            Ok((image_id, run_id)) => {
                image_ids.push(image_id);
                run_ids.push(run_id);
            }
            Err(e) => errors.push(format!("Image {}: {}", i, e)),
        }

        jobs.update_progress(job_id, (i + 1) as u32, Some(&format!("Saved image {}/{}", i + 1, request.n)));
        let _ = app_handle.emit("generation-progress", serde_json::json!({
            "job_id": job_id,
            "current": i + 1,
            "total": request.n,
        }));
    }

    let lineage_group_id = if image_ids.len() > 1 || request.source_image_id.is_some() {
        create_generation_lineage(db, &image_ids, request.source_image_id.as_deref(), &request.prompt).ok()
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

    let status_str = if errors.is_empty() { "completed" } else if image_ids.is_empty() { "failed" } else { "completed" };
    let _ = app_handle.emit("job-status-changed", serde_json::json!({
        "job_id": job_id,
        "kind": "generation",
        "status": status_str,
        "current": image_ids.len(),
        "total": request.n,
    }));

    let _ = app_handle.emit("generation-complete", serde_json::json!({
        "job_id": job_id,
        "image_ids": &image_ids,
        "lineage_group_id": &lineage_group_id,
    }));

    Ok(GenerationResult {
        job_id: job_id.to_string(),
        image_ids,
        generation_run_ids: run_ids,
        lineage_group_id,
        errors,
    })
}

fn save_generated_image(
    item: &OpenAiImageData,
    index: usize,
    request: &GenerationRequest,
    generated_dir: &Path,
    db: &Database,
    parent_run_id: Option<&str>,
    raw_api_response: &str,
) -> Result<(String, String), String> {
    let b64 = item.b64_json.as_deref()
        .ok_or("No b64_json in response")?;

    let bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD, b64
    ).map_err(|e| format!("Base64 decode error: {}", e))?;

    let image_id = Uuid::new_v4().to_string();
    let filename = format!("{}_{}.png", &image_id[..8], index);
    let file_path = generated_dir.join(&filename);

    std::fs::write(&file_path, &bytes).map_err(|e| format!("Write error: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
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
    };
    db.insert_image(&image_record).map_err(|e| e.to_string())?;

    let file_record = crate::db_core::models::ImageFile {
        id: Uuid::new_v4().to_string(),
        image_id: image_id.clone(),
        path: file_path.to_string_lossy().to_string(),
        last_seen_at: now.clone(),
        missing_at: None,
    };
    db.insert_image_file(&file_record).map_err(|e| e.to_string())?;

    let aspect = width as f64 / height.max(1) as f64;
    let orientation = if (aspect - 1.0).abs() < 0.05 { "square" }
        else if aspect > 1.0 { "landscape" } else { "portrait" };
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;
    db.update_source_detection(
        &image_id, Some(&request.provider), 100.0,
        "{\"source\":\"openai_api_generation\"}", Some(true),
        Some(&request.prompt), aspect, orientation, megapixels,
    ).map_err(|e| e.to_string())?;

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
        source_type: "openai_api".to_string(),
        source_path: Some(file_path.to_string_lossy().to_string()),
        raw_metadata_json: Some(raw_api_response.to_string()),
        created_at: Some(now.clone()),
        imported_at: now,
    };
    db.insert_generation_run(&run).map_err(|e| e.to_string())?;
    db.link_image_to_run(&image_id, &run_id).map_err(|e| e.to_string())?;

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
        let end = prompt.char_indices().nth(40).map(|(i, _)| i).unwrap_or(prompt.len());
        &prompt[..end]
    } else {
        prompt
    };
    let name = format!("Gen: {}", truncated);
    let group_id = db.create_lineage_group(&name, "generation", 100.0)
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
