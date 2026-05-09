# Export Pipeline Design Spec

## Overview

Social media export pipeline for ImageView. Enables curated images to be exported as platform-ready content (sized images, PDF carousels) with optional AI-powered text and image generation via MCP tools.

**Priority:** Manifest spec + MCP tools first, UI later.

## Architecture

```
ImageView App (curation) ──► Manifest (JSON contract)
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
              MCP Tools      Agent SDK        CLI/Skills
              (enrich,       (orchestrate     (gpt-image-2,
               render)        pipeline)        de-ai, etc.)
                    │               │               │
                    └───────────────┼───────────────┘
                                    ▼
                            Rendered Output
                         (PNGs, PDFs, ZIPs)
```

## Manifest Schema: `imageview-story/v1`

### Envelope

```json
{
  "kind": "imageview-story/v1",
  "schema_version": 1,
  "id": "story_2026_05_09_001",
  "title": "May selects",
  "locale": "en",
  "created_at": "2026-05-09T12:00:00Z",
  "updated_at": "2026-05-09T12:00:00Z"
}
```

### Source

```json
"source": {
  "app": "imageview",
  "collection_id": "col_xyz",
  "image_ids": ["img_001", "img_002", "img_003"]
}
```

### Defaults

```json
"defaults": {
  "template": "editorial",
  "fonts": { "serif": "EB Garamond", "mono": "JetBrains Mono" },
  "colors": {
    "preset": "light",
    "background": "#f7f3ea",
    "foreground": "#171717",
    "accent": "#c6422b"
  },
  "safe_area": { "top": 96, "right": 72, "bottom": 96, "left": 72 }
}
```

Templates: `"terminal"` | `"editorial"` | `"bleed"`

Color presets:
- `light`: #f7f3ea bg, #171717 fg, #c6422b accent
- `dark`: #08080c bg, #e0e0e0 fg, #9ece6a accent (matches app theme)

### Targets

Each target has a stable ID for progress tracking and output references.

```json
"targets": [
  { "id": "ig_carousel", "platform": "instagram", "format": "carousel", "width": 1080, "height": 1350, "mime": "image/png", "quality": 0.92 },
  { "id": "ig_story", "platform": "instagram", "format": "story", "width": 1080, "height": 1920, "mime": "image/png" },
  { "id": "li_pdf", "platform": "linkedin", "format": "pdf_carousel", "width": 1080, "height": 1350, "mime": "application/pdf" },
  { "id": "tw_post", "platform": "twitter", "format": "post", "width": 1600, "height": 900, "mime": "image/jpeg", "quality": 0.90 },
  { "id": "tt_story", "platform": "tiktok", "format": "story", "width": 1080, "height": 1920, "mime": "image/png" },
  { "id": "pin", "platform": "pinterest", "format": "pin", "width": 1000, "height": 1500, "mime": "image/png" },
  { "id": "tg_post", "platform": "telegram", "format": "post", "width": 1280, "height": 1280, "mime": "image/jpeg", "quality": 0.85 },
  { "id": "yt_thumb", "platform": "youtube", "format": "thumbnail", "width": 1280, "height": 720, "mime": "image/png" },
  { "id": "bsky_post", "platform": "bluesky", "format": "post", "width": 2000, "height": 2000, "mime": "image/png" },
  { "id": "threads_post", "platform": "threads", "format": "post", "width": 1080, "height": 1350, "mime": "image/png" }
]
```

Custom presets supported for niche platforms (e.g., FreeFeed).

### Slides

```json
"slides": [
  {
    "id": "slide_001",
    "template": "bleed",
    "targets": ["ig_carousel", "li_pdf"],
    "image": {
      "asset_id": "asset_src_001",
      "fit": "cover",
      "focal_point": { "x": 0.5, "y": 0.45 }
    },
    "text": {
      "headline": "",
      "body": "",
      "caption": "Berlin, 2026"
    },
    "overlay": {
      "position": "bottom-left",
      "scrim": {
        "type": "linear",
        "direction": "to-top",
        "from": "rgba(0,0,0,0)",
        "to": "rgba(0,0,0,0.72)"
      },
      "text_color": "#ffffff"
    },
    "metadata": {
      "rating": 5,
      "tags": ["sunset", "berlin"],
      "alt": ""
    }
  }
]
```

- `template` per slide overrides the default
- `targets` per slide filters which platforms include this slide
- `fit`: `"cover"` | `"contain"` | `"fill"`
- `position`: `"bottom-left"` | `"bottom-right"` | `"top-left"` | `"top-right"` | `"center"` | `"none"`
- Array order is authoritative for slide sequence

### Assets

Unified array for source and generated assets. Source assets are immutable.

