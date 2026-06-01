use crate::commands::export::PresetInfo;
use crate::db_core::db::Database;
use crate::db_core::models::ImageWithFile;
use crate::export::presets;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn list_presets() -> Vec<PresetInfo> {
    presets::PRESETS
        .iter()
        .map(|p| PresetInfo {
            id: p.id.to_string(),
            platform: p.platform.to_string(),
            format: p.format.to_string(),
            width: p.width,
            height: p.height,
            mime: p.mime.to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ExportImagesParams {
    pub image_ids: Option<Vec<String>>,
    pub collection_id: Option<String>,
    pub folder_path: Option<String>,
    pub output_dir: String,
    pub format: Option<String>,
    pub flatten: Option<bool>,
    pub naming: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportedImage {
    pub image_id: String,
    pub source_path: String,
    pub output_path: String,
    pub format: String,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportImagesResult {
    pub exported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub output_dir: String,
    pub files: Vec<ExportedImage>,
}

pub fn export_images(
    db: &Database,
    _app_data_dir: &Path,
    params: ExportImagesParams,
) -> Result<ExportImagesResult, String> {
    let images = resolve_export_images(db, &params)?;
    let output_dir = PathBuf::from(&params.output_dir);
    fs::create_dir_all(&output_dir).map_err(|e| {
        format!(
            "Failed to create output dir '{}': {}",
            output_dir.display(),
            e
        )
    })?;

    let format = params
        .format
        .as_deref()
        .unwrap_or("original")
        .to_lowercase();
    validate_export_format(&format)?;

    let flatten = params.flatten.unwrap_or(true);
    let naming = params.naming.as_deref().unwrap_or("original");
    let mut used_paths = HashSet::new();
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let mut skipped = 0u32;

    for (idx, image) in images.iter().enumerate() {
        match export_one_image(
            image,
            idx,
            &params,
            &output_dir,
            &format,
            flatten,
            naming,
            &mut used_paths,
        ) {
            Ok(file) => files.push(file),
            Err(e) => {
                skipped += 1;
                errors.push(e);
            }
        }
    }

    Ok(ExportImagesResult {
        exported: files.len() as u32,
        skipped,
        errors,
        output_dir: output_dir.to_string_lossy().to_string(),
        files,
    })
}

fn resolve_export_images(
    db: &Database,
    params: &ExportImagesParams,
) -> Result<Vec<ImageWithFile>, String> {
    let selector_count = [
        params
            .image_ids
            .as_ref()
            .map(|ids| !ids.is_empty())
            .unwrap_or(false),
        params
            .collection_id
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        params
            .folder_path
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();

    if selector_count != 1 {
        return Err(
            "Provide exactly one selector: image_ids, collection_id, or folder_path".to_string(),
        );
    }

    if let Some(image_ids) = params.image_ids.as_ref() {
        let refs: Vec<&str> = image_ids.iter().map(|id| id.as_str()).collect();
        return db.get_images_by_ids(&refs).map_err(|e| e.to_string());
    }

    if let Some(collection_id) = params.collection_id.as_ref() {
        return db
            .list_collection_images(collection_id)
            .map_err(|e| e.to_string());
    }

    let folder_path = params.folder_path.as_ref().expect("selector checked");
    db.list_images_by_folder(folder_path, 100_000, 0)
        .map_err(|e| e.to_string())
}

fn validate_export_format(format: &str) -> Result<(), String> {
    match format {
        "original" | "png" | "jpg" | "jpeg" | "webp" => Ok(()),
        other => Err(format!(
            "Unsupported export format '{}'. Supported: original, png, jpg, jpeg, webp",
            other
        )),
    }
}

fn export_one_image(
    image: &ImageWithFile,
    idx: usize,
    params: &ExportImagesParams,
    output_dir: &Path,
    format: &str,
    flatten: bool,
    naming: &str,
    used_paths: &mut HashSet<PathBuf>,
) -> Result<ExportedImage, String> {
    let source = Path::new(&image.path);
    if !source.exists() {
        return Err(format!(
            "Source file missing for image '{}': {}",
            image.image.id, image.path
        ));
    }

    let rel_dir = if !flatten {
        params
            .folder_path
            .as_ref()
            .and_then(|base| {
                source
                    .parent()
                    .and_then(|parent| parent.strip_prefix(base).ok())
            })
            .map(PathBuf::from)
            .unwrap_or_default()
    } else {
        PathBuf::new()
    };
    let target_dir = output_dir.join(rel_dir);
    fs::create_dir_all(&target_dir).map_err(|e| {
        format!(
            "Failed to create export dir '{}': {}",
            target_dir.display(),
            e
        )
    })?;

    let ext = if format == "original" {
        source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("img")
            .to_lowercase()
    } else if format == "jpeg" {
        "jpg".to_string()
    } else {
        format.to_string()
    };
    let stem = export_stem(source, &image.image.id, idx, naming)?;
    let target = unique_target_path(&target_dir, &stem, &ext, used_paths);

    if format == "original" {
        fs::copy(source, &target).map_err(|e| {
            format!(
                "Failed to copy '{}' to '{}': {}",
                source.display(),
                target.display(),
                e
            )
        })?;
    } else {
        let dyn_image = image::open(source)
            .map_err(|e| format!("Failed to decode '{}': {}", source.display(), e))?;
        let image_format = match format {
            "png" => image::ImageFormat::Png,
            "jpg" | "jpeg" => image::ImageFormat::Jpeg,
            "webp" => image::ImageFormat::WebP,
            _ => unreachable!("format validated"),
        };
        dyn_image
            .save_with_format(&target, image_format)
            .map_err(|e| format!("Failed to write '{}': {}", target.display(), e))?;
    }

    let bytes_written = fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
    Ok(ExportedImage {
        image_id: image.image.id.clone(),
        source_path: image.path.clone(),
        output_path: target.to_string_lossy().to_string(),
        format: ext,
        bytes_written,
    })
}

fn export_stem(source: &Path, image_id: &str, idx: usize, naming: &str) -> Result<String, String> {
    // Preset keywords kept for back-compat with existing MCP callers.
    match naming {
        "original" => {
            return Ok(source
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(image_id)
                .to_string())
        }
        "id" => return Ok(image_id.to_string()),
        "index" => return Ok(format!("{:04}", idx + 1)),
        _ => {}
    }

    // Otherwise treat `naming` as a filename template with {name}, {id},
    // {index}, and {index1} tokens. {index} is zero-padded (0001), {index1}
    // is the bare 1-based ordinal.
    if naming.contains('{') {
        return render_naming_template(source, image_id, idx, naming);
    }

    Err(format!(
        "Unsupported naming '{}'. Use a preset (original, id, index) or a template such as '{{index}}_{{name}}'",
        naming
    ))
}

fn render_naming_template(
    source: &Path,
    image_id: &str,
    idx: usize,
    template: &str,
) -> Result<String, String> {
    let name = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(image_id);
    let rendered = template
        .replace("{name}", name)
        .replace("{id}", image_id)
        .replace("{index1}", &format!("{}", idx + 1))
        .replace("{index}", &format!("{:04}", idx + 1));

    // Reject any leftover/unknown tokens so typos fail loudly instead of
    // producing files with literal braces in their names.
    if rendered.contains('{') || rendered.contains('}') {
        return Err(format!(
            "Naming template '{}' has unknown tokens. Supported: {{name}}, {{id}}, {{index}}, {{index1}}",
            template
        ));
    }

    let sanitized = sanitize_stem(&rendered);
    if sanitized.is_empty() {
        return Err(format!("Naming template '{}' produced an empty filename", template));
    }
    Ok(sanitized)
}

// Strip path separators and characters that are invalid in filenames so a
// template can't escape the output directory or produce an unwritable name.
fn sanitize_stem(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_naming_keywords_still_work() {
        let src = Path::new("/photos/sunset.jpg");
        assert_eq!(export_stem(src, "img-1", 0, "original").unwrap(), "sunset");
        assert_eq!(export_stem(src, "img-1", 0, "id").unwrap(), "img-1");
        assert_eq!(export_stem(src, "img-1", 4, "index").unwrap(), "0005");
    }

    #[test]
    fn template_tokens_render() {
        let src = Path::new("/photos/sunset.jpg");
        assert_eq!(
            export_stem(src, "img-9", 2, "{index}_{name}").unwrap(),
            "0003_sunset"
        );
        assert_eq!(
            export_stem(src, "img-9", 2, "{index1}-{id}").unwrap(),
            "3-img-9"
        );
    }

    #[test]
    fn template_rejects_unknown_tokens() {
        let src = Path::new("/photos/sunset.jpg");
        assert!(export_stem(src, "img-1", 0, "{bogus}").is_err());
    }

    #[test]
    fn template_sanitizes_path_separators() {
        let src = Path::new("/photos/sunset.jpg");
        // A template that would otherwise inject a path separator is flattened.
        assert_eq!(
            export_stem(src, "a/b", 0, "{id}").unwrap(),
            "a_b"
        );
    }

    #[test]
    fn unsupported_plain_naming_errors() {
        let src = Path::new("/photos/sunset.jpg");
        assert!(export_stem(src, "img-1", 0, "weird").is_err());
    }

    #[test]
    fn validate_export_format_accepts_known_formats() {
        for fmt in ["original", "png", "jpg", "jpeg", "webp"] {
            assert!(validate_export_format(fmt).is_ok());
        }
        assert!(validate_export_format("gif").is_err());
    }
}

fn unique_target_path(
    target_dir: &Path,
    stem: &str,
    ext: &str,
    used_paths: &mut HashSet<PathBuf>,
) -> PathBuf {
    let mut candidate = target_dir.join(format!("{}.{}", stem, ext));
    let mut suffix = 2;
    while candidate.exists() || used_paths.contains(&candidate) {
        candidate = target_dir.join(format!("{}_{}.{}", stem, suffix, ext));
        suffix += 1;
    }
    used_paths.insert(candidate.clone());
    candidate
}
