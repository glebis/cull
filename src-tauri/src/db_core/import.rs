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


#[derive(Debug)]
pub enum SyncOutcome {
    Unchanged,
    Restored,
    ContentChanged { image_id: String },
    NewImport { image_id: String },
    Registered,
    Skipped,
}

pub fn sync_file(
    db: &Database,
    file_path: &Path,
    app_data_dir: &Path,
) -> Result<SyncOutcome, String> {
    let ext = file_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !crate::extensions::supported_extensions(false).contains(&ext.as_str()) {
        return Ok(SyncOutcome::Skipped);
    }

    let metadata = fs::metadata(file_path).map_err(|e| format!("Stat error: {}", e))?;
    let file_size = metadata.len();
    let mtime = metadata.modified()
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
        .unwrap_or_default();

    let path_str = file_path.to_string_lossy().to_string();
    let can_decode = crate::extensions::is_decodable(&ext, false);

    if let Some(existing_file) = db.get_image_file_by_path(&path_str).map_err(|e| e.to_string())? {
        let size_match = existing_file.last_seen_size.map_or(false, |s| s == file_size);
        let mtime_match = existing_file.last_seen_mtime.as_deref().map_or(false, |m| m == mtime);

        if size_match && mtime_match && existing_file.missing_at.is_none() {
            let _ = db.touch_image_file(&existing_file.id, file_size, &mtime);
            return Ok(SyncOutcome::Unchanged);
        }

        if existing_file.missing_at.is_some() && size_match && mtime_match {
            let _ = db.touch_image_file(&existing_file.id, file_size, &mtime);
            return Ok(SyncOutcome::Restored);
        }

        let data = fs::read(file_path).map_err(|e| format!("Read error: {}", e))?;
        let hash = compute_hash(&data);

        if let Some(img) = db.find_by_hash(&hash).map_err(|e| e.to_string())? {
            if img.id == existing_file.image_id {
                let _ = db.touch_image_file(&existing_file.id, file_size, &mtime);
                return Ok(if existing_file.missing_at.is_some() {
                    SyncOutcome::Restored
                } else {
                    SyncOutcome::Unchanged
                });
            }
            let _ = db.repoint_image_file(&existing_file.id, &img.id, file_size, &mtime);
            if can_decode {
                let _ = thumbnails::generate_thumbnail(file_path, app_data_dir, &img.id);
            }
            return Ok(SyncOutcome::ContentChanged { image_id: img.id });
        }

        let image_id = create_image_record(db, file_path, &hash, &ext, &data, can_decode)?;
        let _ = db.repoint_image_file(&existing_file.id, &image_id, file_size, &mtime);
        if can_decode {
            let _ = thumbnails::generate_thumbnail(file_path, app_data_dir, &image_id);
            run_source_detection(db, file_path, &image_id, &ext);
            run_sidecar_detection(db, file_path, &image_id);
        }
        return Ok(SyncOutcome::ContentChanged { image_id });
    }

    // New path
    let data = fs::read(file_path).map_err(|e| format!("Read error: {}", e))?;
    let hash = compute_hash(&data);

    if let Some(existing_img) = db.find_by_hash(&hash).map_err(|e| e.to_string())? {
        let file_record = ImageFile {
            id: Uuid::new_v4().to_string(),
            image_id: existing_img.id.clone(),
            path: path_str,
            last_seen_at: Utc::now().to_rfc3339(),
            missing_at: None,
            last_seen_size: Some(file_size),
            last_seen_mtime: Some(mtime),
        };
        db.insert_image_file(&file_record).map_err(|e| e.to_string())?;
        return Ok(SyncOutcome::NewImport { image_id: existing_img.id });
    }

    let image_id = create_image_record(db, file_path, &hash, &ext, &data, can_decode)?;
    let file_record = ImageFile {
        id: Uuid::new_v4().to_string(),
        image_id: image_id.clone(),
        path: path_str,
        last_seen_at: Utc::now().to_rfc3339(),
        missing_at: None,
        last_seen_size: Some(file_size),
        last_seen_mtime: Some(mtime),
    };
    db.insert_image_file(&file_record).map_err(|e| e.to_string())?;

    if can_decode {
        let _ = thumbnails::generate_thumbnail(file_path, app_data_dir, &image_id);
        run_source_detection(db, file_path, &image_id, &ext);
        run_sidecar_detection(db, file_path, &image_id);
        Ok(SyncOutcome::NewImport { image_id })
    } else {
        Ok(SyncOutcome::Registered)
    }
}

pub fn import_file(
    db: &Database,
    file_path: &Path,
    app_data_dir: &Path,
) -> Result<Option<String>, String> {
    match sync_file(db, file_path, app_data_dir)? {
        SyncOutcome::NewImport { image_id } | SyncOutcome::ContentChanged { image_id } => Ok(Some(image_id)),
        _ => Ok(None),
    }
}

fn create_image_record(
    db: &Database,
    file_path: &Path,
    hash: &str,
    ext: &str,
    data: &[u8],
    can_decode: bool,
) -> Result<String, String> {
    let (width, height) = if can_decode {
        let img = image::open(file_path).map_err(|e| format!("Decode error: {}", e))?;
        (img.width(), img.height())
    } else {
        (0, 0)
    };

    let image_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let image = Image {
        id: image_id.clone(),
        sha256_hash: hash.to_string(),
        width,
        height,
        format: ext.to_string(),
        file_size: data.len() as u64,
        created_at: now.clone(),
        imported_at: now,
        ai_prompt: None,
    };
    db.insert_image(&image).map_err(|e| e.to_string())?;
    Ok(image_id)
}

fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn run_source_detection(db: &Database, file_path: &Path, image_id: &str, ext: &str) {
    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let png_chunks = if ext == "png" {
        read_png_text_chunks(file_path).unwrap_or_default()
    } else {
        vec![]
    };
    let detection = detect_source(filename, &png_chunks, file_path);

    let (width, height) = image::open(file_path)
        .map(|i| (i.width(), i.height()))
        .unwrap_or((0, 0));
    let aspect_ratio = width as f64 / height.max(1) as f64;
    let orientation = if (aspect_ratio - 1.0).abs() < 0.05 { "square" }
        else if aspect_ratio > 1.0 { "landscape" }
        else { "portrait" };
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;

    let _ = db.update_source_detection(
        image_id,
        detection.source_label.as_deref(),
        detection.confidence,
        &detection.to_evidence_json(),
        detection.is_ai_generated,
        detection.ai_prompt.as_deref(),
        aspect_ratio, orientation, megapixels,
    );
}

fn run_sidecar_detection(db: &Database, file_path: &Path, image_id: &str) {
    if let Some(sidecar_path) = sidecar::find_sidecar(file_path) {
        if let Ok(sc) = sidecar::parse_sidecar(&sidecar_path) {
            let run_id = Uuid::new_v4().to_string();
            let run = GenerationRun {
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
            let _ = db.link_image_to_run(image_id, &run_id);
        }
    }
}
