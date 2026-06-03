use chrono::Utc;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use uuid::Uuid;

use super::color;
use super::db::Database;
use super::models::*;
use super::perceptual_hash;
use super::sidecar;
use super::source_detection::{detect_source, read_png_text_chunks};
use super::thumbnails;

fn is_module_raw_enabled(db: &Database) -> bool {
    db.get_setting("module_raw")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false)
}

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
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let module_raw = is_module_raw_enabled(db);
    if !crate::extensions::supported_extensions(module_raw).contains(&ext.as_str()) {
        return Ok(SyncOutcome::Skipped);
    }

    let metadata = fs::metadata(file_path).map_err(|e| format!("Stat error: {}", e))?;
    let file_size = metadata.len();

    // Refuse to read pathologically large files into memory; skip rather than
    // risk a memory cliff during normal import.
    if !import_size_within_limit(file_size) {
        crate::safe_eprintln!(
            "[import] skipping oversized file ({} bytes > {} limit): {}",
            file_size,
            MAX_IMPORT_FILE_BYTES,
            file_path.display()
        );
        return Ok(SyncOutcome::Skipped);
    }
    let mtime = metadata
        .modified()
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
        .unwrap_or_default();

    let path_str = file_path.to_string_lossy().to_string();
    let can_decode = crate::extensions::is_decodable(&ext, module_raw);

    if let Some(existing_file) = db
        .get_image_file_by_path(&path_str)
        .map_err(|e| e.to_string())?
    {
        let size_match = existing_file
            .last_seen_size
            .map_or(false, |s| s == file_size);
        let mtime_match = existing_file
            .last_seen_mtime
            .as_deref()
            .map_or(false, |m| m == mtime);

        if size_match && mtime_match && existing_file.missing_at.is_none() {
            let _ = db.touch_image_file(&existing_file.id, file_size, &mtime);
            return Ok(SyncOutcome::Unchanged);
        }

        if existing_file.missing_at.is_some() && size_match && mtime_match {
            let _ = db.touch_image_file(&existing_file.id, file_size, &mtime);
            return Ok(SyncOutcome::Restored);
        }

        let hash = hash_file(file_path).map_err(|e| format!("Read error: {}", e))?;

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
                match crate::db_core::image_decode::decode_image(file_path, module_raw) {
                    Ok(decoded) => {
                        let _ = thumbnails::generate_thumbnail_from_image(
                            &decoded.image,
                            app_data_dir,
                            &img.id,
                        );
                    }
                    Err(e) => {
                        crate::safe_eprintln!("Thumbnail decode failed for {}: {}", path_str, e)
                    }
                }
            }
            return Ok(SyncOutcome::ContentChanged { image_id: img.id });
        }

        // New content: only now read the bytes (source detection / decode need them).
        let data = fs::read(file_path).map_err(|e| format!("Read error: {}", e))?;
        let (image_id, decoded) =
            create_image_record(db, file_path, &hash, &ext, &data, can_decode, module_raw)?;
        let _ = db.repoint_image_file(&existing_file.id, &image_id, file_size, &mtime);
        if can_decode {
            let decoded_dims = decoded
                .as_ref()
                .map(|d| (d.image.width(), d.image.height()));
            if let Some(decoded) = &decoded {
                let _ = thumbnails::generate_thumbnail_from_image(
                    &decoded.image,
                    app_data_dir,
                    &image_id,
                );
                if let Some(metadata) = &decoded.raw_metadata {
                    if let Ok(meta_json) = serde_json::to_string(metadata) {
                        let _ = db.update_raw_metadata(&image_id, &meta_json);
                    }
                }
            } else {
                let _ = thumbnails::generate_thumbnail(file_path, app_data_dir, &image_id);
            }
            run_source_detection(db, file_path, &image_id, &ext, decoded_dims);
            run_sidecar_detection(db, file_path, &image_id);
            run_perceptual_hash(db, file_path, &image_id, decoded.as_ref().map(|d| &d.image));
            run_color_metrics(db, file_path, &image_id, decoded.as_ref().map(|d| &d.image));
        }
        return Ok(SyncOutcome::ContentChanged { image_id });
    }

    // New path
    let hash = hash_file(file_path).map_err(|e| format!("Read error: {}", e))?;

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
        db.insert_image_file(&file_record)
            .map_err(|e| e.to_string())?;
        return Ok(SyncOutcome::NewImport {
            image_id: existing_img.id,
        });
    }

    // New content: only now read the bytes (source detection / decode need them).
    let data = fs::read(file_path).map_err(|e| format!("Read error: {}", e))?;
    let (image_id, decoded) =
        create_image_record(db, file_path, &hash, &ext, &data, can_decode, module_raw)?;
    let file_record = ImageFile {
        id: Uuid::new_v4().to_string(),
        image_id: image_id.clone(),
        path: path_str,
        last_seen_at: Utc::now().to_rfc3339(),
        missing_at: None,
        last_seen_size: Some(file_size),
        last_seen_mtime: Some(mtime),
    };
    db.insert_image_file(&file_record)
        .map_err(|e| e.to_string())?;

    if can_decode {
        let decoded_dims = decoded
            .as_ref()
            .map(|d| (d.image.width(), d.image.height()));
        if let Some(decoded) = &decoded {
            let _ =
                thumbnails::generate_thumbnail_from_image(&decoded.image, app_data_dir, &image_id);
            if let Some(metadata) = &decoded.raw_metadata {
                if let Ok(meta_json) = serde_json::to_string(metadata) {
                    let _ = db.update_raw_metadata(&image_id, &meta_json);
                }
            }
        } else {
            let _ = thumbnails::generate_thumbnail(file_path, app_data_dir, &image_id);
        }
        run_source_detection(db, file_path, &image_id, &ext, decoded_dims);
        run_sidecar_detection(db, file_path, &image_id);
        run_perceptual_hash(db, file_path, &image_id, decoded.as_ref().map(|d| &d.image));
        run_color_metrics(db, file_path, &image_id, decoded.as_ref().map(|d| &d.image));
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
        SyncOutcome::NewImport { image_id } | SyncOutcome::ContentChanged { image_id } => {
            Ok(Some(image_id))
        }
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
    module_raw: bool,
) -> Result<(String, Option<crate::db_core::image_decode::DecodedImage>), String> {
    let decoded = if can_decode {
        Some(crate::db_core::image_decode::decode_image(
            file_path, module_raw,
        )?)
    } else {
        None
    };
    let (width, height) = decoded
        .as_ref()
        .map(|d| (d.image.width(), d.image.height()))
        .unwrap_or((0, 0));

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
        raw_metadata: None,
    };
    db.insert_image(&image).map_err(|e| e.to_string())?;
    Ok((image_id, decoded))
}

