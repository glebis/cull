# RAW File Support Module

Date: 2026-05-11
Status: Approved (revised after Codex audit)

## Overview

Add RAW camera file support (RAF, CR2, CR3, NEF, ARW, DNG, ORF, RW2) as the first toggleable module in Cull. V1 extracts embedded JPEG previews for thumbnails and viewing — no full sensor decode.

## Module System

Cull introduces a settings-driven module system. Each module has a settings key, a UI toggle, and a clear boundary for what it enables.

**RAW module specifics:**

- Settings key: `module_raw`
- Default: `false` (opt-in)
- When disabled: RAW extensions are excluded from all extension checks. RAW files are invisible to the app.
- When enabled: RAW extensions are included. Import, thumbnailing, and viewing work for RAW files.
- UI: Toggle in Settings > Modules section. When toggled on, offer to rescan library roots for RAW files.

### Extension filtering — single source of truth

**Problem identified by audit:** Extension lists are scattered across 5 locations:
1. `import.rs` — `SUPPORTED_EXTENSIONS` and `DECODABLE_EXTENSIONS`
2. `commands/import.rs` — folder-import allowlist
3. `watcher.rs` — event filter
4. `lib.rs` — `IMAGE_EXTENSIONS` and `is_image_path()`

**Solution:** Create `src-tauri/src/extensions.rs` with a single API:

```rust
pub const BASE_IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif",
    "heic", "heif", "avif", "svg", "ico", "psd",
];

pub const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "nef", "arw", "dng", "orf", "raf", "rw2",
];

pub const BASE_DECODABLE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif",
];

pub fn supported_extensions(module_raw: bool) -> Vec<&'static str> {
    let mut exts = BASE_IMAGE_EXTENSIONS.to_vec();
    if module_raw { exts.extend_from_slice(RAW_EXTENSIONS); }
    exts
}

pub fn is_image_path(path: &Path, module_raw: bool) -> bool { ... }
pub fn is_decodable(ext: &str, module_raw: bool) -> bool { ... }
pub fn is_raw_extension(ext: &str) -> bool { ... }
```

All 5 existing sites are refactored to call this module. The watcher receives `module_raw` state at startup and when the setting changes (via event).

PSD stays in `BASE_IMAGE_EXTENSIONS` — it's not a RAW camera format.

## Decoder Architecture

Two-tier decoder behind a `RawDecoder` trait:

```
RawDecoder trait
├── FujiRafDecoder  (pure Rust, .raf only)
└── LibRawDecoder   (rsraw crate, all other RAW formats)
```

### RawDecoder trait

```rust
pub struct RawPreview {
    pub image: image::DynamicImage,
    pub metadata: RawMetadata,
}

pub struct RawMetadata {
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub shutter_speed: Option<String>,
    pub aperture: Option<String>,
    pub iso: Option<u32>,
    pub focal_length: Option<f32>,
    pub date_taken: Option<String>,
    pub film_simulation: Option<String>,
    pub gps: Option<(f64, f64)>,
    pub sensor_width: u32,
    pub sensor_height: u32,
}

pub trait RawDecoder: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn extract_preview(&self, path: &Path) -> Result<RawPreview, String>;
}
```

### FujiRafDecoder (pure Rust)

~100-150 lines. Parses the RAF file header to find the embedded JPEG preview offset and length, extracts the JPEG blob, decodes it via the `image` crate.