```json
"assets": [
  {
    "id": "asset_src_001",
    "kind": "source",
    "uri": "imageview://images/img_001/original",
    "mime": "image/jpeg",
    "width": 4032,
    "height": 3024
  },
  {
    "id": "asset_gen_001",
    "kind": "generated",
    "uri": "imageview://exports/job_abc/assets/asset_gen_001.png",
    "mime": "image/png",
    "width": 1080,
    "height": 1350,
    "provenance": {
      "provider": "openai",
      "model": "gpt-image-2",
      "prompt": "Editorial style photo with warm tones...",
      "thinking": true,
      "reference_assets": ["asset_src_001"],
      "created_at": "2026-05-09T12:05:00Z"
    }
  }
]
```

### Agent Tasks

Explicit list of fields for agents to fill. Uses `slide_id` + `field` path (not array indexes).

```json
"agent_tasks": [
  { "slide_id": "slide_001", "field": "text.headline", "task": "fill", "required": true, "max_chars": 72 },
  { "slide_id": "slide_001", "field": "text.body", "task": "fill", "required": false, "max_chars": 220 },
  { "slide_id": "slide_001", "field": "metadata.alt", "task": "fill", "required": true, "max_chars": 125 }
]
```

### Agent Hints

Creative direction for agents.

```json
"agent_hints": {
  "tone": "quiet editorial",
  "allow_generated_images": true,
  "language": "en"
}
```

### Agent Contract

Mutation boundaries — immutable wins over mutable.

```json
"agent_contract": {
  "mutable_paths": ["/slides/*/text/*", "/slides/*/metadata/alt", "/slides/*/overlay"],
  "append_only": ["/assets"],
  "immutable_paths": ["/kind", "/source", "/targets", "/slides/*/image/asset_id", "/assets/*[kind=source]"]
}
```

Wildcard paths use ImageView's own glob matcher (not RFC 6901 JSON Pointer). Source assets cannot be modified or deleted by agents.

## MCP Tools

### `imageview_export_create_manifest`

Build a manifest from selected images.

```
Input:  { image_ids?: string[], collection_id?: string, target_presets: string[], template?: string }
Output: { manifest: ExportManifest }
```

Resolves images to assets with `imageview://` URIs. Populates `agent_tasks` for empty text fields.

### `imageview_export_validate_manifest`

Schema and constraint validation.

```
Input:  { manifest: ExportManifest }
Output: { valid: bool, errors: ValidationError[], warnings: ValidationWarning[] }
```

Validates: unique IDs, asset references exist, agent_tasks point to mutable fields, quality only for JPEG/WebP, focal_point 0..1, safe_area within target dimensions, required fields non-empty after enrichment.

### `imageview_export_apply_patches`

Apply agent changes to a manifest using JSON Patch (RFC 6902).

```
Input:  { manifest: ExportManifest, patches: JsonPatch[] }
Output: { manifest: ExportManifest, applied_patches: JsonPatch[], rejected_patches: RejectedPatch[] }
```

Rejects patches to immutable paths. Rejects mutations to source assets. Allows appending generated assets.

### `imageview_export_enrich_manifest`

App-hosted AI enrichment (uses Claude/Agent SDK internally).

```
Input:  { manifest: ExportManifest, instructions?: string }
Output: { manifest: ExportManifest, patches: JsonPatch[], notes: string[] }
```

Fills agent_tasks using AI. Returns patches for transparency/review.

### `imageview_export_render`

Produce final output files.

```
Input:  { manifest: ExportManifest, target_ids?: string[] }
Output: { job_id: string, outputs: RenderOutput[] }
```

```json
{
  "job_id": "export_abc",
  "outputs": [
    {
      "target_id": "ig_carousel",
      "kind": "image_sequence",
      "files": [
        { "slide_id": "slide_001", "uri": "imageview://exports/export_abc/ig_carousel/slide_001.png", "mime": "image/png", "width": 1080, "height": 1350, "bytes": 842102 }
      ]
    },
    {
      "target_id": "li_pdf",
      "kind": "pdf",
      "uri": "imageview://exports/export_abc/li_pdf/carousel.pdf",
      "pages": 3,
      "mime": "application/pdf",
      "bytes": 2540000
    }
  ]
}
```

Progress events: `export-progress { job_id, current, total, phase, slide_id, target_id }`, `export-complete { job_id, outputs }`, `export-error { job_id, error }`.

Renders to app data dir by default. No arbitrary filesystem writes for MCP agents.

### `imageview_export_get_asset`

Resolve `imageview://` URI to image data.

```
Input:  { uri: string, variant?: "original" | "preview" | "thumbnail", max_width?: number, max_height?: number }
Output: { uri: string, mime: string, width: number, height: number }
```

Default variant: `"preview"`. Returns a temporary file URI. Only returns `data_uri` if explicitly requested. Prevents large base64 payloads.

## Rendering Pipeline

### HTML Templates

Three base templates, each with light and dark variants:

