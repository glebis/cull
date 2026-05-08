use sha2::{Sha256, Digest};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::Utc;
use uuid::Uuid;

use super::db::Database;
use super::models::*;
use super::thumbnails;
use super::source_detection::{detect_source, read_png_text_chunks};

pub struct ImportResult {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
}

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
    };

    db.insert_image(&image).map_err(|e| e.to_string())?;

    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let png_chunks = if ext == "png" {
        read_png_text_chunks(file_path).unwrap_or_default()
    } else {
        vec![]
    };
    let detection = detect_source(filename, &png_chunks);

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

pub fn import_folder(
    db: &Database,
    folder_path: &Path,
    app_data_dir: &Path,
) -> ImportResult {
    let mut result = ImportResult { imported: 0, skipped: 0, errors: vec![] };

    let entries: Vec<PathBuf> = walkdir::WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    for path in entries {
        match import_file(db, &path, app_data_dir) {
            Ok(Some(_)) => result.imported += 1,
            Ok(None) => result.skipped += 1,
            Err(e) => result.errors.push(format!("{}: {}", path.display(), e)),
        }
    }

    result
}
