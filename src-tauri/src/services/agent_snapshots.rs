use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const AGENT_SNAPSHOTS_DIR: &str = "Agent Snapshots";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshotPackage {
    pub snapshot_id: String,
    pub package_dir: PathBuf,
    pub raw_png_path: PathBuf,
    pub annotated_png_path: PathBuf,
    pub manifest_json_path: PathBuf,
    pub manifest: Value,
    pub created_at: String,
}

#[derive(Debug, Default)]
pub struct AgentSnapshotRegistry {
    snapshots: BTreeMap<String, AgentSnapshotPackage>,
    latest_snapshot_id: Option<String>,
}

impl AgentSnapshotRegistry {
    pub fn latest_snapshot_id(&self) -> Option<String> {
        self.latest_snapshot_id.clone()
    }

    pub fn get_snapshot(&self, snapshot_id: &str) -> Option<&AgentSnapshotPackage> {
        self.snapshots.get(snapshot_id)
    }

    pub fn latest_snapshot(&self) -> Option<&AgentSnapshotPackage> {
        self.latest_snapshot_id
            .as_ref()
            .and_then(|snapshot_id| self.snapshots.get(snapshot_id))
    }

    fn insert(&mut self, package: AgentSnapshotPackage) {
        self.latest_snapshot_id = Some(package.snapshot_id.clone());
        self.snapshots.insert(package.snapshot_id.clone(), package);
    }

    fn remove(&mut self, snapshot_id: &str) {
        self.snapshots.remove(snapshot_id);
        if self.latest_snapshot_id.as_deref() == Some(snapshot_id) {
            self.latest_snapshot_id = self.snapshots.keys().next_back().cloned();
        }
    }
}

pub fn write_snapshot_package(
    app_data_dir: &Path,
    registry: &mut AgentSnapshotRegistry,
    snapshot_id: &str,
    mut manifest: Value,
    raw_png: &[u8],
    annotated_png: &[u8],
    retention_limit: usize,
) -> Result<AgentSnapshotPackage, String> {
    validate_snapshot_id(snapshot_id)?;

    let snapshots_dir = app_data_dir.join(AGENT_SNAPSHOTS_DIR);
    let package_dir = snapshots_dir.join(snapshot_id);
    std::fs::create_dir_all(&package_dir)
        .map_err(|e| format!("Failed to create snapshot directory: {}", e))?;

    let raw_png_path = package_dir.join("raw.png");
    let annotated_png_path = package_dir.join("annotated.png");
    let manifest_json_path = package_dir.join("manifest.json");

    set_json_path(
        &mut manifest,
        &["snapshot_id"],
        Value::String(snapshot_id.to_string()),
    );
    set_json_path(
        &mut manifest,
        &["destination", "detail"],
        Value::String(package_dir.to_string_lossy().to_string()),
    );
    set_json_path(
        &mut manifest,
        &["files", "raw_png"],
        Value::String(raw_png_path.to_string_lossy().to_string()),
    );
    set_json_path(
        &mut manifest,
        &["files", "annotated_png"],
        Value::String(annotated_png_path.to_string_lossy().to_string()),
    );
    set_json_path(
        &mut manifest,
        &["files", "manifest_json"],
        Value::String(manifest_json_path.to_string_lossy().to_string()),
    );

    write_atomic(&raw_png_path, raw_png)?;
    write_atomic(&annotated_png_path, annotated_png)?;
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize snapshot manifest: {}", e))?;
    write_atomic(&manifest_json_path, &manifest_bytes)?;

    let created_at = manifest
        .get("created_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let package = AgentSnapshotPackage {
        snapshot_id: snapshot_id.to_string(),
        package_dir,
        raw_png_path,
        annotated_png_path,
        manifest_json_path,
        manifest,
        created_at,
    };
    registry.insert(package.clone());
    prune_snapshot_packages(&snapshots_dir, registry, retention_limit, snapshot_id)?;
    Ok(package)
}

pub fn redact_manifest_for_remote(mut manifest: Value) -> Value {
    redact_path_values(&mut manifest, None);
    manifest
}

