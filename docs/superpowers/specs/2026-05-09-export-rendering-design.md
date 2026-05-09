# Export Rendering Design Spec (Phase 2)

## Overview

HTML template rendering for the export pipeline. Three template styles (Terminal, Editorial, Bleed), html-to-image capture, a quick export UI to test with real images, and PDF assembly for LinkedIn carousels.

## Template Components

### Shared contract

Each template component receives:
```typescript
{
  slide: Slide;
  defaults: ManifestDefaults;
  target: ExportTarget;  // determines render dimensions
}
```

Each renders at exactly `target.width × target.height` pixels. No scrolling, no overflow. The component IS the slide.

### Terminal

- Background: `defaults.colors.background` (dark by default: #08080c)
- All text: JetBrains Mono (`--font`)
- Left accent bar: 4px wide, `defaults.colors.accent` color
- Image: small, top-right corner, 40% width, rounded corners
- Headline: large (48px), bold, `defaults.colors.foreground`
- Body: medium (24px), regular weight, `--text-secondary` color
- Caption: bottom-left, small monospace, dimmed
- Safe area: respected from `defaults.safe_area`

### Editorial

- Background: `defaults.colors.background` (light by default: #f7f3ea)
- Headline: EB Garamond (`--font-serif`), 56px, bold
- Body: EB Garamond, 28px, regular, generous line-height (1.6)
- Caption/labels: JetBrains Mono, 16px, uppercase, letter-spacing
- Image: centered, 80% width, with subtle border
- Wide margins (safe_area), clean whitespace
- Accent used for divider lines

### Bleed

- Image: fills entire frame, `object-fit: cover` with `object-position` from focal_point
- Gradient scrim: from slide.overlay.scrim settings, rendered as CSS linear-gradient
- Text: overlaid on scrim area, position from slide.overlay.position
- Headline: EB Garamond, 48px, bold, slide.overlay.text_color
- Body: EB Garamond, 24px, same color
- Caption: JetBrains Mono, 14px, slightly transparent
- No visible margins — image bleeds to edge

## Export.svelte Component

### Layout

Minimal quick-export view:
- Top bar: template selector (three buttons: Terminal / Editorial / Bleed), platform preset dropdown, Export button
- Main area: grid of slide previews (scaled down, maintaining aspect ratio)
- Each preview shows the actual template rendering at reduced scale

### State

- Reads `$selectedIds` from stores — if empty, shows message "Select images in grid first"
- Creates manifest on mount via `createExportManifest(imageIds, [presetId], template)`
- Stores manifest in local component state
- Template/preset changes regenerate the manifest

### Export flow

1. User clicks "Export All" or "Export PDF"
2. For images: renders each slide at full resolution in hidden container, captures via `html-to-image` `toPng()`, triggers download or save dialog
3. For PDF: captures all slides as PNGs, sends paths to Rust `assemble_pdf` command, saves PDF

## Capture Pipeline

### html-to-image integration

Install: `npm install html-to-image`

Render flow:
1. Create a hidden `<div>` with exact pixel dimensions (`width: {target.width}px; height: {target.height}px`)
2. Mount the template component inside it
3. Wait for images to load (use html-to-image's `{ cacheBust: true }` option)
4. Call `toPng(element, { width, height, pixelRatio: 1 })` 
5. Convert data URL to blob, trigger download

### PDF Assembly (Rust)

New Tauri command:

```rust
#[tauri::command]
pub async fn assemble_export_pdf(
    image_paths: Vec<String>,
    width_mm: f32,
    height_mm: f32,
    output_path: String,
) -> Result<String, String>
```

Uses `printpdf` crate:
- Create PDF document
- For each image path: read PNG, add as full-page image
- Page size: convert from pixels to mm (assuming 72 DPI for screen content)
- Save to output_path
- Return output_path on success

## New Dependencies

- `html-to-image` (npm) — DOM to PNG capture
- `printpdf` (Rust crate) — PDF generation from raster images

## Files

### New
- `src/lib/components/ExportSlideTerminal.svelte`
- `src/lib/components/ExportSlideEditorial.svelte`
- `src/lib/components/ExportSlideBleed.svelte`
- `src/lib/components/Export.svelte`
- `src-tauri/src/export/pdf.rs`

### Modified
- `src/routes/+page.svelte` — add Export component routing
- `src-tauri/src/commands/export.rs` — add assemble_export_pdf command
- `src-tauri/src/lib.rs` — register new command
- `src-tauri/Cargo.toml` — add printpdf dependency
- `package.json` — add html-to-image
- `src-tauri/src/export/mod.rs` — add `pub mod pdf;`

## Image Resolution in Templates

Templates need to display actual images. The `Slide.image.asset_id` references an asset with a `imageview://` URI. For rendering in the webview:

1. On manifest creation, the command already resolves image IDs to assets
2. For display, use Tauri's asset protocol: `convertFileSrc()` from `@tauri-apps/api/core` converts absolute file paths to `asset://` URLs the webview can render
3. The Export component calls `getExportAsset(uri, 'original')` to get the file path, then `convertFileSrc(path)` for the `<img src>`

## Design Decisions

1. **Quick export over full UI** — get testing fast, add template picker polish later
2. **html-to-image over webview screenshot** — more reliable cross-platform, no Tauri API gaps
3. **PNG-first PDF** — render slides as PNG, embed in PDF, avoids font rendering issues
4. **Hidden render container** — full-res rendering happens off-screen, previews are scaled copies
5. **convertFileSrc for images** — standard Tauri pattern for displaying local files in webview