/// Whole-buffer SHA-256, retained as a test reference for `hash_file`'s streaming
/// implementation. Production import paths hash via `hash_file` (streaming IO).
#[cfg(test)]
fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Upper bound on files we will read fully into memory during import. Pathological
/// or malicious TIFF/PSD/RAW/GIF can be enormous; reject them up front rather than
/// risking a memory cliff. Generous enough for legitimate high-resolution art.
const MAX_IMPORT_FILE_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB

fn import_size_within_limit(size: u64) -> bool {
    size <= MAX_IMPORT_FILE_BYTES
}

/// Stream a file through SHA-256 in fixed-size chunks so the whole file never
/// needs to live in a single `Vec<u8>` just to compute its content hash.
fn hash_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn run_source_detection(
    db: &Database,
    file_path: &Path,
    image_id: &str,
    ext: &str,
    override_dims: Option<(u32, u32)>,
) {
    let (width, height) = override_dims.unwrap_or_else(|| {
        image::open(file_path)
            .map(|i| (i.width(), i.height()))
            .unwrap_or((0, 0))
    });

    if crate::extensions::is_raw_extension(ext) {
        let aspect_ratio = width as f64 / height.max(1) as f64;
        let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
            "square"
        } else if aspect_ratio > 1.0 {
            "landscape"
        } else {
            "portrait"
        };
        let megapixels = (width as f64 * height as f64) / 1_000_000.0;
        let _ = db.update_source_detection(
            image_id,
            Some("camera"),
            1.0,
            &serde_json::json!({"method": "raw_format"}).to_string(),
            Some(false),
            None,
            aspect_ratio,
            orientation,
            megapixels,
        );
        return;
    }

    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let png_chunks = if ext == "png" {
        read_png_text_chunks(file_path).unwrap_or_default()
    } else {
        vec![]
    };
    let detection = detect_source(filename, &png_chunks, file_path);

    let aspect_ratio = width as f64 / height.max(1) as f64;
    let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
        "square"
    } else if aspect_ratio > 1.0 {
        "landscape"
    } else {
        "portrait"
    };
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;

    let _ = db.update_source_detection(
        image_id,
        detection.source_label.as_deref(),
        detection.confidence,
        &detection.to_evidence_json(),
        detection.is_ai_generated,
        detection.ai_prompt.as_deref(),
        aspect_ratio,
        orientation,
        megapixels,
    );
}

fn run_sidecar_detection(db: &Database, file_path: &Path, image_id: &str) {
    if let Some(sidecar_path) = sidecar::find_sidecar(file_path) {
        let _ = sidecar::link_sidecar_to_image(db, image_id, file_path, &sidecar_path, "sidecar");
    }
}

fn run_perceptual_hash(
    db: &Database,
    file_path: &Path,
    image_id: &str,
    preview: Option<&image::DynamicImage>,
) {
    let result = match preview {
        Some(img) => Ok(perceptual_hash::analyze_dynamic_image_perceptual_hash(
            image_id, img,
        )),
        None => perceptual_hash::analyze_image_perceptual_hash(image_id, file_path),
    };

    if let Ok(hash) = result {
        let _ = db.store_image_perceptual_hash(&hash);
    }
}

fn run_color_metrics(
    db: &Database,
    file_path: &Path,
    image_id: &str,
    preview: Option<&image::DynamicImage>,
) {
    let result = match preview {
        Some(img) => Ok(color::analyze_dynamic_image_color_metrics(image_id, img)),
        None => color::analyze_image_color_metrics(image_id, file_path),
    };

    if let Ok(metrics) = result {
        let _ = db.store_image_color_metrics(&metrics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn hash_file_matches_in_memory_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blob.bin");
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::File::create(&path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();

        // Streaming hash must equal the whole-buffer hash for the same content.
        assert_eq!(hash_file(&path).unwrap(), compute_hash(&bytes));
    }

    #[test]
    fn hash_file_handles_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.bin");
        std::fs::File::create(&path).unwrap();
        assert_eq!(hash_file(&path).unwrap(), compute_hash(&[]));
    }

    #[test]
    fn import_size_within_limit_boundary() {
        assert!(import_size_within_limit(0));
        assert!(import_size_within_limit(MAX_IMPORT_FILE_BYTES));
        assert!(!import_size_within_limit(MAX_IMPORT_FILE_BYTES + 1));
    }
}