pub fn image_ids_for_snapshot_labels(
    manifest: &Value,
    labels: &[String],
) -> Result<Vec<String>, String> {
    let visible_images = manifest
        .get("visible_images")
        .and_then(Value::as_array)
        .ok_or_else(|| "Snapshot manifest missing visible_images".to_string())?;
    let mut by_label = BTreeMap::new();
    for image in visible_images {
        let Some(label) = image.get("label").and_then(Value::as_str) else {
            continue;
        };
        let Some(image_id) = image.get("image_id").and_then(Value::as_str) else {
            continue;
        };
        by_label.insert(label.to_string(), image_id.to_string());
    }

    labels
        .iter()
        .map(|label| {
            by_label
                .get(label)
                .cloned()
                .ok_or_else(|| format!("Unknown snapshot label: {}", label))
        })
        .collect()
}

pub fn snapshot_response_value(package: &AgentSnapshotPackage, redact: bool) -> Value {
    let manifest = if redact {
        redact_manifest_for_remote(package.manifest.clone())
    } else {
        package.manifest.clone()
    };
    serde_json::json!({
        "snapshot_id": package.snapshot_id,
        "package_dir": if redact { "[redacted:path]".to_string() } else { package.package_dir.to_string_lossy().to_string() },
        "raw_png_path": if redact { "[redacted:path]".to_string() } else { package.raw_png_path.to_string_lossy().to_string() },
        "annotated_png_path": if redact { "[redacted:path]".to_string() } else { package.annotated_png_path.to_string_lossy().to_string() },
        "manifest_json_path": if redact { "[redacted:path]".to_string() } else { package.manifest_json_path.to_string_lossy().to_string() },
        "manifest": manifest,
    })
}

fn validate_snapshot_id(snapshot_id: &str) -> Result<(), String> {
    if snapshot_id.is_empty()
        || !snapshot_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err("Invalid snapshot_id".to_string());
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid snapshot file path".to_string())?;
    let tmp = path.with_file_name(format!(".{}.tmp", file_name));
    std::fs::write(&tmp, bytes).map_err(|e| format!("Failed to write snapshot file: {}", e))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Failed to finalize snapshot file: {}", e))
}

fn set_json_path(value: &mut Value, path: &[&str], next: Value) {
    if path.is_empty() {
        *value = next;
        return;
    }

    let Value::Object(map) = value else {
        return;
    };
    if path.len() == 1 {
        map.insert(path[0].to_string(), next);
        return;
    }
    if let Some(child) = map.get_mut(path[0]) {
        set_json_path(child, &path[1..], next);
    }
}

fn redact_path_values(value: &mut Value, key: Option<&str>) {
    match value {
        Value::Object(map) => {
            for (child_key, child_value) in map.iter_mut() {
                redact_path_values(child_value, Some(child_key));
            }
        }
        Value::Array(items) => {
            for child in items {
                redact_path_values(child, key);
            }
        }
        Value::String(text) if should_redact_string(key, text) => {
            *value = Value::String("[redacted:path]".to_string());
        }
        _ => {}
    }
}

fn should_redact_string(key: Option<&str>, value: &str) -> bool {
    if !(value.starts_with('/') || value.starts_with("~/")) {
        return false;
    }
    matches!(
        key,
        Some("path")
            | Some("thumbnail_path")
            | Some("raw_png")
            | Some("annotated_png")
            | Some("manifest_json")
            | Some("detail")
            | Some("package_dir")
            | Some("raw_png_path")
            | Some("annotated_png_path")
            | Some("manifest_json_path")
    )
}