**Terminal** — monospace identity
- JetBrains Mono for all text
- Dark bg (#08080c), colored accent bar (green/blue/purple from app palette)
- Grid/prompt-like layout, metadata rows
- Best for: quotes, stats, announcements

**Editorial** — serif readability
- EB Garamond headlines + body, JetBrains Mono for labels/accents
- Light bg for LinkedIn, generous margins, large line-height
- Best for: carousels, long-form, thought leadership

**Bleed** — image-forward
- Full-frame image with `object-fit: cover` + focal point
- Gradient scrim overlay, minimal text
- Best for: photography, visual stories

### Rendering Process

1. Hidden Svelte route `/export/render` receives manifest
2. Renders one slide at a time at exact pixel dimensions
3. `html-to-image` library captures DOM to PNG
4. PNG bytes sent to Rust backend
5. Rust writes files, assembles PDFs (via `printpdf`), emits progress events

### Fonts

- JetBrains Mono: already bundled in `static/fonts/`
- EB Garamond: add Regular, Medium, Bold, Italic woff2 to `static/fonts/`

### PDF Assembly

LinkedIn PDF carousels: render each slide as PNG first, embed full-page in PDF via `printpdf` crate. Avoids font/layout discrepancies in Rust PDF text rendering.

## Rust Backend

### New Files

```
src-tauri/src/commands/export.rs     — Tauri command handlers
src-tauri/src/db_core/export.rs      — Export job persistence (optional)
src-tauri/src/export/mod.rs          — Module root
src-tauri/src/export/manifest.rs     — Manifest DTOs, serde types
src-tauri/src/export/render.rs       — Job orchestration, output assembly
src-tauri/src/export/pdf.rs          — PDF assembly
src-tauri/src/export/presets.rs      — Platform preset definitions
```

### Commands

Follow existing pattern from `commands/import.rs`:

```rust
#[tauri::command]
pub async fn create_export_manifest(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
    target_presets: Vec<String>,
    template: Option<String>,
) -> Result<ExportManifest, String> { ... }

#[tauri::command]
pub async fn validate_export_manifest(
    manifest: ExportManifest,
) -> Result<ManifestValidation, String> { ... }

#[tauri::command]
pub async fn apply_export_patches(
    manifest: ExportManifest,
    patches: Vec<JsonPatch>,
) -> Result<PatchResult, String> { ... }

#[tauri::command]
pub async fn render_export(
    app: AppHandle,
    state: State<'_, AppState>,
    manifest: ExportManifest,
    target_ids: Option<Vec<String>>,
) -> Result<ExportJobResult, String> { ... }

#[tauri::command]
pub async fn get_export_asset(
    state: State<'_, AppState>,
    uri: String,
    variant: Option<String>,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<AssetResponse, String> { ... }
```

### New Crate Dependencies

- `printpdf` — PDF generation
- `schemars` — JSON Schema generation from Rust types
- `jsonschema` — manifest validation
- `zip` — ZIP bundle output
- `sanitize-filename` — safe output filenames

### Capabilities

Add `fs` plugin to `default.json` for writing export files to app data directory.

## Phase Plan

### Phase 1: Manifest + MCP (this spec)
- Manifest Rust types + JSON Schema
- Platform presets
- Tauri commands: create_manifest, validate_manifest, apply_patches, get_asset
- MCP tool wrappers
- EB Garamond font bundle

### Phase 2: Rendering
- HTML templates (Terminal, Editorial, Bleed)
- `html-to-image` integration
- PDF assembly
- render_export command
- Progress events

### Phase 3: Agent Integration
- enrich_manifest command (app-hosted AI enrichment)
- GPT Image 2 skill adapter
- Agent SDK orchestrator example

### Phase 4: Export UI
- Export.svelte component
- Image selection from collections
- Template/platform picker
- Preview
- Download/save

## Validation Invariants

- Unique: manifest ID, target IDs, slide IDs, asset IDs
- Every `slide.image.asset_id` references an existing asset
- Every `slide.targets[]` entry references an existing target ID
- Every `agent_task.slide_id` references an existing slide
- `quality` only valid for JPEG/WebP targets
- `focal_point.x/y` in range 0..1
- `safe_area` values non-negative and smaller than target dimensions
- Required agent_tasks fields non-empty after enrichment (whitespace-trimmed)
- Source assets immutable via patches

## Design Decisions Log

1. **agent_tasks over null convention** — Explicit fill instructions avoid ambiguity between "empty" and "please fill"
2. **imageview:// URIs over absolute paths** — MCP agents don't need filesystem access
3. **slide_id + field over JSON Pointer indexes** — Stable references survive reordering
4. **apply_patches separate from enrich_manifest** — External agents apply patches, app-hosted AI enriches
5. **HTML rendering over Rust rendering** — Better typographic control, EB Garamond + JetBrains Mono
6. **PNG-first PDF assembly** — Avoids font rendering issues in Rust PDF libraries
7. **Preview variant default for get_asset** — Prevents large payload issues in MCP
8. **Manifest-first, UI-later** — Get the contract right, everything else builds on it
