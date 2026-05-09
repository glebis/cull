use crate::export::manifest::ExportManifest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPatch {
    pub op: String,
    pub path: String,
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct RejectedPatch {
    pub patch: JsonPatch,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct PatchResult {
    pub manifest: ExportManifest,
    pub applied_patches: Vec<JsonPatch>,
    pub rejected_patches: Vec<RejectedPatch>,
}

fn is_immutable_path(path: &str) -> bool {
    if path == "/kind" || path == "/source" || path == "/targets" {
        return true;
    }
    if path.starts_with("/source/") || path.starts_with("/targets/") {
        return true;
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 5 && parts[1] == "slides" && parts[3] == "image" && parts[4] == "asset_id" {
        return true;
    }
    false
}

fn is_source_asset_mutation(path: &str, manifest: &ExportManifest) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[1] == "assets" {
        if let Ok(idx) = parts[2].parse::<usize>() {
            if let Some(asset) = manifest.assets.get(idx) {
                if asset.kind == "source" {
                    return true;
                }
            }
        }
    }
    false
}

fn is_mutable_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    // /slides/N/text/*
    if parts.len() >= 5 && parts[1] == "slides" && parts[3] == "text" {
        return true;
    }
    // /slides/N/metadata/alt
    if parts.len() >= 5 && parts[1] == "slides" && parts[3] == "metadata" && parts[4] == "alt" {
        return true;
    }
    // /slides/N/overlay or /slides/N/overlay/*
    if parts.len() >= 4 && parts[1] == "slides" && parts[3] == "overlay" {
        return true;
    }
    false
}

pub fn apply_patches(manifest: ExportManifest, patches: Vec<JsonPatch>) -> PatchResult {
    let mut json_value = serde_json::to_value(&manifest).unwrap();
    let mut applied = Vec::new();
    let mut rejected = Vec::new();

    for patch in patches {
        if is_immutable_path(&patch.path) {
            rejected.push(RejectedPatch {
                patch,
                reason: "Path is immutable".to_string(),
            });
            continue;
        }

        if is_source_asset_mutation(&patch.path, &manifest) {
            rejected.push(RejectedPatch {
                patch,
                reason: "Source assets are immutable".to_string(),
            });
            continue;
        }

        let is_asset_append = patch.path == "/assets/-" && patch.op == "add";
        if !is_asset_append && !is_mutable_path(&patch.path) {
            rejected.push(RejectedPatch {
                patch,
                reason: "Path is not in mutable_paths".to_string(),
            });
            continue;
        }

        match patch.op.as_str() {
            "replace" => {
                if let Some(ref value) = patch.value {
                    if let Some(target) = resolve_pointer_mut(&mut json_value, &patch.path) {
                        *target = value.clone();
                        applied.push(patch);
                    } else {
                        rejected.push(RejectedPatch {
                            patch,
                            reason: "Path not found in manifest".to_string(),
                        });
                    }
                } else {
                    rejected.push(RejectedPatch {
                        patch,
                        reason: "Replace operation requires a value".to_string(),
                    });
                }
            }
            "add" => {
                if let Some(ref value) = patch.value {
                    if patch.path.ends_with("/-") {
                        let array_path = &patch.path[..patch.path.len() - 2];
                        if let Some(arr) = resolve_pointer_mut(&mut json_value, array_path) {
                            if let Some(arr) = arr.as_array_mut() {
                                arr.push(value.clone());
                                applied.push(patch);
                            } else {
                                rejected.push(RejectedPatch {
                                    patch,
                                    reason: "Target is not an array".to_string(),
                                });
                            }
                        } else {
                            rejected.push(RejectedPatch {
                                patch,
                                reason: "Array path not found".to_string(),
                            });
                        }
                    } else if let Some(target) = resolve_pointer_mut(&mut json_value, &patch.path) {
                        *target = value.clone();
                        applied.push(patch);
                    } else {
                        rejected.push(RejectedPatch {
                            patch,
                            reason: "Path not found".to_string(),
                        });
                    }
                } else {
                    rejected.push(RejectedPatch {
                        patch,
                        reason: "Add operation requires a value".to_string(),
                    });
                }
            }
            other => {
                let op = other.to_string();
                rejected.push(RejectedPatch {
                    patch,
                    reason: format!("Unsupported operation '{}'. Supported: replace, add", op),
                });
            }
        }
    }

    let patched_manifest: ExportManifest = serde_json::from_value(json_value)
        .unwrap_or(manifest);

    PatchResult {
        manifest: patched_manifest,
        applied_patches: applied,
        rejected_patches: rejected,
    }
}

fn resolve_pointer_mut<'a>(value: &'a mut serde_json::Value, pointer: &str) -> Option<&'a mut serde_json::Value> {
    if pointer.is_empty() || pointer == "/" {
        return Some(value);
    }
    let parts: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
    let mut current = value;
    for part in parts {
        if let Ok(idx) = part.parse::<usize>() {
            current = current.get_mut(idx)?;
        } else {
            current = current.get_mut(part)?;
        }
    }
    Some(current)
}