fn prune_snapshot_packages(
    snapshots_dir: &Path,
    registry: &mut AgentSnapshotRegistry,
    retention_limit: usize,
    keep_snapshot_id: &str,
) -> Result<(), String> {
    if retention_limit == 0 || !snapshots_dir.exists() {
        return Ok(());
    }

    let mut packages: Vec<(std::time::SystemTime, PathBuf, String)> =
        std::fs::read_dir(snapshots_dir)
            .map_err(|e| format!("Failed to read snapshot directory: {}", e))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }
                let snapshot_id = path.file_name()?.to_string_lossy().to_string();
                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                Some((modified, path, snapshot_id))
            })
            .collect();
    if packages.len() <= retention_limit {
        return Ok(());
    }

    packages.sort_by_key(|(modified, _, _)| *modified);
    let prune_count = packages.len().saturating_sub(retention_limit);
    for (_, path, snapshot_id) in packages.into_iter().take(prune_count) {
        if snapshot_id == keep_snapshot_id {
            continue;
        }
        registry.remove(&snapshot_id);
        trash::delete(&path).map_err(|e| {
            format!(
                "Failed to move old snapshot package to Trash ({}): {}",
                path.display(),
                e
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "snapshot_id": "snap_123",
            "created_at": "2026-06-04T10:30:00Z",
            "view_mode": "grid",
            "capture_reason": "shortcut",
            "destination": { "kind": "local", "detail": "/tmp/Cull/Agent Snapshots/snap_123" },
            "files": {
                "raw_png": "/tmp/Cull/Agent Snapshots/snap_123/raw.png",
                "annotated_png": "/tmp/Cull/Agent Snapshots/snap_123/annotated.png",
                "manifest_json": "/tmp/Cull/Agent Snapshots/snap_123/manifest.json"
            },
            "window": {
                "label": "main",
                "title": "Cull",
                "width_css": 400,
                "height_css": 300,
                "device_pixel_ratio": 2.0
            },
            "scope": {
                "kind": "folder",
                "id": null,
                "label": "Library",
                "path": "/library"
            },
            "visible_images": [
                {
                    "label": "1",
                    "image_id": "img-a",
                    "filename": "a.png",
                    "path": "/library/a.png",
                    "thumbnail_path": "/library/.thumbs/a.jpg",
                    "bounds_css": { "left": 0, "top": 0, "width": 100, "height": 80 },
                    "bounds_px": { "left": 0, "top": 0, "width": 200, "height": 160 },
                    "visible_ratio": 1.0,
                    "focused": true,
                    "selected": false,
                    "rating": 5,
                    "decision": "accept",
                    "view_role": "grid-cell"
                },
                {
                    "label": "2",
                    "image_id": "img-b",
                    "filename": "b.png",
                    "path": "/library/b.png",
                    "thumbnail_path": null,
                    "bounds_css": { "left": 120, "top": 0, "width": 100, "height": 80 },
                    "bounds_px": { "left": 240, "top": 0, "width": 200, "height": 160 },
                    "visible_ratio": 0.8,
                    "focused": false,
                    "selected": true,
                    "rating": null,
                    "decision": "undecided",
                    "view_role": "grid-cell"
                }
            ]
        })
    }

    #[test]
    fn test_write_snapshot_package_creates_files_and_tracks_latest() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = AgentSnapshotRegistry::default();
        let manifest = sample_manifest();

        let package = write_snapshot_package(
            dir.path(),
            &mut registry,
            "snap_123",
            manifest,
            b"raw-png",
            b"annotated-png",
            25,
        )
        .unwrap();

        assert_eq!(package.snapshot_id, "snap_123");
        assert_eq!(package.package_dir.file_name().unwrap(), "snap_123");
        assert_eq!(std::fs::read(package.raw_png_path).unwrap(), b"raw-png");
        assert_eq!(
            std::fs::read(package.annotated_png_path).unwrap(),
            b"annotated-png"
        );
        assert!(package.manifest_json_path.exists());
        assert_eq!(registry.latest_snapshot_id().as_deref(), Some("snap_123"));
        assert!(registry.get_snapshot("snap_123").is_some());
    }

    #[test]
    fn test_remote_manifest_redaction_preserves_labels_and_hides_paths() {
        let redacted = redact_manifest_for_remote(sample_manifest());

        assert_eq!(redacted["visible_images"][0]["label"], "1");
        assert_eq!(redacted["visible_images"][0]["image_id"], "img-a");
        assert_eq!(redacted["visible_images"][0]["filename"], "a.png");
        assert_eq!(redacted["visible_images"][0]["path"], "[redacted:path]");
        assert_eq!(
            redacted["visible_images"][0]["thumbnail_path"],
            "[redacted:path]"
        );
        assert_eq!(redacted["scope"]["path"], "[redacted:path]");
        assert_eq!(redacted["files"]["raw_png"], "[redacted:path]");
        assert_eq!(redacted["destination"]["detail"], "[redacted:path]");
    }

    #[test]
    fn test_snapshot_labels_map_to_image_ids_and_reject_unknown_labels() {
        let manifest = sample_manifest();

        let ids =
            image_ids_for_snapshot_labels(&manifest, &["2".to_string(), "1".to_string()]).unwrap();
        assert_eq!(ids, vec!["img-b".to_string(), "img-a".to_string()]);

        let error = image_ids_for_snapshot_labels(&manifest, &["99".to_string()]).unwrap_err();
        assert!(error.contains("Unknown snapshot label: 99"));
    }
}