RAF file structure (simplified):
- Bytes 0-15: magic ("FUJIFILMCCD-RAW ")
- Header contains offset table with JPEG preview location + length
- JPEG preview is typically 4416x2944 (~13MP) on modern Fuji cameras (GFX 100S may be larger)
- The parser reads the embedded JPEG — it does NOT decode the compressed sensor data (that's the future full-decode path via LibRaw)
- Works for all RAF variants: uncompressed, lossless compressed, and lossy compressed — because the embedded JPEG is independent of the CFA compression method

**Implementation must include:** exact byte offset parsing, Big Endian reading, bounds checks, SOI/EOI marker validation. The spec acknowledges that "works for all RAF variants" needs verification with real GFX compressed/lossless/lossy samples.

If RAF parsing fails (corrupt header, unexpected format version), falls back to LibRawDecoder.

### LibRawDecoder (via rsraw)

Wraps the `rsraw` crate (Rust bindings to LibRaw C library). Uses thumbnail extraction API for embedded JPEG preview. Handles all RAW formats LibRaw supports.

**rsraw maturity concern (from audit):** rsraw v0.1 has 0% doc coverage on docs.rs. The actual API may be `RawImage::open(&[u8])` + `extract_thumbs()`, not `unpack_thumb()`. A build spike on Apple Silicon is required before committing to this dependency. If rsraw proves unusable, fallback plan is direct LibRaw FFI via `libraw-sys` or shelling out to `dcraw_emu -e`.

Future upgrade path: call `unpack()` + `process()` for full sensor decode.

### Decoder dispatch

```rust
pub fn decode_raw_preview(path: &Path) -> Result<RawPreview, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("raf") {
        match FujiRafDecoder.extract_preview(path) {
            Ok(preview) => return Ok(preview),
            Err(_) => {} // fall through to LibRaw
        }
    }
    LibRawDecoder.extract_preview(path)
}
```

## Integration Points

### Import pipeline (import.rs)

`sync_file()` changes:

1. Extension checks use `extensions::supported_extensions(module_raw)` and `extensions::is_decodable(ext, module_raw)`
2. When a RAW file is imported and the module is enabled, the file enters the normal import flow instead of returning `SyncOutcome::Registered`
3. `create_image_record()` calls `decode_raw_preview()` instead of `image::open()` for RAW extensions
4. Dimensions stored: **preview JPEG dimensions** for `width`/`height` fields (what the UI uses for layout). Sensor dimensions stored in `raw_metadata` JSON for display in Loupe info panel.
5. The preview `DynamicImage` is passed to `generate_thumbnail_from_image()`

### Thumbnail pipeline (thumbnails.rs)

New entry point for pre-decoded images:

```rust
pub fn generate_thumbnail_from_image(
    img: &image::DynamicImage,
    app_data_dir: &Path,
    image_id: &str,
) -> Result<PathBuf, String>
```

Shares the internal resize + JPEG save logic with `generate_thumbnail()`.

**Thumbnail regeneration commands** (`regenerate_thumbnails`, `regenerate_thumbnails_by_ids`, `regenerate_single_thumbnail`) must also use the RAW decoder path when the source file is RAW. Check `extensions::is_raw_extension()` before calling `image::open()`.

### Viewing

- Grid and Loupe show thumbnails (same as any image)
- Full-resolution viewing uses the largest extracted preview (800px thumbnail), since browsers cannot render RAW files
- **Loupe change:** When `image.format` is a RAW extension, `convertFileSrc(image.path)` must be replaced with `convertFileSrc(thumbnail_path)`. This affects Loupe.svelte, Compare.svelte, and Tinder.svelte.
- **Crop/rotate disabled for RAW in v1.** The transform commands call `image::open()` on the original file, which won't work for RAW. Disable these actions in the UI when viewing a RAW file. Full RAW editing requires the sensor decode upgrade (future).

### Source detection

RAW files are camera files by definition:
- `is_ai_generated: false`
- `source_label: "camera"` or specific camera model from EXIF
- EXIF metadata extracted and stored in `raw_metadata`
- `run_source_detection()` calls `image::open()` for dimensions — for RAW files, use the already-decoded preview dimensions instead

### ML pipelines (embeddings, detection, safety)

Object detection (YOLO), NSFW detection (NudeNet), CLIP embeddings, and Gemini embeddings all call `image::open()` on original file paths. For RAW files:
- **Use the preview thumbnail** (800px) as input instead of the original file
- The thumbnail is a valid JPEG, so `image::open(thumbnail_path)` works without changes to the ML code
- This means ML results are based on the camera's rendering, not raw sensor data — acceptable for v1

### Export

When exporting RAW files:
- `original` asset type returns the **actual RAF/CR2/etc. file** (the real original)
- `preview` asset type (new) returns the **extracted JPEG preview**
- Export manifests for RAW files should default to `preview` since most export targets can't handle RAW formats
- Store provenance as a simple string field on the manifest entry: `provenance: "raw_preview"` (avoids changing the existing AssetProvenance struct)

### File watcher

The watcher's extension filter uses `extensions::is_image_path(path, module_raw)`. The `module_raw` state is stored in an `Arc<AtomicBool>` shared between the watcher's debouncer thread and the main thread. When the setting changes, the main thread updates the atomic — no watcher restart needed, no thread leak.

### Backfill for existing registered RAW files

**Problem:** When the module is first enabled, previously imported folders may have RAW files that were either skipped (with the new "skip when disabled" behavior) or registered with width=0 (legacy behavior from before the module existed).

**Solution:** New command `backfill_raw_previews`:
1. Query images where `format` is a RAW extension AND `width = 0` (thumbnails are derived files on disk, not a DB column — check filesystem if needed)
2. For each, find the image file path, run `decode_raw_preview()`, generate thumbnail, update dimensions
3. Exposed as a Tauri command, triggered by the "rescan" toast when the module is toggled on
4. Also handles the case where the user imported a folder before enabling the module — the rescan re-walks library roots and picks up RAW files

## Metadata

RAW files carry rich EXIF metadata. Extracted fields are stored as JSON in a new `raw_metadata` column on the `images` table.

Column name: `raw_metadata` (distinct from `generation_runs.raw_metadata_json` which stores AI generation sidecar data — different concept).

Fields extracted:
- Camera model (e.g., "Fujifilm GFX 100S")
- Lens (e.g., "GF 63mm f/2.8 R WR")
- Exposure (shutter speed, aperture, ISO)
- Focal length
- GPS coordinates (if present)
- Film simulation (Fuji-specific)
- Date taken (used as `created_at` instead of file modification date)
- Sensor dimensions (distinct from preview dimensions stored in `width`/`height`)

Displayed in the Loupe info panel. For Fuji files, the pure Rust parser reads EXIF from the embedded JPEG's EXIF tags. For other formats, LibRaw provides structured metadata access.

The `Image` model struct and all queries that select from `images` must be updated to include the new column.

## New Files

```
src-tauri/src/extensions.rs   # Single source of truth for extension lists
src-tauri/src/raw/
├── mod.rs        # RawDecoder trait, dispatch function, RawPreview/RawMetadata structs
├── fuji.rs       # FujiRafDecoder — pure Rust RAF parser
└── libraw.rs     # LibRawDecoder — rsraw wrapper
```

## Cargo Changes

```toml
[dependencies]
rsraw = "0.1"  # LibRaw bindings for general RAW format support
```

**Build spike required:** Before implementation, verify `rsraw` compiles on macOS ARM64 in the existing Tauri build. If it fails, fallback to `libraw-sys` or direct FFI.

No new frontend dependencies.

## Database Changes

- Add `raw_metadata` TEXT column to `images` table (nullable JSON)
- Migration: `ALTER TABLE images ADD COLUMN raw_metadata TEXT`
- Update `Image` struct in `models.rs` to include `raw_metadata: Option<String>`
- Update all queries that SELECT from `images` to include the new column

## Settings UI

New "Modules" section in Settings panel:
- Toggle: "RAW File Support"
- Description: "Enable import and preview of RAW camera files (RAF, CR2, NEF, ARW, DNG, etc.)"
- When toggled on: toast offering to rescan library roots
- Status: "X RAW files in library" or "Module disabled"

## Testing

- Unit test: RAF header parsing with a real GFX RAF file (or a crafted minimal RAF with valid header + embedded JPEG)
- Unit test: `generate_thumbnail_from_image()` produces valid JPEG at all 4 sizes
- Unit test: `extensions::supported_extensions()` returns correct sets for both module states
- Integration test: import a RAW file with module enabled → thumbnail exists, dimensions > 0, raw_metadata populated
- Integration test: import with module disabled → file skipped, no DB entry
- Integration test: backfill — rescan library roots after enabling module → RAW files imported with thumbnails
- Integration test: backfill — legacy RAW files with width=0 (from before module existed) → thumbnails generated
- Integration test: Loupe displays preview for RAW files (not broken image)
- Edge cases: corrupt RAF header (graceful fallback to LibRaw), zero-byte file, non-RAW file with .raf extension
- Build spike: `cargo check` with rsraw on Apple Silicon

## Known Limitations (v1)

- Preview JPEG is the camera's rendering, not raw sensor data — exposure/WB adjustments not possible
- Preview resolution depends on camera model (~13MP on modern Fuji, could be lower on older models)
- rsraw/LibRaw Windows MSVC support is currently broken — macOS-only for now
- Crop and rotate are disabled for RAW files (requires full sensor decode)
- ML pipeline results (embeddings, detection) are based on preview JPEG, not raw sensor data
- Full-resolution Loupe view is limited to 800px preview (the largest thumbnail size)

## Future Upgrades

- Full sensor decode via LibRaw `unpack()` + `process()` — enables custom rendering and full-res Loupe
- Cargo feature flag for LibRaw to reduce binary size when not needed
- Additional pure-Rust parsers for other common formats (DNG, CR3)
- Larger preview extraction (some cameras embed multiple preview sizes — extract the largest)
- Crop/rotate support for RAW via decode → edit → save-as-TIFF workflow
- Publish/export connectors as additional modules following the same pattern
