use sha2::{Sha256, Digest};
use std::fs;
use std::path::Path;
use chrono::Utc;
use uuid::Uuid;

use super::db::Database;
use super::models::*;
use super::thumbnails;
use super::source_detection::{detect_source, read_png_text_chunks};
use super::sidecar;

pub fn import_file(
    db: &Database,
    file_path: &Path,
    app_data_dir: &Path,
) -> Result<Option<String>, String> {
    let ext = file_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !["jpg", "jpeg", "png", "webp", "gif"].contains(&ext.as_str()) {
        return Ok(None);
    }

    let data = fs::read(file_path).map_err(|e| format!("Read error: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = format!("{:x}", hasher.finalize());

    if let Some(existing) = db.find_by_hash(&hash).map_err(|e| e.to_string())? {
        let file_record = ImageFile {
            id: Uuid::new_v4().to_string(),
            image_id: existing.id.clone(),
            path: file_path.to_string_lossy().to_string(),
            last_seen_at: Utc::now().to_rfc3339(),
            missing_at: None,
        };
        db.insert_image_file(&file_record).map_err(|e| e.to_string())?;
        return Ok(None);
    }

    let img = image::open(file_path).map_err(|e| format!("Image decode error: {}", e))?;
    let (width, height) = (img.width(), img.height());
    let file_size = data.len() as u64;

    let image_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let image = Image {
        id: image_id.clone(),
        sha256_hash: hash,
        width,
        height,
        format: ext.clone(),
        file_size,
        created_at: now.clone(),
        imported_at: now.clone(),
        ai_prompt: None,
    };

    db.insert_image(&image).map_err(|e| e.to_string())?;

    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let png_chunks = if ext == "png" {
        read_png_text_chunks(file_path).unwrap_or_default()
    } else {
        vec![]
    };
    let detection = detect_source(filename, &png_chunks, file_path);

    // Check for sidecar JSON and create generation_run
    if let Some(sidecar_path) = sidecar::find_sidecar(file_path) {
        if let Ok(sc) = sidecar::parse_sidecar(&sidecar_path) {
            let run_id = Uuid::new_v4().to_string();
            let run = super::models::GenerationRun {
                id: run_id.clone(),
                prompt: sc.prompt.clone(),
                negative_prompt: sc.negative_prompt,
                provider: sc.provider,
                model: sc.model,
                settings_json: sc.settings_json,
                seed: sc.seed,
                parent_run_id: None,
                source_type: "sidecar".to_string(),
                source_path: Some(sidecar_path.to_string_lossy().to_string()),
                raw_metadata_json: Some(sc.raw_json),
                created_at: sc.created_at,
                imported_at: Utc::now().to_rfc3339(),
            };
            let _ = db.insert_generation_run(&run);
            let _ = db.link_image_to_run(&image_id, &run_id);
        }
    }

    let aspect_ratio = width as f64 / height.max(1) as f64;
    let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
        "square"
    } else if aspect_ratio > 1.0 {
        "landscape"
    } else {
        "portrait"
    };
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;

    db.update_source_detection(
        &image_id,
        detection.source_label.as_deref(),
        detection.confidence,
        &detection.to_evidence_json(),
        detection.is_ai_generated,
        detection.ai_prompt.as_deref(),
        aspect_ratio,
        orientation,
        megapixels,
    ).map_err(|e| e.to_string())?;

    let file_record = ImageFile {
        id: Uuid::new_v4().to_string(),
        image_id: image_id.clone(),
        path: file_path.to_string_lossy().to_string(),
        last_seen_at: Utc::now().to_rfc3339(),
        missing_at: None,
    };
    db.insert_image_file(&file_record).map_err(|e| e.to_string())?;

    thumbnails::generate_thumbnail(file_path, app_data_dir, &image_id)?;

    Ok(Some(image_id))
}

