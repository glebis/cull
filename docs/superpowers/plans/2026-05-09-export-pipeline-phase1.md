# Export Pipeline Phase 1: Manifest + MCP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the `imageview-story/v1` manifest format, platform presets, Tauri export commands, TypeScript API layer, and bundle EB Garamond fonts.

**Architecture:** Rust serde types define the manifest schema. Tauri commands expose create/validate/patch/get-asset operations. TypeScript API wrappers call them via `invoke()`. Platform presets are data-driven (a const array of target definitions). No rendering or UI in this phase.

**Tech Stack:** Rust (serde, serde_json, uuid, chrono), Tauri 2 commands, TypeScript API wrappers, EB Garamond woff2 fonts.

---

## File Structure

### New files to create:
```
src-tauri/src/export/mod.rs           — module root, re-exports
src-tauri/src/export/manifest.rs      — all manifest serde types (ExportManifest, Slide, Asset, etc.)
src-tauri/src/export/presets.rs       — platform preset definitions
src-tauri/src/export/validate.rs      — manifest validation logic
src-tauri/src/export/patch.rs         — JSON patch application with contract enforcement
src-tauri/src/commands/export.rs      — Tauri command handlers
src/lib/export-types.ts               — TypeScript types mirroring Rust manifest types
src/lib/export-api.ts                 — TypeScript API wrappers for export commands
static/fonts/EBGaramond-Regular.woff2
static/fonts/EBGaramond-Medium.woff2
static/fonts/EBGaramond-Bold.woff2
static/fonts/EBGaramond-Italic.woff2
```

### Files to modify:
```
src-tauri/src/lib.rs                  — add `mod export;`, register export commands
src-tauri/src/commands/mod.rs         — add `pub mod export;`
src-tauri/Cargo.toml                  — no new deps needed (serde, serde_json, uuid, chrono already present)
src/app.css                           — add @font-face for EB Garamond
```

---

### Task 1: Manifest Types (Rust)

**Files:**
- Create: `src-tauri/src/export/mod.rs`
- Create: `src-tauri/src/export/manifest.rs`

- [ ] **Step 1: Create the export module root**

```rust
// src-tauri/src/export/mod.rs
pub mod manifest;
pub mod presets;
pub mod validate;
pub mod patch;
```

- [ ] **Step 2: Register the export module in lib.rs**

Add `mod export;` after the existing `mod db_core;` line in `src-tauri/src/lib.rs:2`:

```rust
mod commands;
mod db_core;
mod export;
mod menu;
```

- [ ] **Step 3: Write manifest types**

