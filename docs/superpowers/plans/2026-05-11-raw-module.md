# RAW File Support Module — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add RAW camera file support (RAF, CR2, NEF, ARW, DNG, etc.) as Cull's first toggleable module, using embedded JPEG preview extraction for thumbnails and viewing.

**Architecture:** Pure Rust RAF parser for Fuji files (fast path, zero deps), rsraw/LibRaw for other RAW formats (fallback). Extensions centralized in a single module. Module state shared via `Arc<AtomicBool>` for watcher thread. All existing `image::open()` call sites get RAW-aware fallbacks.

**Tech Stack:** Rust (rsraw, image crate), Svelte 5, Tauri 2, rusqlite

**Spec:** `docs/superpowers/specs/2026-05-11-raw-module-design.md`

---

### Task 1: Build spike — verify rsraw compiles on Apple Silicon

**Files:**
- Modify: `src-tauri/Cargo.toml`

This task validates that the rsraw dependency actually builds before we write any code against it.

- [ ] **Step 1: Add rsraw to Cargo.toml**

In `src-tauri/Cargo.toml`, add after the `dirs = "5"` line:

```toml
rsraw = "0.1"
```

- [ ] **Step 2: Run cargo check**

Run: `cd src-tauri && cargo check 2>&1 | tail -30`

Expected: Compiles successfully. If it fails with bindgen/libclang errors, note the error and switch to the fallback plan (direct `libraw-sys` FFI or shelling out to `dcraw_emu -e`).

- [ ] **Step 3: Verify rsraw API shape**

The actual rsraw API (per docs.rs) is:
- `let mut raw = RawImage::open(&data)` — note `mut` required
- `raw.extract_thumbs()` returns `Vec<ThumbnailImage>`
- `ThumbnailImage` has public fields: `width: u32`, `height: u32`, `data: Vec<u8>`, `format: ThumbnailFormat`
- NOT method calls like `.width()` or `.as_jpeg()`

Write a minimal compile-check test:
```rust
#[test]
fn rsraw_api_compiles() {
    // Just verify the types exist — no actual file needed
    let _: fn(&[u8]) -> rsraw::Result<rsraw::RawImage> = rsraw::RawImage::open;
}
```

Run: `cd src-tauri && cargo test rsraw_api_compiles --no-run 2>&1 | tail -10`

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: add rsraw dependency for RAW file support spike"
```

---

### Task 2: Create extensions.rs — single source of truth

**Files:**
- Create: `src-tauri/src/extensions.rs`
- Modify: `src-tauri/src/lib.rs:1` (add `mod extensions;`)
- Test: inline `#[cfg(test)]` in `extensions.rs`

This centralizes all extension lists. Currently scattered across 5 files.

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/extensions.rs` with tests only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn supported_without_raw_excludes_raf() {
        let exts = supported_extensions(false);
        assert!(exts.contains(&"jpg"));
        assert!(exts.contains(&"png"));
        assert!(exts.contains(&"psd"));
        assert!(!exts.contains(&"raf"));
        assert!(!exts.contains(&"cr2"));
    }

    #[test]
    fn supported_with_raw_includes_raf() {
        let exts = supported_extensions(true);
        assert!(exts.contains(&"jpg"));
        assert!(exts.contains(&"raf"));
        assert!(exts.contains(&"cr2"));
        assert!(exts.contains(&"nef"));
        assert!(exts.contains(&"arw"));
        assert!(exts.contains(&"dng"));
    }

    #[test]
    fn is_raw_extension_checks() {
        assert!(is_raw_extension("raf"));
        assert!(is_raw_extension("RAF"));
        assert!(is_raw_extension("cr2"));
        assert!(!is_raw_extension("jpg"));
        assert!(!is_raw_extension("psd"));
        assert!(!is_raw_extension(""));
    }

    #[test]
    fn is_image_path_respects_module() {
        assert!(is_image_path(Path::new("photo.jpg"), false));
        assert!(!is_image_path(Path::new("photo.raf"), false));
        assert!(is_image_path(Path::new("photo.raf"), true));
        assert!(is_image_path(Path::new("photo.RAF"), true));
        assert!(!is_image_path(Path::new("doc.txt"), true));
    }

    #[test]
    fn is_decodable_raw_only_when_enabled() {
        assert!(is_decodable("jpg", false));
        assert!(!is_decodable("raf", false));
        assert!(is_decodable("raf", true));
        assert!(is_decodable("cr2", true));
        assert!(!is_decodable("bmp", false)); // bmp is supported but not decodable by image crate
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test extensions::tests -- 2>&1 | tail -10`
Expected: Compilation errors — functions don't exist yet.

- [ ] **Step 3: Implement extensions.rs**

Add the implementation above the tests in `src-tauri/src/extensions.rs`:

```rust
use std::path::Path;

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
    if module_raw {
        exts.extend_from_slice(RAW_EXTENSIONS);
    }
    exts
}

pub fn is_raw_extension(ext: &str) -> bool {
    RAW_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

pub fn is_image_path(path: &Path, module_raw: bool) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_lowercase();
            BASE_IMAGE_EXTENSIONS.contains(&lower.as_str())
                || (module_raw && RAW_EXTENSIONS.contains(&lower.as_str()))
        })
        .unwrap_or(false)
}

pub fn is_decodable(ext: &str, module_raw: bool) -> bool {
    let lower = ext.to_lowercase();
    BASE_DECODABLE_EXTENSIONS.contains(&lower.as_str())
        || (module_raw && RAW_EXTENSIONS.contains(&lower.as_str()))
}
```

- [ ] **Step 4: Register the module in lib.rs**

In `src-tauri/src/lib.rs`, after `mod cloud;` (line 10), add:

```rust
pub mod extensions;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test extensions::tests -- 2>&1`
Expected: All 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/extensions.rs src-tauri/src/lib.rs
git commit -m "feat: centralized extension module with RAW support toggle"
```

---

### Task 3: Refactor existing code to use extensions.rs

**Files:**
- Modify: `src-tauri/src/db_core/import.rs:13-20` (remove SUPPORTED_EXTENSIONS and DECODABLE_EXTENSIONS)
- Modify: `src-tauri/src/lib.rs:79-90` (remove IMAGE_EXTENSIONS and is_image_path)
- Modify: `src-tauri/src/watcher.rs:23-59` (remove IMAGE_EXTENSIONS and is_image_ext)
- Modify: `src-tauri/src/commands/import.rs:31` (remove hardcoded extensions)

All 5 call sites switch to `crate::extensions::*`. No behavior change when `module_raw = false` — exact same extension set as before.

- [ ] **Step 1: Refactor import.rs**

In `src-tauri/src/db_core/import.rs`, remove lines 13-20 (the two const arrays) and replace `SUPPORTED_EXTENSIONS.contains(...)` with `crate::extensions::supported_extensions(false).contains(...)` and `DECODABLE_EXTENSIONS.contains(...)` with `crate::extensions::is_decodable(&ext, false)`.

The `false` is a temporary hardcode — Task 7 will wire it to the actual setting.

Replace lines 13-20:
```rust
// Extension checks now use crate::extensions module
```

In `sync_file()`, replace:
```rust
if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
```
with:
```rust
if !crate::extensions::supported_extensions(false).contains(&ext.as_str()) {
```

Replace:
```rust
let can_decode = DECODABLE_EXTENSIONS.contains(&ext.as_str());
```
with:
```rust
let can_decode = crate::extensions::is_decodable(&ext, false);
```

- [ ] **Step 2: Refactor lib.rs**

In `src-tauri/src/lib.rs`, remove the `IMAGE_EXTENSIONS` const (lines 79-83) and the `is_image_path` function (lines 85-90). Replace all call sites in `lib.rs` with `crate::extensions::is_image_path(path, false)`.

Search for `is_image_path(` in lib.rs and replace each call. There are ~4 occurrences in the drag-and-drop handler.

- [ ] **Step 3: Refactor watcher.rs**

In `src-tauri/src/watcher.rs`, remove lines 23-27 (the `IMAGE_EXTENSIONS` const) and the `is_image_ext` function (lines 55-60). Replace all `is_image_ext(path)` calls with `crate::extensions::is_image_path(path, false)`.

There are ~6 occurrences in `handle_event()`.

- [ ] **Step 4: Refactor commands/import.rs**

In `src-tauri/src/commands/import.rs`, replace the hardcoded extensions on line 31:
```rust
let extensions = ["jpg", "jpeg", "png", "webp", "gif"];
```
with:
```rust
let extensions = crate::extensions::supported_extensions(false);
```

And update the filter closure to use `extensions.contains(&ext.to_lowercase().as_str())`.

- [ ] **Step 5: Run full test suite**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`
Expected: All existing tests still pass. The watcher tests (`test_is_image_ext_*`) will fail since we removed `is_image_ext` — update those tests to use `crate::extensions::is_image_path()` instead.

- [ ] **Step 6: Fix watcher tests**

In `src-tauri/src/watcher.rs`, update the two test functions `test_is_image_ext_recognizes_common_formats` and `test_is_image_ext_rejects_non_images` to use `crate::extensions::is_image_path`:

```rust
#[test]
fn test_is_image_path_recognizes_common_formats() {
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.jpg"), false));
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.JPEG"), false));
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.png"), false));
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.webp"), false));
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.heic"), false));
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.psd"), false));
}

#[test]
fn test_is_image_path_rejects_non_images() {
    assert!(!crate::extensions::is_image_path(std::path::Path::new("doc.txt"), false));
    assert!(!crate::extensions::is_image_path(std::path::Path::new("data.json"), false));
    assert!(!crate::extensions::is_image_path(std::path::Path::new("script.rs"), false));
    assert!(!crate::extensions::is_image_path(std::path::Path::new("noext"), false));
}

#[test]
fn test_raw_extensions_visible_when_enabled() {
    assert!(!crate::extensions::is_image_path(std::path::Path::new("photo.raf"), false));
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.raf"), true));
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.cr2"), true));
    assert!(crate::extensions::is_image_path(std::path::Path::new("photo.dng"), true));
}
```

- [ ] **Step 7: Run tests again**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/db_core/import.rs src-tauri/src/lib.rs src-tauri/src/watcher.rs src-tauri/src/commands/import.rs
git commit -m "refactor: centralize extension checks via extensions module"
```

---

### Task 4: FujiRafDecoder — pure Rust RAF parser

**Files:**
- Create: `src-tauri/src/raw/mod.rs`
- Create: `src-tauri/src/raw/fuji.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod raw;`)
- Test: inline `#[cfg(test)]` in `fuji.rs`

- [ ] **Step 1: Write failing tests for RAF parser**

Create `src-tauri/src/raw/fuji.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_raf_magic_validation() {
        let bad_data = b"NOT A RAF FILE AT ALL!!";
        assert!(parse_raf_header(bad_data).is_err());
    }

    #[test]
    fn test_raf_magic_too_short() {
        let short = b"FUJI";
        assert!(parse_raf_header(short).is_err());
    }

    #[test]
    fn test_raf_magic_accepted() {
        // Valid magic but truncated after — should fail on offset read, not magic
        let mut data = b"FUJIFILMCCD-RAW 0201".to_vec();
        data.resize(120, 0); // pad to minimum header size
        let result = parse_raf_header(&data);
        // Should fail because jpeg_offset points to 0 (no JPEG), not because of magic
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.contains("magic"), "Should pass magic check: {}", err);
    }

    #[test]
    fn test_extract_jpeg_validates_soi_eoi() {
        // Construct minimal "RAF" with a fake JPEG blob
        let jpeg_blob = build_minimal_jpeg();
        let raf_data = build_test_raf(&jpeg_blob);
        let header = parse_raf_header(&raf_data).unwrap();
        let jpeg = extract_embedded_jpeg(&raf_data, &header).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8])); // SOI marker
    }

    fn build_minimal_jpeg() -> Vec<u8> {
        // SOI + APP0 (minimal) + EOI — must have both markers for validation
        let mut jpeg = vec![0xFF, 0xD8]; // SOI
        jpeg.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x02, 0x00, 0x00]); // APP0
        jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI
        jpeg
    }

    fn build_test_raf(jpeg_blob: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        // Magic (16 bytes)
        buf.extend_from_slice(b"FUJIFILMCCD-RAW ");
        // Format version (4 bytes)
        buf.extend_from_slice(b"0201");
        // Camera ID (8 bytes)
        buf.extend_from_slice(b"GFX100S\0");
        // Camera model string (32 bytes)
        let mut model = b"GFX 100S".to_vec();
        model.resize(32, 0);
        buf.extend_from_slice(&model);
        // Directory version (4 bytes)
        buf.extend_from_slice(b"0100");
        // Padding to offset 84 (20 bytes to fill)
        buf.resize(84, 0);
        // JPEG offset (4 bytes, Big Endian) — points to after header
        let jpeg_offset: u32 = 100; // offset where JPEG data starts
        buf.extend_from_slice(&jpeg_offset.to_be_bytes());
        // JPEG length (4 bytes, Big Endian)
        let jpeg_len = jpeg_blob.len() as u32;
        buf.extend_from_slice(&jpeg_len.to_be_bytes());
        // CFA offset (4 bytes) — not used for preview
        buf.extend_from_slice(&0u32.to_be_bytes());
        // CFA length (4 bytes)
        buf.extend_from_slice(&0u32.to_be_bytes());
        // Pad to jpeg_offset
        buf.resize(jpeg_offset as usize, 0);
        // JPEG blob
        buf.extend_from_slice(jpeg_blob);
        buf
    }
}
```

- [ ] **Step 2: Write the trait definitions in mod.rs**

Create `src-tauri/src/raw/mod.rs`:

```rust
pub mod fuji;

use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub struct RawPreview {
    pub image: image::DynamicImage,
    pub metadata: RawMetadata,
}

pub trait RawDecoder: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn extract_preview(&self, path: &Path) -> Result<RawPreview, String>;
}

pub fn decode_raw_preview(path: &Path) -> Result<RawPreview, String> {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ext.eq_ignore_ascii_case("raf") {
        match fuji::FujiRafDecoder.extract_preview(path) {
            Ok(preview) => return Ok(preview),
            Err(e) => {
                eprintln!("[raw] Fuji RAF parser failed, trying LibRaw: {}", e);
            }
        }
    }

    // TODO: Task 5 adds LibRaw fallback here
    Err(format!("No RAW decoder available for .{}", ext))
}
```

- [ ] **Step 3: Register module in lib.rs**

In `src-tauri/src/lib.rs`, after `pub mod extensions;`, add:

```rust
pub mod raw;
```

- [ ] **Step 4: Implement the RAF parser**

In `src-tauri/src/raw/fuji.rs`, add above the tests:

```rust
use std::path::Path;
use super::{RawDecoder, RawPreview, RawMetadata};

pub struct FujiRafDecoder;

const RAF_MAGIC: &[u8; 16] = b"FUJIFILMCCD-RAW ";
const JPEG_OFFSET_POS: usize = 84;
const JPEG_LENGTH_POS: usize = 88;
const MIN_HEADER_SIZE: usize = 100;

struct RafHeader {
    jpeg_offset: u32,
    jpeg_length: u32,
    camera_model: String,
}

fn parse_raf_header(data: &[u8]) -> Result<RafHeader, String> {
    if data.len() < MIN_HEADER_SIZE {
        return Err(format!("File too small for RAF: {} bytes", data.len()));
    }

    if &data[0..16] != RAF_MAGIC {
        return Err("Invalid RAF magic bytes".to_string());
    }

    let jpeg_offset = u32::from_be_bytes(
        data[JPEG_OFFSET_POS..JPEG_OFFSET_POS + 4]
            .try_into()
            .map_err(|_| "Failed to read JPEG offset")?
    );
    let jpeg_length = u32::from_be_bytes(
        data[JPEG_LENGTH_POS..JPEG_LENGTH_POS + 4]
            .try_into()
            .map_err(|_| "Failed to read JPEG length")?
    );

    // Camera model is at offset 28, 32 bytes, null-terminated
    let model_bytes = &data[28..60];
    let camera_model = std::str::from_utf8(model_bytes)
        .unwrap_or("")
        .trim_end_matches('\0')
        .trim()
        .to_string();

    Ok(RafHeader {
        jpeg_offset,
        jpeg_length,
        camera_model,
    })
}

fn extract_embedded_jpeg<'a>(data: &'a [u8], header: &RafHeader) -> Result<&'a [u8], String> {
    let start = header.jpeg_offset as usize;
    let end = (start as u64)
        .checked_add(header.jpeg_length as u64)
        .ok_or_else(|| "JPEG offset + length overflow".to_string())? as usize;

    if end > data.len() {
        return Err(format!(
            "JPEG range {}..{} exceeds file size {}",
            start, end, data.len()
        ));
    }

    let jpeg = &data[start..end];

    // Validate SOI marker
    if jpeg.len() < 2 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return Err("Embedded data does not start with JPEG SOI marker".to_string());
    }

    // Validate EOI marker
    if jpeg.len() < 2 || jpeg[jpeg.len() - 2] != 0xFF || jpeg[jpeg.len() - 1] != 0xD9 {
        return Err("Embedded JPEG missing EOI marker".to_string());
    }

    Ok(jpeg)
}

impl RawDecoder for FujiRafDecoder {
    fn extensions(&self) -> &[&str] {
        &["raf"]
    }

    fn extract_preview(&self, path: &Path) -> Result<RawPreview, String> {
        let data = std::fs::read(path)
            .map_err(|e| format!("Failed to read RAF file: {}", e))?;

        let header = parse_raf_header(&data)?;
        let jpeg_data = extract_embedded_jpeg(&data, &header)?;

        let image = image::load_from_memory_with_format(jpeg_data, image::ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to decode embedded JPEG: {}", e))?;

        let metadata = RawMetadata {
            camera_model: if header.camera_model.is_empty() { None } else { Some(header.camera_model) },
            lens: None,
            shutter_speed: None,
            aperture: None,
            iso: None,
            focal_length: None,
            date_taken: None,
            film_simulation: None,
            gps: None,
            sensor_width: 0,
            sensor_height: 0,
        };

        Ok(RawPreview { image, metadata })
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test raw::fuji::tests -- 2>&1`
Expected: All 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/raw/mod.rs src-tauri/src/raw/fuji.rs src-tauri/src/lib.rs
git commit -m "feat: pure Rust RAF parser with embedded JPEG extraction"
```

---

### Task 5: LibRawDecoder — rsraw wrapper

**Files:**
- Create: `src-tauri/src/raw/libraw.rs`
- Modify: `src-tauri/src/raw/mod.rs` (add `pub mod libraw;`, wire into dispatch)

**Depends on:** Task 1 (rsraw spike must pass). If rsraw doesn't compile, skip this task and adjust `decode_raw_preview()` to return an error for non-RAF formats.

- [ ] **Step 1: Write failing test**

Create `src-tauri/src/raw/libraw.rs` with a test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_libraw_decoder_extensions() {
        let decoder = LibRawDecoder;
        let exts = decoder.extensions();
        assert!(exts.contains(&"cr2"));
        assert!(exts.contains(&"nef"));
        assert!(exts.contains(&"arw"));
        assert!(exts.contains(&"dng"));
        assert!(!exts.contains(&"raf")); // Fuji handled by FujiRafDecoder
    }
}
```

- [ ] **Step 2: Implement LibRawDecoder**

In `src-tauri/src/raw/libraw.rs`, above the tests:

```rust
use std::path::Path;
use super::{RawDecoder, RawPreview, RawMetadata};

pub struct LibRawDecoder;

impl RawDecoder for LibRawDecoder {
    fn extensions(&self) -> &[&str] {
        &["cr2", "cr3", "nef", "arw", "dng", "orf", "rw2"]
    }

    fn extract_preview(&self, path: &Path) -> Result<RawPreview, String> {
        let data = std::fs::read(path)
            .map_err(|e| format!("Failed to read RAW file: {}", e))?;

        let mut raw_image = rsraw::RawImage::open(&data)
            .map_err(|e| format!("LibRaw failed to open: {}", e))?;

        let thumbs = raw_image.extract_thumbs()
            .map_err(|e| format!("LibRaw failed to extract thumbnails: {}", e))?;

        // ThumbnailImage has public fields: width, height, data, format
        let thumb = thumbs.into_iter()
            .max_by_key(|t| t.width as u64 * t.height as u64)
            .ok_or_else(|| "No thumbnails found in RAW file".to_string())?;

        // Check format is JPEG before decoding
        let image = image::load_from_memory(&thumb.data)
            .map_err(|e| format!("Failed to decode extracted thumbnail: {}", e))?;

        let metadata = RawMetadata {
            camera_model: None,
            lens: None,
            shutter_speed: None,
            aperture: None,
            iso: None,
            focal_length: None,
            date_taken: None,
            film_simulation: None,
            gps: None,
            sensor_width: 0,
            sensor_height: 0,
        };

        Ok(RawPreview { image, metadata })
    }
}
```

- [ ] **Step 3: Wire LibRaw into dispatch**

In `src-tauri/src/raw/mod.rs`, add:

```rust
pub mod libraw;
```

And update `decode_raw_preview()` — replace the TODO comment with:

```rust
    libraw::LibRawDecoder.extract_preview(path)
```

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test raw:: -- 2>&1`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/raw/libraw.rs src-tauri/src/raw/mod.rs
git commit -m "feat: LibRaw decoder via rsraw for non-Fuji RAW formats"
```

---

### Task 6: Database migration — add raw_metadata column

**Files:**
- Modify: `src-tauri/src/db_core/db.rs` (add migration function, add to `run_migrations`)
- Modify: `src-tauri/src/db_core/models.rs` (add field to `Image`)

- [ ] **Step 1: Add migration function**

In `src-tauri/src/db_core/db.rs`, after `migrate_image_file_stat_columns` (around line 60), add:

```rust
    fn migrate_raw_metadata(&self) -> Result<()> {
        let conn = self.conn.lock();
        let sql = "ALTER TABLE images ADD COLUMN raw_metadata TEXT";
        match conn.execute(sql, []) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }
```

- [ ] **Step 2: Register migration**

In `run_migrations()` (around line 34), add after `self.migrate_image_file_stat_columns()?;`:

```rust
        self.migrate_raw_metadata()?;
```

- [ ] **Step 3: Add field to Image model**

In `src-tauri/src/db_core/models.rs`, add to the `Image` struct after `ai_prompt`:

```rust
    pub raw_metadata: Option<String>,
```

- [ ] **Step 4: Update all Image-reading queries**

Every query that constructs an `Image` struct needs the new column. Search for `ai_prompt: row.get(` in `db.rs` to find all locations. At each one:

1. Add `i.raw_metadata` to the SELECT column list
2. Add `raw_metadata: row.get(N)?` to the struct constructor (where N is the next column index)

The affected functions are approximately: `find_by_hash`, `list_images`, `list_images_by_folder`, `list_images_filtered`, `get_images_by_ids`, `list_collection_images`, and any other function returning `Image` or `ImageWithFile`.

**Also check `db_core/lineage.rs`** (~line 210) which constructs `Image` structs in lineage queries. And search the entire `src-tauri/src` directory for `Image {` constructors:

Run: `grep -rn "ai_prompt:" src-tauri/src/ --include="*.rs"` to find all locations that need the new `raw_metadata` field.

- [ ] **Step 5: Add store/update method**

In `src-tauri/src/db_core/db.rs`, add:

```rust
    pub fn update_raw_metadata(&self, image_id: &str, metadata_json: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET raw_metadata = ?1 WHERE id = ?2",
            params![metadata_json, image_id],
        )?;
        Ok(())
    }
```

- [ ] **Step 6: Run tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`
Expected: All tests pass. Compilation may need fixing if any `Image { ... }` constructors in tests are missing the new field — add `raw_metadata: None` to each.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db_core/db.rs src-tauri/src/db_core/models.rs
git commit -m "feat: add raw_metadata column to images table"
```

---

### Task 7: Wire module toggle + import pipeline

**Files:**
- Modify: `src-tauri/src/db_core/import.rs` (add RAW decode path)
- Modify: `src-tauri/src/db_core/thumbnails.rs` (add `generate_thumbnail_from_image`)
- Modify: `src-tauri/src/commands/import.rs` (use module setting for extensions)
- Modify: `src-tauri/src/watcher.rs` (use `Arc<AtomicBool>` for module_raw)
- Modify: `src-tauri/src/lib.rs` (read module_raw setting at startup, pass to watcher)

- [ ] **Step 1: Add generate_thumbnail_from_image**

In `src-tauri/src/db_core/thumbnails.rs`, add after `generate_thumbnail`:

```rust
pub fn generate_thumbnail_from_image(
    img: &image::DynamicImage,
    app_data_dir: &Path,
    image_id: &str,
) -> Result<PathBuf, String> {
    let thumb_dir = thumbnail_dir(app_data_dir);

    let mut current = img.clone();
    let src_max = current.width().max(current.height());
    let last_path = thumb_dir.join(format!("{}.jpg", image_id));

    for &size in THUMBNAIL_SIZES.iter().rev() {
        if size >= src_max {
            if size == 800 {
                save_jpeg(&current, &last_path)?;
            } else {
                let sized_path = thumb_dir.join(format!("{}_{}.jpg", image_id, size));
                save_jpeg(&current, &sized_path)?;
            }
            continue;
        }
        let resized = current.resize(size, size, FilterType::Lanczos3);
        if size == 800 {
            save_jpeg(&resized, &last_path)?;
        } else {
            let sized_path = thumb_dir.join(format!("{}_{}.jpg", image_id, size));
            save_jpeg(&resized, &sized_path)?;
        }
        current = resized;
    }

    Ok(last_path)
}
```

- [ ] **Step 2: Update import.rs to decode RAW files**

In `src-tauri/src/db_core/import.rs`, update `create_image_record` to handle RAW:

Replace the current dimension-reading logic:
```rust
    let (width, height) = if can_decode {
        let img = image::open(file_path).map_err(|e| format!("Decode error: {}", e))?;
        (img.width(), img.height())
    } else {
        (0, 0)
    };
```

With:
```rust
    let (width, height, raw_preview) = if can_decode {
        if crate::extensions::is_raw_extension(ext) {
            // RAW decode failure is an error — don't import broken records
            let preview = crate::raw::decode_raw_preview(file_path)
                .map_err(|e| format!("RAW decode failed: {}", e))?;
            let w = preview.image.width();
            let h = preview.image.height();
            (w, h, Some(preview))
        } else {
            let img = image::open(file_path).map_err(|e| format!("Decode error: {}", e))?;
            (img.width(), img.height(), None)
        }
    } else {
        (0, 0, None)
    };
```

The function signature also needs to return `Option<crate::raw::RawPreview>` or the preview needs to be handled in `sync_file`. The cleanest approach: have `create_image_record` return `(String, Option<RawPreview>)` so the caller can generate the thumbnail.

Update `create_image_record` return type to `Result<(String, Option<crate::raw::RawPreview>), String>` and return `Ok((image_id, raw_preview))`.

Then update **all three caller sites** in `sync_file` (new-path import ~line 117, content-changed repoint ~line 89, and hash-match repoint ~line 83 which calls `generate_thumbnail` directly) — after calling `create_image_record`, check if a `RawPreview` was returned and use it for thumbnailing:

```rust
let (image_id, raw_preview) = create_image_record(db, file_path, &hash, &ext, &data, can_decode)?;
// ...
if can_decode {
    if let Some(preview) = &raw_preview {
        let _ = thumbnails::generate_thumbnail_from_image(&preview.image, app_data_dir, &image_id);
        // Store raw metadata
        if let Ok(meta_json) = serde_json::to_string(&preview.metadata) {
            let _ = db.update_raw_metadata(&image_id, &meta_json);
        }
    } else {
        let _ = thumbnails::generate_thumbnail(file_path, app_data_dir, &image_id);
    }
    run_source_detection(db, file_path, &image_id, &ext);
    run_sidecar_detection(db, file_path, &image_id);
}
```

- [ ] **Step 3: Wire module_raw setting into extension checks**

Replace all `false` hardcodes from Task 3 with actual setting reads. Add a helper in `import.rs`:

```rust
fn is_module_raw_enabled(db: &Database) -> bool {
    db.get_setting("module_raw")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false)
}
```

Then replace `supported_extensions(false)` with `supported_extensions(is_module_raw_enabled(db))` and `is_decodable(&ext, false)` with `is_decodable(&ext, is_module_raw_enabled(db))`.

In `commands/import.rs`, read the setting from state:
```rust
let module_raw = state.db.get_setting("module_raw")
    .ok().flatten().map(|v| v == "true").unwrap_or(false);
let extensions = crate::extensions::supported_extensions(module_raw);
```

- [ ] **Step 4: Wire AtomicBool into watcher**

In `src-tauri/src/watcher.rs`, add to the `FileWatcher` struct:

```rust
pub module_raw: Arc<std::sync::atomic::AtomicBool>,
```

Initialize in `new()`:
```rust
module_raw: Arc::new(std::sync::atomic::AtomicBool::new(false)),
```

In `start()`, capture it for the closure:
```rust
let module_raw = self.module_raw.clone();
```

Pass to `handle_event`:
```rust
let module_raw_val = module_raw.load(std::sync::atomic::Ordering::Relaxed);
```

And replace all `crate::extensions::is_image_path(path, false)` with `crate::extensions::is_image_path(path, module_raw_val)`.

In `src-tauri/src/lib.rs`, in the `setup` closure, after creating the watcher, set the initial value:
```rust
let module_raw = state.db.get_setting("module_raw")
    .ok().flatten().map(|v| v == "true").unwrap_or(false);
state.file_watcher.lock().module_raw.store(module_raw, std::sync::atomic::Ordering::Relaxed);
```

Also update `set_app_setting` in `commands/library.rs` to sync the watcher when `module_raw` changes:

```rust
// After writing the setting to DB, check if it's module_raw
if key == "module_raw" {
    let enabled = value == "true";
    state.file_watcher.lock().module_raw.store(enabled, std::sync::atomic::Ordering::Relaxed);
}
```

- [ ] **Step 5: Run full test suite**

Run: `cd src-tauri && cargo test 2>&1 | tail -30`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db_core/import.rs src-tauri/src/db_core/thumbnails.rs src-tauri/src/commands/import.rs src-tauri/src/watcher.rs src-tauri/src/lib.rs
git commit -m "feat: wire RAW module toggle into import pipeline and watcher"
```

---

### Task 8: Frontend — Loupe/Compare/Tinder RAW preview serving

**Files:**
- Modify: `src/lib/components/Loupe.svelte:18` (RAW-aware src)
- Modify: `src/lib/components/Compare.svelte` (same pattern)
- Modify: `src/lib/components/Tinder.svelte` (same pattern)
- Modify: `src/lib/api.ts` (add `raw_metadata` to `Image` interface)

- [ ] **Step 1: Update Image interface in api.ts**

In `src/lib/api.ts`, add to the `Image` interface after `ai_prompt`:

```typescript
    raw_metadata: string | null;
```

- [ ] **Step 2: Add RAW extension check helper**

In `src/lib/api.ts`, add:

```typescript
const RAW_EXTENSIONS = ['cr2', 'cr3', 'nef', 'arw', 'dng', 'orf', 'raf', 'rw2'];

export function isRawFormat(format: string): boolean {
    return RAW_EXTENSIONS.includes(format.toLowerCase());
}
```

- [ ] **Step 3: Update Loupe.svelte**

In `src/lib/components/Loupe.svelte`, line 18, change:

```typescript
let src = $derived(image ? convertFileSrc(image.path) : '');
```

To:

```typescript
let src = $derived(image ? (isRawFormat(image.image.format) ? convertFileSrc(image.thumbnail_path ?? image.path) : convertFileSrc(image.path)) : '');
```

Add the import at the top:
```typescript
import { isRawFormat } from '$lib/api';
```

- [ ] **Step 4: Update Compare.svelte and Tinder.svelte**

Apply the same pattern: wherever `convertFileSrc(image.path)` is used for display, wrap it with the RAW check to use `thumbnail_path` instead. Search for `convertFileSrc` in each file.

- [ ] **Step 5: Disable crop/rotate for RAW**

In `Loupe.svelte`, find the crop and rotate UI elements and add a condition to disable them when `isRawFormat(image.image.format)`. The exact implementation depends on how they're triggered (buttons, context menu, etc.).

- [ ] **Step 6: Build frontend**

Run: `cd /Users/glebkalinin/ai_projects/claude-code-lab/20260502-obsidian/cull && npx vite build 2>&1 | tail -10`
Expected: Builds successfully.

- [ ] **Step 7: Commit**

```bash
git add src/lib/api.ts src/lib/components/Loupe.svelte src/lib/components/Compare.svelte src/lib/components/Tinder.svelte
git commit -m "feat: serve RAW preview thumbnails in Loupe/Compare/Tinder"
```

---

### Task 9: Settings UI — Module toggle

**Files:**
- Modify: `src/lib/components/McpSettings.svelte` (the Settings panel, rendered at `+page.svelte:285`)

- [ ] **Step 1: Add module state and toggle function**

In `McpSettings.svelte`, add to the script section (using Svelte 5 runes):

```typescript
import { getAppSetting, setAppSetting, backfillRawPreviews } from '$lib/api';
import { showToast } from '$lib/stores';

let moduleRaw = $state(false);

onMount(async () => {
    const val = await getAppSetting('module_raw');
    moduleRaw = val === 'true';
});

async function toggleModuleRaw() {
    await setAppSetting('module_raw', moduleRaw ? 'true' : 'false');
    if (moduleRaw) {
        showToast('RAW support enabled.', {
            type: 'success',
            duration: 10000,
            actions: [{ label: 'Rescan library', onclick: () => backfillRawPreviews() }],
        });
    }
}
```

- [ ] **Step 2: Add the Modules section to the template**

In the template, add a new section (following the existing `.section` / `.section-header` / `.section-item` CSS pattern):

```svelte
<div class="section">
    <div class="section-header">Modules</div>
    <div class="section-item">
        <label style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
            <input type="checkbox" bind:checked={moduleRaw} onchange={toggleModuleRaw} />
            RAW File Support
        </label>
        <span class="text-secondary" style="font-size: 0.85em; margin-top: 4px;">
            Import and preview RAW camera files (RAF, CR2, NEF, ARW, DNG, etc.)
        </span>
    </div>
</div>
```

- [ ] **Step 3: Build and test**

Run: `cd /Users/glebkalinin/ai_projects/claude-code-lab/20260502-obsidian/cull && npx vite build 2>&1 | tail -10`

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/McpSettings.svelte
git commit -m "feat: module toggle for RAW file support in Settings"
```

---

### Task 10: Backfill command + thumbnail regeneration RAW support

**Files:**
- Create: `src-tauri/src/commands/raw.rs` (new command file)
- Modify: `src-tauri/src/commands/mod.rs` (add `pub mod raw;`)
- Modify: `src-tauri/src/commands/import.rs:172` (RAW-aware thumbnail regeneration)
- Modify: `src-tauri/src/lib.rs` (register new command)

- [ ] **Step 1: Create backfill command**

Create `src-tauri/src/commands/raw.rs`:

```rust
use tauri::{AppHandle, Emitter, Manager, State};
use crate::AppState;

#[tauri::command]
pub async fn backfill_raw_previews(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;

    let raw_exts: Vec<String> = crate::extensions::RAW_EXTENSIONS.iter()
        .map(|e| format!("'{}'", e))
        .collect();
    let in_clause = raw_exts.join(",");

    let images: Vec<(String, String)> = {
        let conn = db.conn.lock();
        let query = format!(
            "SELECT i.id, f.path FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             WHERE i.format IN ({}) AND i.width = 0
             GROUP BY i.id",
            in_clause
        );
        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let total = images.len() as u32;
    let mut backfilled = 0u32;

    for (i, (image_id, path_str)) in images.iter().enumerate() {
        let path = std::path::Path::new(path_str);
        if !path.exists() { continue; }

        match crate::raw::decode_raw_preview(path) {
            Ok(preview) => {
                let w = preview.image.width();
                let h = preview.image.height();
                let _ = crate::db_core::thumbnails::generate_thumbnail_from_image(
                    &preview.image, app_data_dir, image_id,
                );
                // Update dimensions
                {
                    let conn = db.conn.lock();
                    let _ = conn.execute(
                        "UPDATE images SET width = ?1, height = ?2 WHERE id = ?3",
                        rusqlite::params![w, h, image_id],
                    );
                }
                // Store metadata
                if let Ok(meta_json) = serde_json::to_string(&preview.metadata) {
                    let _ = db.update_raw_metadata(image_id, &meta_json);
                }
                backfilled += 1;
            }
            Err(e) => {
                eprintln!("[backfill] Failed for {}: {}", path_str, e);
            }
        }

        let _ = app.emit("backfill-progress", serde_json::json!({
            "current": i + 1, "total": total
        }));
    }

    Ok(backfilled)
}
```

- [ ] **Step 2: Register module and command**

In `src-tauri/src/commands/mod.rs`, add: `pub mod raw;`

In `src-tauri/src/lib.rs`, add to the `invoke_handler` list:
```rust
commands::raw::backfill_raw_previews,
```

- [ ] **Step 3: Update thumbnail regeneration to handle RAW**

In `src-tauri/src/commands/import.rs`, update `regenerate_thumbnails` (line 172) and `regenerate_thumbnails_by_ids` (line 203) and `regenerate_single_thumbnail` (line 233). Before calling `generate_thumbnail`, check if the source is RAW:

```rust
let ext = source_path.extension()
    .and_then(|e| e.to_str())
    .unwrap_or("");
if crate::extensions::is_raw_extension(ext) {
    match crate::raw::decode_raw_preview(source_path) {
        Ok(preview) => {
            match crate::db_core::thumbnails::generate_thumbnail_from_image(
                &preview.image, app_data_dir, &img.image.id,
            ) {
                Ok(_) => regenerated += 1,
                Err(e) => eprintln!("RAW thumbnail failed for {}: {}", img.path, e),
            }
        }
        Err(e) => eprintln!("RAW decode failed for {}: {}", img.path, e),
    }
} else {
    match crate::db_core::thumbnails::generate_thumbnail(source_path, app_data_dir, &img.image.id) {
        Ok(_) => regenerated += 1,
        Err(e) => eprintln!("Thumbnail failed for {}: {}", img.path, e),
    }
}
```

- [ ] **Step 4: Update post-import detection for RAW**

In `src-tauri/src/commands/import.rs`, in `run_post_import_detection`, the YOLO and NudeNet detection code calls `engine.detect(Path::new(&img.path))`. For RAW files, this will fail because the engine calls `image::open()` internally.

Add a check: if the image format is RAW, use the thumbnail path instead:

```rust
let detect_path = if crate::extensions::is_raw_extension(
    std::path::Path::new(&img.path).extension().and_then(|e| e.to_str()).unwrap_or("")
) {
    crate::db_core::thumbnails::thumbnail_path(
        &app.state::<AppState>().app_data_dir, &image_id
    )
} else {
    std::path::PathBuf::from(&img.path)
};
```

Then use `&detect_path` instead of `Path::new(&img.path)` in the `engine.detect()` call.

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/raw.rs src-tauri/src/commands/mod.rs src-tauri/src/commands/import.rs src-tauri/src/lib.rs
git commit -m "feat: backfill command and RAW-aware thumbnail regeneration"
```

---

### Task 11: Source detection for RAW + MCP raw_metadata exposure

**Files:**
- Modify: `src-tauri/src/db_core/import.rs` (RAW source detection)
- Modify: `src-tauri/src/mcp/tools.rs` (expose raw_metadata if needed)

- [ ] **Step 1: Update source detection for RAW files**

In `src-tauri/src/db_core/import.rs`, `run_source_detection` calls `image::open()` for dimensions. For RAW files, use the already-known dimensions from the image record:

After the sync_file import path, when `run_source_detection` is called for RAW files, the width/height are already set from the preview. The `run_source_detection` function should be updated to accept optional dimensions:

```rust
fn run_source_detection(db: &Database, file_path: &Path, image_id: &str, ext: &str, override_dims: Option<(u32, u32)>) {
    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let png_chunks = if ext == "png" {
        read_png_text_chunks(file_path).unwrap_or_default()
    } else {
        vec![]
    };
    let detection = detect_source(filename, &png_chunks, file_path);

    let (width, height) = override_dims.unwrap_or_else(|| {
        image::open(file_path)
            .map(|i| (i.width(), i.height()))
            .unwrap_or((0, 0))
    });
    // ... rest is the same
```

For RAW files, also force `is_ai_generated: false` and set `source_label` to "camera":

After running `detect_source`, if the extension is RAW:
```rust
    if crate::extensions::is_raw_extension(ext) {
        let _ = db.update_source_detection(
            image_id, Some("camera"), 1.0,
            &serde_json::json!({"method": "raw_format"}).to_string(),
            false, None,
            aspect_ratio, orientation, megapixels,
        );
        return;
    }
```

- [ ] **Step 2: Run tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db_core/import.rs
git commit -m "feat: RAW files detected as camera source, dimension override for source detection"
```

---

### Task 12: Integration test with real RAF file

**Files:**
- Test file needed: a real `.raf` file from a Fuji camera (user must provide)
- Create: `src-tauri/tests/raw_integration.rs` (or inline test)

- [ ] **Step 1: Test with a real RAF file**

Ask the user for a sample GFX compressed RAF file. Place it in a test fixtures directory or use a path the user provides.

Run manual verification:

```bash
cd src-tauri && cargo test -- --ignored raw_integration 2>&1
```

Or write a quick test:

```rust
#[test]
#[ignore] // requires real RAF file
fn test_real_gfx_raf() {
    let path = std::path::Path::new("/path/to/sample.raf"); // user provides
    assert!(path.exists(), "Test RAF file not found");
    let preview = crate::raw::decode_raw_preview(path).unwrap();
    assert!(preview.image.width() > 0);
    assert!(preview.image.height() > 0);
    println!("Preview: {}x{}", preview.image.width(), preview.image.height());
    println!("Camera: {:?}", preview.metadata.camera_model);
}
```

- [ ] **Step 2: Verify end-to-end**

Start the dev app, enable RAW module in settings, import a folder with RAF files, verify:
1. Thumbnails appear in grid
2. Loupe shows the preview
3. Source label shows "camera"
4. Dimensions are non-zero

- [ ] **Step 3: Commit test**

```bash
git add src-tauri/tests/
git commit -m "test: integration test for RAF preview extraction"
```

---

### Task 13: ML pipelines — use thumbnails for RAW files

**Files:**
- Modify: `src-tauri/src/commands/detection.rs` (detect_objects, detect_nsfw)
- Modify: `src-tauri/src/commands/embeddings.rs` (generate_embeddings, generate_gemini_embeddings)

The manual detection/embedding commands (not just post-import) also call `image::open()` or pass `img.path` to engines. For RAW files, they must use the thumbnail instead.

- [ ] **Step 1: Create helper function**

Add to `src-tauri/src/commands/mod.rs` or a shared location:

```rust
pub fn resolve_image_path_for_ml(img: &crate::db_core::models::ImageWithFile, app_data_dir: &std::path::Path) -> std::path::PathBuf {
    let ext = std::path::Path::new(&img.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if crate::extensions::is_raw_extension(ext) {
        crate::db_core::thumbnails::thumbnail_path(app_data_dir, &img.image.id)
    } else {
        std::path::PathBuf::from(&img.path)
    }
}
```

- [ ] **Step 2: Update detection commands**

In `src-tauri/src/commands/detection.rs`, find `detect_objects` and `detect_nsfw` functions. Where they call `engine.detect(Path::new(&img.path))`, replace with:

```rust
let detect_path = crate::commands::resolve_image_path_for_ml(img, &state.app_data_dir);
engine.detect(&detect_path)
```

- [ ] **Step 3: Update embedding commands**

In `src-tauri/src/commands/embeddings.rs`, find `generate_embeddings` and `generate_gemini_embeddings`. Where they read image files or pass paths, use the same `resolve_image_path_for_ml` helper.

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/detection.rs src-tauri/src/commands/embeddings.rs src-tauri/src/commands/mod.rs
git commit -m "feat: ML pipelines use thumbnails for RAW files"
```

---

### Task 14: Export — RAW provenance

**Files:**
- Modify: `src-tauri/src/commands/export.rs`

- [ ] **Step 1: Update export manifest for RAW files**

In `src-tauri/src/commands/export.rs`, where export assets are created, check if the image format is RAW. If so:
- The `original` asset still points to the actual RAW file
- Add a `provenance: "raw_preview"` field to the manifest entry when serving the preview
- Default export behavior for RAW files should use the preview thumbnail

Find the asset creation code and add:

```rust
let ext = std::path::Path::new(&img.path)
    .extension()
    .and_then(|e| e.to_str())
    .unwrap_or("");
if crate::extensions::is_raw_extension(ext) {
    // Use preview thumbnail as the export source, mark provenance
    // The original RAW file stays accessible as "original" asset type
}
```

The exact implementation depends on the export manifest structure — follow the existing pattern in `export.rs`.

- [ ] **Step 2: Run tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/export.rs
git commit -m "feat: export provenance for RAW files"
```

---

### Task 15: Final cleanup and frontend api binding

**Files:**
- Modify: `src/lib/api.ts` (add backfill_raw_previews function)

- [ ] **Step 1: Add frontend API binding**

In `src/lib/api.ts`:

```typescript
export async function backfillRawPreviews(): Promise<number> {
    return invoke<number>('backfill_raw_previews');
}
```

- [ ] **Step 2: Wire backfill to Settings toggle**

In the Settings component, when the RAW toggle is turned on, add a button or auto-trigger:

```typescript
async function toggleModuleRaw() {
    await setAppSetting('module_raw', moduleRaw ? 'true' : 'false');
    if (moduleRaw) {
        showToast('RAW support enabled. Click "Rescan" to import RAW files from your library.', {
            type: 'success',
            actions: [{ label: 'Rescan', onclick: () => backfillRawPreviews() }],
        });
    }
}
```

- [ ] **Step 3: Build everything**

Run: `cd /Users/glebkalinin/ai_projects/claude-code-lab/20260502-obsidian/cull && cd src-tauri && cargo build 2>&1 | tail -10`
Run: `cd /Users/glebkalinin/ai_projects/claude-code-lab/20260502-obsidian/cull && npx vite build 2>&1 | tail -10`

- [ ] **Step 4: Final commit**

```bash
git add src/lib/api.ts src/lib/components/
git commit -m "feat: complete RAW module with settings toggle and backfill UI"
```
