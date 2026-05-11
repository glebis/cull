use tauri::State;
use crate::AppState;
use crate::export::manifest::*;
use crate::export::presets;
use crate::export::validate;
use crate::export::pdf;
use base64::Engine;
use crate::export::patch::{self, JsonPatch, PatchResult};

#[derive(serde::Serialize)]
pub struct PresetInfo {
    pub id: String,
    pub platform: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub mime: String,
}

#[derive(serde::Serialize)]
pub struct AssetResponse {
    pub path: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
pub async fn create_export_manifest(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
    collection_id: Option<String>,
    target_presets: Vec<String>,
    template: Option<String>,
) -> Result<ExportManifest, String> {
    let id = format!("story_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string());
    let mut manifest = ExportManifest::new(id, "Untitled Story".to_string());

    manifest.source.image_ids = image_ids.clone();
    manifest.source.collection_id = collection_id;

    if let Some(ref tmpl) = template {
        manifest.defaults.template = tmpl.clone();
    }

    for preset_id in &target_presets {
        if let Some(preset) = presets::get_preset(preset_id) {
            manifest.targets.push(preset.to_target());
        } else {
            return Err(format!("Unknown preset: '{}'", preset_id));
        }
    }

    let id_refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    let images = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;

    for (idx, img) in images.iter().enumerate() {
        let clean_id = img.image.id.replace("-", "");
        let asset_id = format!("asset_src_{}", &clean_id[..clean_id.len().min(8)]);

        let ext = std::path::Path::new(&img.path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let is_raw = crate::extensions::is_raw_extension(ext);

        // For RAW files, export the preview thumbnail (JPEG) rather than the
        // original RAW file, since most consumers cannot render proprietary RAW.
        let (uri, mime, source_kind) = if is_raw {
            (
                format!("cull://images/{}/preview", img.image.id),
                "image/jpeg".to_string(),
                Some("raw_preview".to_string()),
            )
        } else {
            (
                format!("cull://images/{}/original", img.image.id),
                format!("image/{}", img.image.format),
                None,
            )
        };

        manifest.assets.push(Asset {
            id: asset_id.clone(),
            kind: "source".to_string(),
            uri,
            mime,
            width: img.image.width,
            height: img.image.height,
            provenance: None,
            source_kind,
        });

        let slide_id = format!("slide_{:03}", idx + 1);

        manifest.slides.push(Slide {
            id: slide_id.clone(),
            template: None,
            targets: None,
            image: SlideImage {
                asset_id: asset_id.clone(),
                fit: "cover".to_string(),
                focal_point: Some(FocalPoint { x: 0.5, y: 0.5 }),
            },
            text: SlideText {
                headline: String::new(),
                body: String::new(),
                caption: String::new(),
            },
            overlay: SlideOverlay {
                position: "bottom-left".to_string(),
                scrim: Scrim {
                    scrim_type: "linear".to_string(),
                    direction: "to-top".to_string(),
                    from: "rgba(0,0,0,0)".to_string(),
                    to: "rgba(0,0,0,0.72)".to_string(),
                },
                text_color: "#ffffff".to_string(),
            },
            metadata: SlideMetadata {
                rating: img.selection.as_ref().and_then(|s| s.star_rating),
                tags: vec![],
                alt: String::new(),
            },
        });

        manifest.agent_tasks.push(AgentTask {
            slide_id: slide_id.clone(),
            field: "text.headline".to_string(),
            task: "fill".to_string(),
            required: true,
            max_chars: Some(72),
        });
        manifest.agent_tasks.push(AgentTask {
            slide_id: slide_id.clone(),
            field: "text.body".to_string(),
            task: "fill".to_string(),
            required: false,
            max_chars: Some(220),
        });
        manifest.agent_tasks.push(AgentTask {
            slide_id: slide_id.clone(),
            field: "metadata.alt".to_string(),
            task: "fill".to_string(),
            required: true,
            max_chars: Some(125),
        });
    }

    Ok(manifest)
}

#[tauri::command]
pub async fn validate_export_manifest(
    manifest: ExportManifest,
) -> Result<validate::ValidationResult, String> {
    Ok(validate::validate_manifest(&manifest))
}

#[tauri::command]
pub async fn apply_export_patches(
    manifest: ExportManifest,
    patches: Vec<JsonPatch>,
) -> Result<PatchResult, String> {
    Ok(patch::apply_patches(manifest, patches))
}

#[tauri::command]
pub async fn list_export_presets() -> Result<Vec<PresetInfo>, String> {
    let infos: Vec<PresetInfo> = presets::PRESETS
        .iter()
        .map(|p| PresetInfo {
            id: p.id.to_string(),
            platform: p.platform.to_string(),
            format: p.format.to_string(),
            width: p.width,
            height: p.height,
            mime: p.mime.to_string(),
        })
        .collect();
    Ok(infos)
}

#[tauri::command]
pub async fn get_export_asset(
    state: State<'_, AppState>,
    uri: String,
    variant: Option<String>,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<AssetResponse, String> {
    let _ = (max_width, max_height); // reserved for future resize support

    let stripped = uri.strip_prefix("cull://images/")
        .ok_or_else(|| format!("Unsupported URI scheme: {}", uri))?;

    let parts: Vec<&str> = stripped.split('/').collect();
    if parts.len() < 2 {
        return Err(format!("Invalid URI format: {}", uri));
    }
    let image_id = parts[0];
    let variant_str = variant.as_deref().unwrap_or("preview");

    let id_refs = vec![image_id];
    let images = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
    let img = images.first().ok_or_else(|| format!("Image '{}' not found", image_id))?;

    match variant_str {
        "original" => {
            // For RAW files, serving the original is not useful for most export
            // consumers. Fall back to the 800 px preview thumbnail (JPEG).
            let ext = std::path::Path::new(&img.path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if crate::extensions::is_raw_extension(ext) {
                let thumb_path = crate::db_core::thumbnails::thumbnail_path(
                    &state.app_data_dir, &img.image.id,
                );
                Ok(AssetResponse {
                    path: thumb_path.to_string_lossy().to_string(),
                    mime: "image/jpeg".to_string(),
                    width: 800.min(img.image.width),
                    height: 800.min(img.image.height),
                })
            } else {
                Ok(AssetResponse {
                    path: img.path.clone(),
                    mime: format!("image/{}", img.image.format),
                    width: img.image.width,
                    height: img.image.height,
                })
            }
        }
        "thumbnail" => {
            let thumb_path = crate::db_core::thumbnails::sized_thumbnail_path(
                &state.app_data_dir, &img.image.id, 256
            );
            Ok(AssetResponse {
                path: thumb_path.to_string_lossy().to_string(),
                mime: "image/jpeg".to_string(),
                width: 256.min(img.image.width),
                height: 256.min(img.image.height),
            })
        }
        _ => {
            // "preview" default — 800px thumbnail
            let thumb_path = crate::db_core::thumbnails::thumbnail_path(
                &state.app_data_dir, &img.image.id
            );
            Ok(AssetResponse {
                path: thumb_path.to_string_lossy().to_string(),
                mime: "image/jpeg".to_string(),
                width: 800.min(img.image.width),
                height: 800.min(img.image.height),
            })
        }
    }
}

#[tauri::command]
pub async fn save_export_image(
    state: State<'_, AppState>,
    base64_data: String,
    slide_id: String,
    target_id: String,
    manifest_id: String,
) -> Result<String, String> {
    let export_dir = state.app_data_dir.join("exports").join(&manifest_id).join(&target_id);
    std::fs::create_dir_all(&export_dir)
        .map_err(|e| format!("Failed to create export dir: {}", e))?;

    let filename = format!("{}.png", slide_id);
    let path = export_dir.join(&filename);

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&base64_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    std::fs::write(&path, &bytes)
        .map_err(|e| format!("Failed to write image: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn assemble_export_pdf(
    state: State<'_, AppState>,
    image_paths: Vec<String>,
    width_px: u32,
    height_px: u32,
    manifest_id: String,
    target_id: String,
) -> Result<String, String> {
    let export_dir = state.app_data_dir.join("exports").join(&manifest_id).join(&target_id);
    std::fs::create_dir_all(&export_dir)
        .map_err(|e| format!("Failed to create export dir: {}", e))?;

    let output_path = export_dir.join("carousel.pdf");
    let output_str = output_path.to_string_lossy().to_string();

    pdf::assemble_pdf(&image_paths, width_px, height_px, &output_str)
}
