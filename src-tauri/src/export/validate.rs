use crate::export::manifest::ExportManifest;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ValidationWarning {
    pub path: String,
    pub message: String,
}

const VALID_TEMPLATES: &[&str] = &["terminal", "editorial", "bleed"];

pub fn validate_manifest(manifest: &ExportManifest) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if manifest.kind != "imageview-story/v1" {
        errors.push(ValidationError {
            path: "/kind".to_string(),
            message: format!("Expected 'imageview-story/v1', got '{}'", manifest.kind),
        });
    }

    if manifest.schema_version != 1 {
        errors.push(ValidationError {
            path: "/schema_version".to_string(),
            message: format!("Expected schema_version 1, got {}", manifest.schema_version),
        });
    }

    if !VALID_TEMPLATES.contains(&manifest.defaults.template.as_str()) {
        errors.push(ValidationError {
            path: "/defaults/template".to_string(),
            message: format!("Invalid default template '{}'", manifest.defaults.template),
        });
    }

    // Unique target IDs
    let mut target_ids = HashSet::new();
    for (i, target) in manifest.targets.iter().enumerate() {
        if !target_ids.insert(&target.id) {
            errors.push(ValidationError {
                path: format!("/targets/{}", i),
                message: format!("Duplicate target ID: '{}'", target.id),
            });
        }
        if target.quality.is_some() && target.mime != "image/jpeg" && target.mime != "image/webp" {
            errors.push(ValidationError {
                path: format!("/targets/{}/quality", i),
                message: format!("Quality only valid for JPEG/WebP, not '{}'", target.mime),
            });
        }
    }

    // Unique asset IDs
    let mut asset_ids = HashSet::new();
    for (i, asset) in manifest.assets.iter().enumerate() {
        if !asset_ids.insert(&asset.id) {
            errors.push(ValidationError {
                path: format!("/assets/{}", i),
                message: format!("Duplicate asset ID: '{}'", asset.id),
            });
        }
    }

    // Unique slide IDs + referential integrity
    let mut slide_ids = HashSet::new();
    for (i, slide) in manifest.slides.iter().enumerate() {
        if !slide_ids.insert(&slide.id) {
            errors.push(ValidationError {
                path: format!("/slides/{}", i),
                message: format!("Duplicate slide ID: '{}'", slide.id),
            });
        }

        if !manifest.assets.iter().any(|a| a.id == slide.image.asset_id) {
            errors.push(ValidationError {
                path: format!("/slides/{}/image/asset_id", i),
                message: format!("Asset '{}' not found in assets array", slide.image.asset_id),
            });
        }

        if let Some(ref slide_targets) = slide.targets {
            for t in slide_targets {
                if !target_ids.contains(t) {
                    errors.push(ValidationError {
                        path: format!("/slides/{}/targets", i),
                        message: format!("Target '{}' not found in targets array", t),
                    });
                }
            }
        }

        if let Some(ref fp) = slide.image.focal_point {
            if fp.x < 0.0 || fp.x > 1.0 || fp.y < 0.0 || fp.y > 1.0 {
                errors.push(ValidationError {
                    path: format!("/slides/{}/image/focal_point", i),
                    message: format!("focal_point x/y must be 0..1, got ({}, {})", fp.x, fp.y),
                });
            }
        }

        if let Some(ref tmpl) = slide.template {
            if !VALID_TEMPLATES.contains(&tmpl.as_str()) {
                errors.push(ValidationError {
                    path: format!("/slides/{}/template", i),
                    message: format!("Invalid template '{}', expected one of: {:?}", tmpl, VALID_TEMPLATES),
                });
            }
        }
    }

    // agent_tasks must reference existing slides
    for (i, task) in manifest.agent_tasks.iter().enumerate() {
        if !slide_ids.contains(&task.slide_id) {
            errors.push(ValidationError {
                path: format!("/agent_tasks/{}", i),
                message: format!("Slide '{}' not found", task.slide_id),
            });
        }
    }

    if manifest.targets.is_empty() {
        warnings.push(ValidationWarning {
            path: "/targets".to_string(),
            message: "No targets defined".to_string(),
        });
    }

    if manifest.slides.is_empty() {
        warnings.push(ValidationWarning {
            path: "/slides".to_string(),
            message: "No slides defined".to_string(),
        });
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}