Create `src-tauri/src/export/manifest.rs` with all serde types:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub kind: String,
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub locale: String,
    pub created_at: String,
    pub updated_at: String,
    pub source: ManifestSource,
    pub defaults: ManifestDefaults,
    pub targets: Vec<ExportTarget>,
    pub slides: Vec<Slide>,
    pub assets: Vec<Asset>,
    pub agent_tasks: Vec<AgentTask>,
    pub agent_hints: AgentHints,
    pub agent_contract: AgentContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSource {
    pub app: String,
    pub collection_id: Option<String>,
    pub image_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestDefaults {
    pub template: String,
    pub fonts: ManifestFonts,
    pub colors: ManifestColors,
    pub safe_area: SafeArea,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFonts {
    pub serif: String,
    pub mono: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestColors {
    pub preset: String,
    pub background: String,
    pub foreground: String,
    pub accent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeArea {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTarget {
    pub id: String,
    pub platform: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub mime: String,
    pub quality: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub id: String,
    pub template: Option<String>,
    pub targets: Option<Vec<String>>,
    pub image: SlideImage,
    pub text: SlideText,
    pub overlay: SlideOverlay,
    pub metadata: SlideMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideImage {
    pub asset_id: String,
    pub fit: String,
    pub focal_point: Option<FocalPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocalPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideText {
    pub headline: String,
    pub body: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideOverlay {
    pub position: String,
    pub scrim: Scrim,
    pub text_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scrim {
    #[serde(rename = "type")]
    pub scrim_type: String,
    pub direction: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideMetadata {
    pub rating: Option<u8>,
    pub tags: Vec<String>,
    pub alt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub kind: String, // "source" or "generated"
    pub uri: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub provenance: Option<AssetProvenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetProvenance {
    pub provider: String,
    pub model: String,
    pub prompt: String,
    pub thinking: Option<bool>,
    pub reference_assets: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub slide_id: String,
    pub field: String,
    pub task: String,
    pub required: bool,
    pub max_chars: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHints {
    pub tone: String,
    pub allow_generated_images: bool,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContract {
    pub mutable_paths: Vec<String>,
    pub append_only: Vec<String>,
    pub immutable_paths: Vec<String>,
}

impl ExportManifest {
    pub fn new(id: String, title: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            kind: "imageview-story/v1".to_string(),
            schema_version: 1,
            id,
            title,
            locale: "en".to_string(),
            created_at: now.clone(),
            updated_at: now,
            source: ManifestSource {
                app: "imageview".to_string(),
                collection_id: None,
                image_ids: vec![],
            },
            defaults: ManifestDefaults {
                template: "editorial".to_string(),
                fonts: ManifestFonts {
                    serif: "EB Garamond".to_string(),
                    mono: "JetBrains Mono".to_string(),
                },
                colors: ManifestColors {
                    preset: "light".to_string(),
                    background: "#f7f3ea".to_string(),
                    foreground: "#171717".to_string(),
                    accent: "#c6422b".to_string(),
                },
                safe_area: SafeArea { top: 96, right: 72, bottom: 96, left: 72 },
            },
            targets: vec![],
            slides: vec![],
            assets: vec![],
            agent_tasks: vec![],
            agent_hints: AgentHints {
                tone: "quiet editorial".to_string(),
                allow_generated_images: true,
                language: "en".to_string(),
            },
            agent_contract: AgentContract {
                mutable_paths: vec![
                    "/slides/*/text/*".to_string(),
                    "/slides/*/metadata/alt".to_string(),
                    "/slides/*/overlay".to_string(),
                ],
                append_only: vec!["/assets".to_string()],
                immutable_paths: vec![
                    "/kind".to_string(),
                    "/source".to_string(),
                    "/targets".to_string(),
                    "/slides/*/image/asset_id".to_string(),
                ],
            },
        }
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | head -20`

Note: `presets.rs`, `validate.rs`, and `patch.rs` don't exist yet — create empty files to satisfy `mod.rs`:

```rust
// src-tauri/src/export/presets.rs
// src-tauri/src/export/validate.rs
// src-tauri/src/export/patch.rs
```

Expected: compiles with no errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/export/ src-tauri/src/lib.rs
git commit -m "feat(export): add manifest serde types for imageview-story/v1"
```

---

### Task 2: Platform Presets

**Files:**
- Modify: `src-tauri/src/export/presets.rs`

- [ ] **Step 1: Write platform preset definitions**

```rust
use crate::export::manifest::ExportTarget;

pub struct PlatformPreset {
    pub id: &'static str,
    pub platform: &'static str,
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
    pub mime: &'static str,
    pub quality: Option<f32>,
}

pub const PRESETS: &[PlatformPreset] = &[
    PlatformPreset { id: "ig_post", platform: "instagram", format: "post", width: 1080, height: 1350, mime: "image/png", quality: None },
    PlatformPreset { id: "ig_story", platform: "instagram", format: "story", width: 1080, height: 1920, mime: "image/png", quality: None },
    PlatformPreset { id: "ig_carousel", platform: "instagram", format: "carousel", width: 1080, height: 1350, mime: "image/png", quality: None },
    PlatformPreset { id: "li_pdf", platform: "linkedin", format: "pdf_carousel", width: 1080, height: 1350, mime: "application/pdf", quality: None },
    PlatformPreset { id: "tw_post", platform: "twitter", format: "post", width: 1600, height: 900, mime: "image/jpeg", quality: Some(0.90) },
    PlatformPreset { id: "tt_story", platform: "tiktok", format: "story", width: 1080, height: 1920, mime: "image/png", quality: None },
    PlatformPreset { id: "pin", platform: "pinterest", format: "pin", width: 1000, height: 1500, mime: "image/png", quality: None },
    PlatformPreset { id: "tg_post", platform: "telegram", format: "post", width: 1280, height: 1280, mime: "image/jpeg", quality: Some(0.85) },
    PlatformPreset { id: "yt_thumb", platform: "youtube", format: "thumbnail", width: 1280, height: 720, mime: "image/png", quality: None },
    PlatformPreset { id: "bsky_post", platform: "bluesky", format: "post", width: 2000, height: 2000, mime: "image/png", quality: None },
    PlatformPreset { id: "threads_post", platform: "threads", format: "post", width: 1080, height: 1350, mime: "image/png", quality: None },
    PlatformPreset { id: "fb_post", platform: "facebook", format: "post", width: 1200, height: 630, mime: "image/jpeg", quality: Some(0.90) },
    PlatformPreset { id: "fb_story", platform: "facebook", format: "story", width: 1080, height: 1920, mime: "image/png", quality: None },
];

pub fn get_preset(id: &str) -> Option<&'static PlatformPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

pub fn list_presets() -> Vec<&'static PlatformPreset> {
    PRESETS.to_vec()
}

impl PlatformPreset {
    pub fn to_target(&self) -> ExportTarget {
        ExportTarget {
            id: self.id.to_string(),
            platform: self.platform.to_string(),
            format: self.format.to_string(),
            width: self.width,
            height: self.height,
            mime: self.mime.to_string(),
            quality: self.quality,
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | head -20`
Expected: compiles with no errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/export/presets.rs
git commit -m "feat(export): add platform presets for 13 social media targets"
```

---

### Task 3: Manifest Validation

**Files:**
- Modify: `src-tauri/src/export/validate.rs`

- [ ] **Step 1: Write validation logic**

```rust
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

pub fn validate_manifest(manifest: &ExportManifest) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // kind must be correct
    if manifest.kind != "imageview-story/v1" {
        errors.push(ValidationError {
            path: "/kind".to_string(),
            message: format!("Expected 'imageview-story/v1', got '{}'", manifest.kind),
        });
    }

    // schema_version must be 1
    if manifest.schema_version != 1 {
        errors.push(ValidationError {
            path: "/schema_version".to_string(),
            message: format!("Expected schema_version 1, got {}", manifest.schema_version),
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
        // quality only for JPEG/WebP
        if target.quality.is_some() && target.mime != "image/jpeg" && target.mime != "image/webp" {
            errors.push(ValidationError {
                path: format!("/targets/{}/quality", i),
                message: format!("Quality only valid for JPEG/WebP, not '{}'", target.mime),
            });
        }
    }

    // Unique slide IDs
    let mut slide_ids = HashSet::new();
    for (i, slide) in manifest.slides.iter().enumerate() {
        if !slide_ids.insert(&slide.id) {
            errors.push(ValidationError {
                path: format!("/slides/{}", i),
                message: format!("Duplicate slide ID: '{}'", slide.id),
            });
        }

        // asset_id must reference an existing asset
        if !manifest.assets.iter().any(|a| a.id == slide.image.asset_id) {
            errors.push(ValidationError {
                path: format!("/slides/{}/image/asset_id", i),
                message: format!("Asset '{}' not found in assets array", slide.image.asset_id),
            });
        }

        // slide targets must reference existing target IDs
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

        // focal_point in range 0..1
        if let Some(ref fp) = slide.image.focal_point {
            if fp.x < 0.0 || fp.x > 1.0 || fp.y < 0.0 || fp.y > 1.0 {
                errors.push(ValidationError {
                    path: format!("/slides/{}/image/focal_point", i),
                    message: format!("focal_point x/y must be 0..1, got ({}, {})", fp.x, fp.y),
                });
            }
        }

        // template must be valid
        let valid_templates = ["terminal", "editorial", "bleed"];
        if let Some(ref tmpl) = slide.template {
            if !valid_templates.contains(&tmpl.as_str()) {
                errors.push(ValidationError {
                    path: format!("/slides/{}/template", i),
                    message: format!("Invalid template '{}', expected one of: {:?}", tmpl, valid_templates),
                });
            }
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

    // agent_tasks must reference existing slides
    for (i, task) in manifest.agent_tasks.iter().enumerate() {
        if !slide_ids.contains(&task.slide_id) {
            errors.push(ValidationError {
                path: format!("/agent_tasks/{}", i),
                message: format!("Slide '{}' not found", task.slide_id),
            });
        }
    }

    // default template must be valid
    let valid_templates = ["terminal", "editorial", "bleed"];
    if !valid_templates.contains(&manifest.defaults.template.as_str()) {
        errors.push(ValidationError {
            path: "/defaults/template".to_string(),
            message: format!("Invalid default template '{}'", manifest.defaults.template),
        });
    }

    // Warnings
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | head -20`
Expected: compiles with no errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/export/validate.rs
git commit -m "feat(export): add manifest validation with constraint checking"
```

---

### Task 4: JSON Patch with Contract Enforcement

**Files:**
- Modify: `src-tauri/src/export/patch.rs`

- [ ] **Step 1: Write patch application logic**

```rust
use crate::export::manifest::ExportManifest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPatch {
    pub op: String,     // "replace", "add", "remove"
    pub path: String,   // JSON Pointer, e.g. "/slides/0/text/headline"
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
    // Direct immutable paths
    if path == "/kind" || path == "/source" || path == "/targets" {
        return true;
    }
    if path.starts_with("/source/") || path.starts_with("/targets/") {
        return true;
    }
    // /slides/N/image/asset_id is immutable
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 5 && parts[1] == "slides" && parts[3] == "image" && parts[4] == "asset_id" {
        return true;
    }
    false
}

fn is_source_asset_mutation(path: &str, manifest: &ExportManifest) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    // /assets/N/... where asset is kind=source
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
        // Check immutable paths
        if is_immutable_path(&patch.path) {
            rejected.push(RejectedPatch {
                patch,
                reason: "Path is immutable".to_string(),
            });
            continue;
        }

        // Check source asset mutation
        if is_source_asset_mutation(&patch.path, &manifest) {
            rejected.push(RejectedPatch {
                patch,
                reason: "Source assets are immutable".to_string(),
            });
            continue;
        }

        // For non-append operations, check mutability
        let is_asset_append = patch.path == "/assets/-" && patch.op == "add";
        if !is_asset_append && !is_mutable_path(&patch.path) {
            rejected.push(RejectedPatch {
                patch,
                reason: "Path is not in mutable_paths".to_string(),
            });
            continue;
        }

        // Apply the patch
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
                        // Append to array
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
            _ => {
                rejected.push(RejectedPatch {
                    patch,
                    reason: format!("Unsupported operation '{}'. Supported: replace, add", patch.op),
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | head -20`
Expected: compiles with no errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/export/patch.rs
git commit -m "feat(export): add JSON patch application with contract enforcement"
```

---

### Task 5: Tauri Export Commands

**Files:**
- Create: `src-tauri/src/commands/export.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the Tauri command handlers**

```rust
// src-tauri/src/commands/export.rs
use tauri::State;
use crate::AppState;
use crate::export::manifest::*;
use crate::export::presets;
use crate::export::validate;
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

    // Resolve target presets
    for preset_id in &target_presets {
        if let Some(preset) = presets::get_preset(preset_id) {
            manifest.targets.push(preset.to_target());
        } else {
            return Err(format!("Unknown preset: '{}'", preset_id));
        }
    }

    // Resolve images to assets and slides
    let id_refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    let images = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;

    for img in &images {
        let asset_id = format!("asset_src_{}", img.image.id.replace("-", "")[..8].to_string());
        let uri = format!("imageview://images/{}/original", img.image.id);

        manifest.assets.push(Asset {
            id: asset_id.clone(),
            kind: "source".to_string(),
            uri,
            mime: format!("image/{}", img.image.format),
            width: img.image.width,
            height: img.image.height,
            provenance: None,
        });

        let slide_id = format!("slide_{}", manifest.slides.len() + 1);
        let slide_id_padded = format!("slide_{:03}", manifest.slides.len() + 1);

        manifest.slides.push(Slide {
            id: slide_id_padded.clone(),
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

        // Add agent tasks for empty text fields
        manifest.agent_tasks.push(AgentTask {
            slide_id: slide_id_padded.clone(),
            field: "text.headline".to_string(),
            task: "fill".to_string(),
            required: true,
            max_chars: Some(72),
        });
        manifest.agent_tasks.push(AgentTask {
            slide_id: slide_id_padded.clone(),
            field: "text.body".to_string(),
            task: "fill".to_string(),
            required: false,
            max_chars: Some(220),
        });
        manifest.agent_tasks.push(AgentTask {
            slide_id: slide_id_padded.clone(),
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
    // Parse imageview:// URI
    // Format: imageview://images/{image_id}/original
    let stripped = uri.strip_prefix("imageview://images/")
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
            Ok(AssetResponse {
                path: img.path.clone(),
                mime: format!("image/{}", img.image.format),
                width: img.image.width,
                height: img.image.height,
            })
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
            // "preview" — use 800px thumbnail
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
```

- [ ] **Step 2: Register the export module in commands/mod.rs**

Add to `src-tauri/src/commands/mod.rs`:

```rust
pub mod export;
```

- [ ] **Step 3: Register export commands in lib.rs**

Add these lines inside the `tauri::generate_handler![...]` block in `src-tauri/src/lib.rs`, after the last `commands::vision::*` entry:

```rust
commands::export::create_export_manifest,
commands::export::validate_export_manifest,
commands::export::apply_export_patches,
commands::export::list_export_presets,
commands::export::get_export_asset,
```

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | head -20`
Expected: compiles with no errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/export.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(export): add Tauri commands for manifest CRUD and asset resolution"
```

---

### Task 6: TypeScript Types

**Files:**
- Create: `src/lib/export-types.ts`

- [ ] **Step 1: Write TypeScript types mirroring Rust manifest types**

```typescript
// src/lib/export-types.ts

export interface ExportManifest {
    kind: string;
    schema_version: number;
    id: string;
    title: string;
    locale: string;
    created_at: string;
    updated_at: string;
    source: ManifestSource;
    defaults: ManifestDefaults;
    targets: ExportTarget[];
    slides: Slide[];
    assets: Asset[];
    agent_tasks: AgentTask[];
    agent_hints: AgentHints;
    agent_contract: AgentContract;
}

export interface ManifestSource {
    app: string;
    collection_id: string | null;
    image_ids: string[];
}

export interface ManifestDefaults {
    template: 'terminal' | 'editorial' | 'bleed';
    fonts: { serif: string; mono: string };
    colors: {
        preset: 'light' | 'dark';
        background: string;
        foreground: string;
        accent: string;
    };
    safe_area: { top: number; right: number; bottom: number; left: number };
}

export interface ExportTarget {
    id: string;
    platform: string;
    format: string;
    width: number;
    height: number;
    mime: string;
    quality?: number;
}

export interface Slide {
    id: string;
    template?: string;
    targets?: string[];
    image: {
        asset_id: string;
        fit: 'cover' | 'contain' | 'fill';
        focal_point?: { x: number; y: number };
    };
    text: {
        headline: string;
        body: string;
        caption: string;
    };
    overlay: {
        position: string;
        scrim: { type: string; direction: string; from: string; to: string };
        text_color: string;
    };
    metadata: {
        rating?: number;
        tags: string[];
        alt: string;
    };
}

export interface Asset {
    id: string;
    kind: 'source' | 'generated';
    uri: string;
    mime: string;
    width: number;
    height: number;
    provenance?: {
        provider: string;
        model: string;
        prompt: string;
        thinking?: boolean;
        reference_assets: string[];
        created_at: string;
    };
}

export interface AgentTask {
    slide_id: string;
    field: string;
    task: string;
    required: boolean;
    max_chars?: number;
}

export interface AgentHints {
    tone: string;
    allow_generated_images: boolean;
    language: string;
}

export interface AgentContract {
    mutable_paths: string[];
    append_only: string[];
    immutable_paths: string[];
}

export interface JsonPatch {
    op: 'replace' | 'add' | 'remove';
    path: string;
    value?: unknown;
}

export interface PatchResult {
    manifest: ExportManifest;
    applied_patches: JsonPatch[];
    rejected_patches: { patch: JsonPatch; reason: string }[];
}

export interface ValidationResult {
    valid: boolean;
    errors: { path: string; message: string }[];
    warnings: { path: string; message: string }[];
}

export interface PresetInfo {
    id: string;
    platform: string;
    format: string;
    width: number;
    height: number;
    mime: string;
}

export interface AssetResponse {
    path: string;
    mime: string;
    width: number;
    height: number;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/export-types.ts
git commit -m "feat(export): add TypeScript types mirroring manifest schema"
```

---

### Task 7: TypeScript API Wrappers

**Files:**
- Create: `src/lib/export-api.ts`

- [ ] **Step 1: Write API wrapper functions**

```typescript
// src/lib/export-api.ts
import { invoke } from '@tauri-apps/api/core';
import type {
    ExportManifest,
    JsonPatch,
    PatchResult,
    ValidationResult,
    PresetInfo,
    AssetResponse,
} from './export-types';

export async function createExportManifest(
    imageIds: string[],
    targetPresets: string[],
    collectionId?: string,
    template?: string,
): Promise<ExportManifest> {
    return invoke<ExportManifest>('create_export_manifest', {
        imageIds,
        targetPresets,
        collectionId: collectionId ?? null,
        template: template ?? null,
    });
}

export async function validateExportManifest(
    manifest: ExportManifest,
): Promise<ValidationResult> {
    return invoke<ValidationResult>('validate_export_manifest', { manifest });
}

export async function applyExportPatches(
    manifest: ExportManifest,
    patches: JsonPatch[],
): Promise<PatchResult> {
    return invoke<PatchResult>('apply_export_patches', { manifest, patches });
}

export async function listExportPresets(): Promise<PresetInfo[]> {
    return invoke<PresetInfo[]>('list_export_presets');
}

export async function getExportAsset(
    uri: string,
    variant?: 'original' | 'preview' | 'thumbnail',
    maxWidth?: number,
    maxHeight?: number,
): Promise<AssetResponse> {
    return invoke<AssetResponse>('get_export_asset', {
        uri,
        variant: variant ?? null,
        maxWidth: maxWidth ?? null,
        maxHeight: maxHeight ?? null,
    });
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/export-api.ts
git commit -m "feat(export): add TypeScript API wrappers for export commands"
```

---

### Task 8: Bundle EB Garamond Fonts

**Files:**
- Create: `static/fonts/EBGaramond-Regular.woff2`
- Create: `static/fonts/EBGaramond-Medium.woff2`
- Create: `static/fonts/EBGaramond-Bold.woff2`
- Create: `static/fonts/EBGaramond-Italic.woff2`
- Modify: `src/app.css`

- [ ] **Step 1: Download EB Garamond woff2 files**

Download from Google Fonts CDN. The files go into `static/fonts/`:

```bash
curl -L -o static/fonts/EBGaramond-Regular.woff2 "https://fonts.gstatic.com/s/ebgaramond/v29/SlGDmQSNjdsmc35JDF1K5E55YMjF_7DPuGi-2fA.woff2"
curl -L -o static/fonts/EBGaramond-Medium.woff2 "https://fonts.gstatic.com/s/ebgaramond/v29/SlGDmQSNjdsmc35JDF1K5E55YMjF_7DPuGi-6PA.woff2"
curl -L -o static/fonts/EBGaramond-Bold.woff2 "https://fonts.gstatic.com/s/ebgaramond/v29/SlGDmQSNjdsmc35JDF1K5E55YMjF_7DPuGi-AfA.woff2"
curl -L -o static/fonts/EBGaramond-Italic.woff2 "https://fonts.gstatic.com/s/ebgaramond/v29/SlGFmQSNjdsmc35JDF1K5GRwUjcdlttVFm-rI7e8QI96WamXlYXs.woff2"
```

Note: Google Fonts URLs can change. If these fail, download from https://fonts.google.com/specimen/EB+Garamond and convert to woff2, or use the npm package `@fontsource/eb-garamond`.

- [ ] **Step 2: Add @font-face declarations to app.css**

Add after the existing JetBrains Mono @font-face blocks (after line 21 in `src/app.css`):

```css
@font-face {
    font-family: 'EB Garamond';
    font-style: normal;
    font-weight: 400;
    font-display: swap;
    src: url('/fonts/EBGaramond-Regular.woff2') format('woff2');
}
@font-face {
    font-family: 'EB Garamond';
    font-style: normal;
    font-weight: 500;
    font-display: swap;
    src: url('/fonts/EBGaramond-Medium.woff2') format('woff2');
}
@font-face {
    font-family: 'EB Garamond';
    font-style: normal;
    font-weight: 700;
    font-display: swap;
    src: url('/fonts/EBGaramond-Bold.woff2') format('woff2');
}
@font-face {
    font-family: 'EB Garamond';
    font-style: italic;
    font-weight: 400;
    font-display: swap;
    src: url('/fonts/EBGaramond-Italic.woff2') format('woff2');
}
```

- [ ] **Step 3: Add CSS custom property for serif font**

Add to the `:root` block in `src/app.css`:

```css
--font-serif: 'EB Garamond', 'Georgia', 'Times New Roman', serif;
```

- [ ] **Step 4: Commit**

```bash
git add static/fonts/EBGaramond-*.woff2 src/app.css
git commit -m "feat(export): bundle EB Garamond fonts and add CSS declarations"
```

---

### Task 9: Integration Smoke Test

**Files:**
- No new files — verify end-to-end compilation and basic functionality.

- [ ] **Step 1: Full cargo build check**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors.

- [ ] **Step 2: Full frontend build check**

Run: `npm run check 2>&1 | tail -10`
Expected: no TypeScript errors.

- [ ] **Step 3: Verify the app starts**

Run: `npm run tauri dev` and verify:
1. App launches without errors
2. No console errors related to export commands
3. Fonts directory contains both JetBrains Mono and EB Garamond files

- [ ] **Step 4: Final commit if any fixups were needed**

```bash
git add -A
git commit -m "fix(export): integration fixups from smoke test"
```

Only commit if there were actual fixups. Skip if everything passed.
