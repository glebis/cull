# Safe Thumbnail Previews Through the Cull MCP — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only MCP tool `get_image_previews` that returns bounded, privacy-aware previews of already-generated thumbnails for explicit image IDs or a bounded folder page.

**Architecture:** One new `#[tool]` handler in `src-tauri/src/mcp/tools/library.rs` returns an `rmcp` `CallToolResult` whose first content block is a JSON manifest and (for authenticated callers only) whose remaining blocks are inline `image/jpeg` payloads. Three pure/DB-level helpers carry all logic so it is unit-testable without a Tauri `AppState`: request validation, per-item resolution, and response assembly with payload budgeting. Local stdio callers get generated-thumbnail file paths; authenticated callers get base64 blocks and no paths.

**Tech Stack:** Rust, `rmcp` 3.1 (MCP server), `rusqlite` via `crate::db_core::db::Database`, `image` 0.25 (`image_dimensions`), `base64` 0.23, `serde_json`, `schemars`.

**Spec:** `docs/superpowers/specs/2026-09-17-mcp-thumbnail-previews-design.md`

## Global Constraints

- Worktree: this plan is written for the isolated worktree `.worktrees/mcp-thumbnail-previews` on branch `feat/mcp-thumbnail-previews` inside the cull checkout; all paths below are relative to that worktree root. Do not put an absolute home path in this file — `npm test` scans `docs/` for personal absolute paths.
- Run all `cargo` commands from inside `src-tauri/` — there is no root `Cargo.toml`, so `cargo fmt` at the repo root is a silent no-op and the pre-push hook will fail.
- Never expose original or RAW source paths (`image.path`) in any preview output, under any transport. Only `db_core::thumbnails` paths may appear.
- Preview redaction keys off `AuthContext::Local` only — not `is_remote()`/`can_expose_private_metadata`, which would leak paths to admin-role tokens over HTTP.
- Sizes are the existing generated sizes only: `64`, `128`, `256`, `800` (longest edge, JPEG q90, from `db_core::thumbnails::THUMBNAIL_SIZES`). Never re-encode, resize, copy, or read originals.
- Bounds: 20 items per call; 2 MB per-image cap on **raw file bytes**; 8 MB authenticated budget on **base64-encoded** payload.
- Every new tool name must be added to `mcp/auth.rs::ALL_TOOLS`, `mcp/auth.rs::READ_TOOLS`, and `services/tokens.rs::tool_capability`, or existing tests fail.
- Do not touch the main checkout's dirty files (`src-tauri/src/extensions.rs`, `src-tauri/src/raw/fuji.rs`, `src-tauri/src/services/referenced_sources.rs`) — they belong to an unrelated branch.
- Do not add `get_image_previews` to the headless CLI slice (`src-tauri/src/cli/tools/mod.rs`); this tool is MCP-only.

---

### Task 1: Request validation and size fallback chain

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs` (add params struct after `ListFolderImagesParams`)
- Modify: `src-tauri/src/mcp/tools/library.rs` (imports, constants, `PreviewSelector`, `validate_preview_request`, `preview_size_chain`, tests)

**Interfaces:**
- Consumes: `crate::db_core::thumbnails::THUMBNAIL_SIZES: [u32; 4]`, `crate::db_core::thumbnails::sized_thumbnail_path`.
- Produces:
  - `pub struct GetImagePreviewsParams { image_ids: Option<Vec<String>>, folder_path: Option<String>, offset: Option<u32>, limit: Option<u32>, size: Option<u32> }` (in `mcp::tools`)
  - `pub(crate) const PREVIEW_MAX_ITEMS: usize = 20;`
  - `pub(crate) const PREVIEW_DEFAULT_SIZE: u32 = 256;`
  - `pub(crate) enum PreviewSelector { Ids(Vec<String>), Folder { path: String, offset: u32, limit: u32 } }`
  - `pub(crate) fn validate_preview_request(params: &GetImagePreviewsParams) -> Result<(PreviewSelector, u32), String>`
  - `pub(crate) fn preview_size_chain(size: u32) -> Vec<u32>`

- [ ] **Step 1: Add the params struct**

In `src-tauri/src/mcp/tools.rs`, immediately after the `ListFolderImagesParams` struct, add:

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetImagePreviewsParams {
    #[schemars(
        description = "Explicit image IDs to preview (1-20 distinct, deduplicated). Mutually exclusive with folder_path. Ignored offset/limit fields are allowed."
    )]
    pub image_ids: Option<Vec<String>>,
    #[schemars(
        description = "Folder path to preview a bounded page from. Mutually exclusive with image_ids."
    )]
    pub folder_path: Option<String>,
    #[schemars(description = "Page offset for folder_path mode (default 0, ignored for image_ids)")]
    pub offset: Option<u32>,
    #[schemars(description = "Page size for folder_path mode (1-20, default 20, ignored for image_ids)")]
    pub limit: Option<u32>,
    #[schemars(description = "Thumbnail size in px: 64, 128, 256 (default), or 800")]
    pub size: Option<u32>,
}
```

- [ ] **Step 2: Write the failing tests**

At the end of `src-tauri/src/mcp/tools/library.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ids_params(ids: &[&str]) -> GetImagePreviewsParams {
        GetImagePreviewsParams {
            image_ids: Some(ids.iter().map(|id| id.to_string()).collect()),
            folder_path: None,
            offset: None,
            limit: None,
            size: None,
        }
    }

    fn folder_params(path: &str, offset: Option<u32>, limit: Option<u32>) -> GetImagePreviewsParams {
        GetImagePreviewsParams {
            image_ids: None,
            folder_path: Some(path.to_string()),
            offset,
            limit,
            size: None,
        }
    }

    #[test]
    fn size_chain_falls_back_to_next_larger_generated_size() {
        assert_eq!(preview_size_chain(64), vec![64, 128, 256, 800]);
        assert_eq!(preview_size_chain(128), vec![128, 256, 800]);
        assert_eq!(preview_size_chain(256), vec![256, 800]);
        assert_eq!(preview_size_chain(800), vec![800]);
    }

    #[test]
    fn validate_rejects_unknown_size() {
        let mut params = ids_params(&["img_a"]);
        params.size = Some(512);
        let err = validate_preview_request(&params).unwrap_err();
        assert!(err.contains("invalid size"), "got: {err}");
    }

    #[test]
    fn validate_defaults_size_to_256() {
        let (_, size) = validate_preview_request(&ids_params(&["img_a"])).unwrap();
        assert_eq!(size, PREVIEW_DEFAULT_SIZE);
        assert_eq!(size, 256);
    }

    #[test]
    fn validate_requires_exactly_one_selector() {
        let neither = GetImagePreviewsParams {
            image_ids: None,
            folder_path: None,
            offset: None,
            limit: None,
            size: None,
        };
        assert!(validate_preview_request(&neither)
            .unwrap_err()
            .contains("exactly one"));

        let both = GetImagePreviewsParams {
            image_ids: Some(vec!["img_a".to_string()]),
            folder_path: Some("/lib".to_string()),
            offset: None,
            limit: None,
            size: None,
        };
        assert!(validate_preview_request(&both)
            .unwrap_err()
            .contains("exactly one"));
    }

    #[test]
    fn validate_dedupes_ids_before_applying_the_limit() {
        // 21 entries, but only 20 distinct: allowed.
        let mut ids: Vec<String> = (0..20).map(|i| format!("img_{i}")).collect();
        ids.push("img_0".to_string());
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let (selector, _) = validate_preview_request(&ids_params(&refs)).unwrap();
        match selector {
            PreviewSelector::Ids(resolved) => {
                assert_eq!(resolved.len(), 20);
                assert_eq!(resolved[0], "img_0");
            }
            other => panic!("expected Ids selector, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_more_than_twenty_distinct_ids() {
        let ids: Vec<String> = (0..21).map(|i| format!("img_{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let err = validate_preview_request(&ids_params(&refs)).unwrap_err();
        assert!(err.contains("at most 20"), "got: {err}");
    }

    #[test]
    fn validate_rejects_an_all_blank_id_list() {
        let err = validate_preview_request(&ids_params(&["", "  "])).unwrap_err();
        assert!(err.contains("at least one image_id"), "got: {err}");
    }

    #[test]
    fn validate_folder_page_clamps_limit_and_defaults_offset() {
        let (selector, _) = validate_preview_request(&folder_params("/lib", None, None)).unwrap();
        match selector {
            PreviewSelector::Folder { offset, limit, .. } => {
                assert_eq!(offset, 0);
                assert_eq!(limit, 20);
            }
            other => panic!("expected Folder selector, got {other:?}"),
        }

        let (selector, _) =
            validate_preview_request(&folder_params("/lib", Some(5), Some(100))).unwrap();
        match selector {
            PreviewSelector::Folder { offset, limit, .. } => {
                assert_eq!(offset, 5);
                assert_eq!(limit, 20);
            }
            other => panic!("expected Folder selector, got {other:?}"),
        }

        let (selector, _) =
            validate_preview_request(&folder_params("/lib", None, Some(0))).unwrap();
        match selector {
            PreviewSelector::Folder { limit, .. } => assert_eq!(limit, 1),
            other => panic!("expected Folder selector, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_blank_folder_path() {
        let err = validate_preview_request(&folder_params("   ", None, None)).unwrap_err();
        assert!(err.contains("folder_path"), "got: {err}");
    }
}
```

`PreviewSelector` needs `#[derive(Debug)]` for the `{other:?}` formatting above.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mcp::tools::library`
Expected: compile error — `GetImagePreviewsParams`, `preview_size_chain`, `validate_preview_request`, `PreviewSelector`, `PREVIEW_DEFAULT_SIZE` not found.

- [ ] **Step 4: Write the minimal implementation**

At the top of `src-tauri/src/mcp/tools/library.rs`, after the existing `use super::*;`, add:

```rust
use base64::Engine as _;
use std::path::Path;

use crate::db_core::db::Database;
use crate::db_core::models::TokenScope;
use crate::db_core::thumbnails;
```

Directly below the imports (above `#[tool_router(router = library_router)]`), add:

```rust
/// Hard cap on previews returned by a single `get_image_previews` call.
pub(crate) const PREVIEW_MAX_ITEMS: usize = 20;
/// Documented default thumbnail size for previews.
pub(crate) const PREVIEW_DEFAULT_SIZE: u32 = 256;

#[derive(Debug)]
pub(crate) enum PreviewSelector {
    Ids(Vec<String>),
    Folder { path: String, offset: u32, limit: u32 },
}

/// The generated sizes to try, smallest-acceptable first. A missing file at the
/// requested size falls back to the next larger generated size, so previews
/// never need to re-encode anything.
pub(crate) fn preview_size_chain(size: u32) -> Vec<u32> {
    thumbnails::THUMBNAIL_SIZES
        .iter()
        .copied()
        .filter(|candidate| *candidate >= size)
        .collect()
}

/// Normalize and validate `get_image_previews` params into a selector and a
/// thumbnail size. `offset`/`limit` are ignored in IDs mode.
pub(crate) fn validate_preview_request(
    params: &GetImagePreviewsParams,
) -> Result<(PreviewSelector, u32), String> {
    let size = params.size.unwrap_or(PREVIEW_DEFAULT_SIZE);
    if !thumbnails::THUMBNAIL_SIZES.contains(&size) {
        return Err(format!(
            "invalid size '{}'. Use one of 64, 128, 256, 800.",
            size
        ));
    }

    let has_ids = params.image_ids.is_some();
    let has_folder = params.folder_path.is_some();
    if has_ids == has_folder {
        return Err("requires exactly one of image_ids or folder_path".to_string());
    }

    if let Some(ids) = &params.image_ids {
        let mut deduped: Vec<String> = Vec::new();
        for id in ids {
            let id = id.trim();
            if id.is_empty() || deduped.iter().any(|existing| existing == id) {
                continue;
            }
            deduped.push(id.to_string());
        }
        if deduped.is_empty() {
            return Err("requires at least one image_id".to_string());
        }
        if deduped.len() > PREVIEW_MAX_ITEMS {
            return Err(format!(
                "accepts at most {} image_ids per call (got {})",
                PREVIEW_MAX_ITEMS,
                deduped.len()
            ));
        }
        return Ok((PreviewSelector::Ids(deduped), size));
    }

    let folder_path = params.folder_path.clone().unwrap_or_default();
    let folder_path = folder_path.trim().to_string();
    if folder_path.is_empty() {
        return Err("folder_path must not be empty".to_string());
    }
    let offset = params.offset.unwrap_or(0);
    let limit = params
        .limit
        .unwrap_or(PREVIEW_MAX_ITEMS as u32)
        .clamp(1, PREVIEW_MAX_ITEMS as u32);

    Ok((
        PreviewSelector::Folder {
            path: folder_path,
            offset,
            limit,
        },
        size,
    ))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mcp::tools::library`
Expected: PASS (8 tests).

- [ ] **Step 6: Format and commit**

```bash
cd src-tauri && cargo fmt
cd .. && git add src-tauri/src/mcp/tools.rs src-tauri/src/mcp/tools/library.rs
git commit -m "feat(mcp): validate thumbnail preview requests (imageview-9k1u.14)"
```

---

### Task 2: Per-item preview resolution

**Files:**
- Modify: `src-tauri/src/mcp/tools/library.rs` (add `PreviewStatus`, `PreviewItem`, `resolve_preview_items`, tests)

**Interfaces:**
- Consumes: `Database::get_images_by_ids(&[&str]) -> Result<Vec<ImageWithFile>>`, `crate::services::tokens::image_id_in_scope(db, scope, image_id) -> Result<bool, String>`, `preview_size_chain`, `thumbnails::sized_thumbnail_path`, `image::image_dimensions`.
- Produces:
  - `pub(crate) enum PreviewStatus { Ok, Missing, NotFound, Unavailable, SkippedTooLarge, SkippedBudget }` with `pub(crate) fn as_str(&self) -> &'static str`
  - `pub(crate) struct PreviewItem { image_id: String, status: PreviewStatus, thumbnail_size: Option<u32>, width: Option<u32>, height: Option<u32>, bytes: Option<u64>, thumbnail_path: Option<String>, inline_bytes: Option<Vec<u8>> }` with `pub(crate) fn unresolved(image_id: &str, status: PreviewStatus) -> Self`
  - `pub(crate) fn resolve_preview_items(db: &Database, app_data_dir: &Path, scope: &Option<TokenScope>, local: bool, image_ids: &[String], size: u32) -> Result<Vec<PreviewItem>, String>`

- [ ] **Step 1: Write the failing tests**

Append to the existing `mod tests` in `src-tauri/src/mcp/tools/library.rs`:

```rust
    use crate::db_core::models::{Image, ImageFile};
    use crate::db_core::thumbnails as thumbnails_mod;
    use std::path::PathBuf;

    fn test_db() -> Database {
        Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    fn insert_test_image(db: &Database, id: &str, source_path: &str) {
        db.insert_image(&Image {
            id: id.to_string(),
            sha256_hash: format!("hash-{id}"),
            width: 8,
            height: 8,
            format: "jpeg".to_string(),
            file_size: 1,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            imported_at: "2026-01-01T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: format!("file-{id}"),
            image_id: id.to_string(),
            path: source_path.to_string(),
            last_seen_at: "2026-01-01T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        })
        .unwrap();
    }

    fn write_thumbnail(app_data_dir: &Path, image_id: &str, size: u32) -> PathBuf {
        let path = thumbnails_mod::sized_thumbnail_path(app_data_dir, image_id, size);
        image::DynamicImage::new_rgb8(8, 8).save(&path).unwrap();
        path
    }

    fn folder_scope(folder: &str) -> Option<TokenScope> {
        Some(TokenScope {
            collections: None,
            folders: Some(vec![folder.to_string()]),
            tags: None,
        })
    }

    #[test]
    fn resolve_reports_not_found_locally_and_unavailable_for_tokens() {
        let db = test_db();
        let tmp = tempfile::tempdir().unwrap();
        let ids = vec!["ghost".to_string()];

        let local = resolve_preview_items(&db, tmp.path(), &None, true, &ids, 256).unwrap();
        assert_eq!(local[0].status, PreviewStatus::NotFound);

        let scoped = resolve_preview_items(
            &db,
            tmp.path(),
            &folder_scope("/lib"),
            false,
            &ids,
            256,
        )
        .unwrap();
        assert_eq!(scoped[0].status, PreviewStatus::Unavailable);
    }

    #[test]
    fn resolve_hides_out_of_scope_images_behind_unavailable() {
        let db = test_db();
        let tmp = tempfile::tempdir().unwrap();
        insert_test_image(&db, "inside", "/lib/a.jpg");
        insert_test_image(&db, "outside", "/other/b.jpg");
        write_thumbnail(tmp.path(), "inside", 256);
        write_thumbnail(tmp.path(), "outside", 256);

        let ids = vec!["inside".to_string(), "outside".to_string()];
        let items = resolve_preview_items(
            &db,
            tmp.path(),
            &folder_scope("/lib"),
            false,
            &ids,
            256,
        )
        .unwrap();

        assert_eq!(items[0].status, PreviewStatus::Ok);
        assert_eq!(items[1].status, PreviewStatus::Unavailable);
        assert!(items[1].thumbnail_path.is_none());
        assert!(items[1].inline_bytes.is_none());
    }

    #[test]
    fn resolve_reports_missing_thumbnails_without_failing_other_items() {
        let db = test_db();
        let tmp = tempfile::tempdir().unwrap();
        insert_test_image(&db, "has_thumb", "/lib/a.jpg");
        insert_test_image(&db, "no_thumb", "/lib/b.jpg");
        write_thumbnail(tmp.path(), "has_thumb", 256);

        let ids = vec!["no_thumb".to_string(), "has_thumb".to_string()];
        let items = resolve_preview_items(&db, tmp.path(), &None, true, &ids, 256).unwrap();

        assert_eq!(items[0].status, PreviewStatus::Missing);
        assert_eq!(items[1].status, PreviewStatus::Ok);
    }

    #[test]
    fn resolve_falls_back_to_a_larger_generated_size() {
        let db = test_db();
        let tmp = tempfile::tempdir().unwrap();
        insert_test_image(&db, "big_only", "/lib/a.jpg");
        write_thumbnail(tmp.path(), "big_only", 800);

        let ids = vec!["big_only".to_string()];
        let items = resolve_preview_items(&db, tmp.path(), &None, true, &ids, 256).unwrap();

        assert_eq!(items[0].status, PreviewStatus::Ok);
        assert_eq!(items[0].thumbnail_size, Some(800));
        let path = items[0].thumbnail_path.clone().unwrap();
        assert!(path.ends_with("big_only.jpg"), "got: {path}");
    }

    #[test]
    fn resolve_local_returns_thumbnail_paths_and_never_source_paths() {
        let db = test_db();
        let tmp = tempfile::tempdir().unwrap();
        insert_test_image(&db, "raf", "/lib/frame.RAF");
        let thumb = write_thumbnail(tmp.path(), "raf", 256);

        let ids = vec!["raf".to_string()];
        let items = resolve_preview_items(&db, tmp.path(), &None, true, &ids, 256).unwrap();

        assert_eq!(items[0].status, PreviewStatus::Ok);
        let served = items[0].thumbnail_path.clone().unwrap();
        assert_eq!(PathBuf::from(&served), thumb);
        assert!(served.ends_with("raf_256.jpg"), "got: {served}");
        assert!(!served.ends_with(".RAF"));
        assert!(items[0].inline_bytes.is_none());
        assert_eq!(items[0].width, Some(8));
        assert_eq!(items[0].height, Some(8));
        assert!(items[0].bytes.unwrap() > 0);
    }

    #[test]
    fn resolve_authenticated_inlines_jpeg_bytes_and_no_paths() {
        let db = test_db();
        let tmp = tempfile::tempdir().unwrap();
        insert_test_image(&db, "a", "/lib/a.jpeg");
        let thumb = write_thumbnail(tmp.path(), "a", 256);

        let ids = vec!["a".to_string()];
        let items = resolve_preview_items(&db, tmp.path(), &None, false, &ids, 256).unwrap();

        assert_eq!(items[0].status, PreviewStatus::Ok);
        assert!(items[0].thumbnail_path.is_none());
        let inlined = items[0].inline_bytes.clone().unwrap();
        assert_eq!(inlined, std::fs::read(&thumb).unwrap());
    }

    #[test]
    fn resolve_empty_id_list_is_empty() {
        let db = test_db();
        let tmp = tempfile::tempdir().unwrap();
        let items = resolve_preview_items(&db, tmp.path(), &None, true, &[], 256).unwrap();
        assert!(items.is_empty());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mcp::tools::library`
Expected: compile error — `resolve_preview_items`, `PreviewStatus`, `PreviewItem` not found.

- [ ] **Step 3: Write the minimal implementation**

In `src-tauri/src/mcp/tools/library.rs`, directly after `preview_size_chain`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreviewStatus {
    Ok,
    Missing,
    NotFound,
    Unavailable,
    SkippedTooLarge,
    SkippedBudget,
}

impl PreviewStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            PreviewStatus::Ok => "ok",
            PreviewStatus::Missing => "missing",
            PreviewStatus::NotFound => "not_found",
            PreviewStatus::Unavailable => "unavailable",
            PreviewStatus::SkippedTooLarge => "skipped_too_large",
            PreviewStatus::SkippedBudget => "skipped_budget",
        }
    }
}

/// One requested preview after authorization and thumbnail lookup, before
/// payload budgeting. `thumbnail_path` is populated for local transport only;
/// `inline_bytes` for authenticated transport only.
#[derive(Debug, Clone)]
pub(crate) struct PreviewItem {
    pub image_id: String,
    pub status: PreviewStatus,
    pub thumbnail_size: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bytes: Option<u64>,
    pub thumbnail_path: Option<String>,
    pub inline_bytes: Option<Vec<u8>>,
}

impl PreviewItem {
    pub(crate) fn unresolved(image_id: &str, status: PreviewStatus) -> Self {
        PreviewItem {
            image_id: image_id.to_string(),
            status,
            thumbnail_size: None,
            width: None,
            height: None,
            bytes: None,
            thumbnail_path: None,
            inline_bytes: None,
        }
    }
}

/// Resolve each requested image ID to a generated thumbnail.
///
/// Authorization runs per item via `tokens::image_id_in_scope`, so an
/// out-of-scope ID is indistinguishable from an unknown one for authenticated
/// callers. Source paths (`ImageWithFile::path`) are intentionally never read
/// here: the preview contract covers generated thumbnails only.
pub(crate) fn resolve_preview_items(
    db: &Database,
    app_data_dir: &Path,
    scope: &Option<TokenScope>,
    local: bool,
    image_ids: &[String],
    size: u32,
) -> Result<Vec<PreviewItem>, String> {
    if image_ids.is_empty() {
        return Ok(Vec::new());
    }

    let refs: Vec<&str> = image_ids.iter().map(String::as_str).collect();
    let images = db.get_images_by_ids(&refs).map_err(|e| e.to_string())?;
    let known: std::collections::HashSet<&str> =
        images.iter().map(|image| image.image.id.as_str()).collect();

    let chain = preview_size_chain(size);
    let mut items = Vec::with_capacity(image_ids.len());

    for image_id in image_ids {
        if !tokens::image_id_in_scope(db, scope, image_id).map_err(|e| e.to_string())? {
            items.push(PreviewItem::unresolved(image_id, PreviewStatus::Unavailable));
            continue;
        }
        if !known.contains(image_id.as_str()) {
            let status = if local {
                PreviewStatus::NotFound
            } else {
                PreviewStatus::Unavailable
            };
            items.push(PreviewItem::unresolved(image_id, status));
            continue;
        }

        let served = chain.iter().find_map(|&candidate| {
            let path = thumbnails::sized_thumbnail_path(app_data_dir, image_id, candidate);
            path.exists().then_some((candidate, path))
        });
        let Some((served_size, thumbnail_file)) = served else {
            items.push(PreviewItem::unresolved(image_id, PreviewStatus::Missing));
            continue;
        };

        let bytes = std::fs::metadata(&thumbnail_file)
            .ok()
            .map(|metadata| metadata.len());
        let dimensions = image::image_dimensions(&thumbnail_file).ok();
        let inline_bytes = if local {
            None
        } else {
            std::fs::read(&thumbnail_file).ok()
        };
        if !local && inline_bytes.is_none() {
            items.push(PreviewItem::unresolved(image_id, PreviewStatus::Missing));
            continue;
        }

        items.push(PreviewItem {
            image_id: image_id.clone(),
            status: PreviewStatus::Ok,
            thumbnail_size: Some(served_size),
            width: dimensions.map(|(width, _)| width),
            height: dimensions.map(|(_, height)| height),
            bytes,
            thumbnail_path: local.then(|| thumbnail_file.to_string_lossy().to_string()),
            inline_bytes,
        });
    }

    Ok(items)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mcp::tools::library`
Expected: PASS (16 tests).

- [ ] **Step 5: Format and commit**

```bash
cd src-tauri && cargo fmt
cd .. && git add src-tauri/src/mcp/tools/library.rs
git commit -m "feat(mcp): resolve preview thumbnails with scope isolation (imageview-9k1u.14)"
```

---

### Task 3: Response assembly with payload bounds

**Files:**
- Modify: `src-tauri/src/mcp/tools/library.rs` (constants, `PreviewResponse`, `build_preview_response`, `base64_encoded_len`, tests)

**Interfaces:**
- Consumes: `PreviewItem`, `PreviewStatus`.
- Produces:
  - `pub(crate) const PREVIEW_PER_IMAGE_MAX_BYTES: u64 = 2 * 1024 * 1024;`
  - `pub(crate) const PREVIEW_TOTAL_MAX_BYTES: u64 = 8 * 1024 * 1024;`
  - `pub(crate) struct PreviewResponse { pub manifest: serde_json::Value, pub image_blocks: Vec<Vec<u8>> }`
  - `pub(crate) fn build_preview_response(items: Vec<PreviewItem>, local: bool, size_requested: u32) -> PreviewResponse`

- [ ] **Step 1: Write the failing tests**

Append to the existing `mod tests` in `src-tauri/src/mcp/tools/library.rs`:

```rust
    fn ok_local_item(image_id: &str) -> PreviewItem {
        PreviewItem {
            image_id: image_id.to_string(),
            status: PreviewStatus::Ok,
            thumbnail_size: Some(256),
            width: Some(8),
            height: Some(8),
            bytes: Some(123),
            thumbnail_path: Some("/app/thumbnails/x_256.jpg".to_string()),
            inline_bytes: None,
        }
    }

    fn ok_inline_item(image_id: &str, raw_bytes: usize) -> PreviewItem {
        PreviewItem {
            image_id: image_id.to_string(),
            status: PreviewStatus::Ok,
            thumbnail_size: Some(256),
            width: Some(8),
            height: Some(8),
            bytes: Some(raw_bytes as u64),
            thumbnail_path: None,
            inline_bytes: Some(vec![0u8; raw_bytes]),
        }
    }

    #[test]
    fn response_local_manifest_carries_paths_and_no_image_blocks() {
        let response = build_preview_response(vec![ok_local_item("a")], true, 256);

        assert!(response.image_blocks.is_empty());
        assert_eq!(response.manifest["transport"], "local_paths");
        assert_eq!(response.manifest["size_requested"], 256);
        assert_eq!(response.manifest["count"], 1);
        assert_eq!(response.manifest["items"][0]["status"], "ok");
        assert_eq!(
            response.manifest["items"][0]["thumbnail_path"],
            "/app/thumbnails/x_256.jpg"
        );
        assert!(response.manifest["items"][0].get("content_index").is_none());
    }

    #[test]
    fn response_authenticated_manifest_never_leaks_paths_or_block_payloads() {
        // Deliberately poison the item with a path AND an inline payload, so it
        // resolves to `ok` and the manifest must still drop the path.
        let poisoned = PreviewItem {
            image_id: "a".to_string(),
            status: PreviewStatus::Ok,
            thumbnail_size: Some(256),
            width: Some(8),
            height: Some(8),
            bytes: Some(32),
            thumbnail_path: Some("/app/thumbnails/x_256.jpg".to_string()),
            inline_bytes: Some(vec![0u8; 32]),
        };
        let response = build_preview_response(vec![poisoned], false, 256);

        let json = response.manifest.to_string();
        assert_eq!(response.manifest["transport"], "inline_base64");
        assert_eq!(response.manifest["items"][0]["status"], "ok");
        assert_eq!(response.manifest["items"][0]["content_index"], 0);
        assert_eq!(response.image_blocks.len(), 1);
        assert!(response.manifest["items"][0].get("thumbnail_path").is_none());
        assert!(!json.contains("/app/thumbnails"), "leaked a path: {json}");
        assert!(!json.contains("x_256.jpg"), "leaked a filename: {json}");
    }

    #[test]
    fn response_links_blocks_to_items_by_content_index() {
        let items = vec![ok_inline_item("a", 32), ok_inline_item("b", 32)];
        let response = build_preview_response(items, false, 256);

        assert_eq!(response.image_blocks.len(), 2);
        assert_eq!(response.manifest["items"][0]["content_index"], 0);
        assert_eq!(response.manifest["items"][1]["content_index"], 1);
    }

    #[test]
    fn response_marks_oversized_images_as_skipped_too_large() {
        let huge = PREVIEW_PER_IMAGE_MAX_BYTES as usize + 1;
        let response =
            build_preview_response(vec![ok_inline_item("huge", huge)], false, 256);

        assert!(response.image_blocks.is_empty());
        assert_eq!(response.manifest["items"][0]["status"], "skipped_too_large");
        assert_eq!(
            response.manifest["items"][0]["bytes"],
            PREVIEW_PER_IMAGE_MAX_BYTES + 1
        );
    }

    #[test]
    fn response_enforces_the_total_budget_deterministically() {
        // 1_800_000 raw bytes -> 2_400_000 base64 bytes each; the fourth item
        // would push the running total past the 8 MB budget.
        let raw = 1_800_000usize;
        let items = vec![
            ok_inline_item("a", raw),
            ok_inline_item("b", raw),
            ok_inline_item("c", raw),
            ok_inline_item("d", raw),
        ];
        let response = build_preview_response(items, false, 256);

        assert_eq!(response.image_blocks.len(), 3);
        assert_eq!(response.manifest["items"][0]["status"], "ok");
        assert_eq!(response.manifest["items"][2]["status"], "ok");
        assert_eq!(response.manifest["items"][3]["status"], "skipped_budget");
        assert!(response.manifest["items"][3].get("content_index").is_none());
    }

    #[test]
    fn response_keeps_non_ok_statuses_when_the_budget_is_exhausted() {
        let raw = 1_800_000usize;
        let items = vec![
            ok_inline_item("a", raw),
            ok_inline_item("b", raw),
            ok_inline_item("c", raw),
            PreviewItem::unresolved("missing_one", PreviewStatus::Missing),
            ok_inline_item("d", raw),
        ];
        let response = build_preview_response(items, false, 256);

        assert_eq!(response.manifest["items"][3]["status"], "missing");
        assert_eq!(response.manifest["items"][4]["status"], "skipped_budget");
    }

    #[test]
    fn response_downgrades_an_ok_item_without_payload_to_missing() {
        let broken = PreviewItem {
            image_id: "broken".to_string(),
            status: PreviewStatus::Ok,
            thumbnail_size: Some(256),
            width: None,
            height: None,
            bytes: None,
            thumbnail_path: None,
            inline_bytes: None,
        };
        let response = build_preview_response(vec![broken], false, 256);
        assert_eq!(response.manifest["items"][0]["status"], "missing");
        assert!(response.image_blocks.is_empty());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mcp::tools::library`
Expected: compile error — `build_preview_response`, `PreviewResponse`, `PREVIEW_PER_IMAGE_MAX_BYTES` not found.

- [ ] **Step 3: Write the minimal implementation**

In `src-tauri/src/mcp/tools/library.rs`, extend the constants block near `PREVIEW_MAX_ITEMS`:

```rust
/// Per-image cap on the raw generated thumbnail file size.
pub(crate) const PREVIEW_PER_IMAGE_MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Total base64 payload budget for one authenticated preview response.
pub(crate) const PREVIEW_TOTAL_MAX_BYTES: u64 = 8 * 1024 * 1024;
```

Then, after `resolve_preview_items`, add:

```rust
pub(crate) struct PreviewResponse {
    pub manifest: serde_json::Value,
    pub image_blocks: Vec<Vec<u8>>,
}

fn base64_encoded_len(raw_len: usize) -> u64 {
    ((raw_len as u64 + 2) / 3) * 4
}

/// Apply the payload caps and build the manifest plus the ordered image blocks.
///
/// `image_blocks[i]` is the payload advertised by the manifest item whose
/// `content_index == i`. Once the total budget is exhausted, every remaining
/// item that would otherwise be `ok` becomes `skipped_budget`.
pub(crate) fn build_preview_response(
    items: Vec<PreviewItem>,
    local: bool,
    size_requested: u32,
) -> PreviewResponse {
    let mut running_budget: u64 = 0;
    let mut image_blocks: Vec<Vec<u8>> = Vec::new();
    let mut manifest_items: Vec<serde_json::Value> = Vec::with_capacity(items.len());

    for mut item in items {
        let mut status = item.status.clone();
        let mut content_index: Option<usize> = None;

        if status == PreviewStatus::Ok {
            if local {
                if item.thumbnail_path.is_none() {
                    status = PreviewStatus::Missing;
                }
            } else {
                match item.inline_bytes.take() {
                    Some(bytes) => {
                        if bytes.len() as u64 > PREVIEW_PER_IMAGE_MAX_BYTES {
                            status = PreviewStatus::SkippedTooLarge;
                        } else {
                            let encoded = base64_encoded_len(bytes.len());
                            if running_budget + encoded > PREVIEW_TOTAL_MAX_BYTES {
                                status = PreviewStatus::SkippedBudget;
                            } else {
                                running_budget += encoded;
                                content_index = Some(image_blocks.len());
                                image_blocks.push(bytes);
                            }
                        }
                    }
                    None => status = PreviewStatus::Missing,
                }
            }
        }

        let mut value = serde_json::json!({
            "image_id": item.image_id,
            "status": status.as_str(),
        });

        match status {
            PreviewStatus::Ok => {
                if let Some(size) = item.thumbnail_size {
                    value["thumbnail_size"] = serde_json::json!(size);
                }
                if let Some(width) = item.width {
                    value["width"] = serde_json::json!(width);
                }
                if let Some(height) = item.height {
                    value["height"] = serde_json::json!(height);
                }
                if let Some(bytes) = item.bytes {
                    value["bytes"] = serde_json::json!(bytes);
                }
                if local {
                    if let Some(path) = &item.thumbnail_path {
                        value["thumbnail_path"] = serde_json::json!(path);
                    }
                } else if let Some(index) = content_index {
                    value["content_index"] = serde_json::json!(index);
                }
            }
            PreviewStatus::SkippedTooLarge | PreviewStatus::SkippedBudget => {
                if let Some(size) = item.thumbnail_size {
                    value["thumbnail_size"] = serde_json::json!(size);
                }
                if let Some(bytes) = item.bytes {
                    value["bytes"] = serde_json::json!(bytes);
                }
            }
            PreviewStatus::Missing | PreviewStatus::NotFound | PreviewStatus::Unavailable => {}
        }

        manifest_items.push(value);
    }

    let manifest = serde_json::json!({
        "size_requested": size_requested,
        "transport": if local { "local_paths" } else { "inline_base64" },
        "count": manifest_items.len(),
        "items": manifest_items,
    });

    PreviewResponse {
        manifest,
        image_blocks,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mcp::tools::library`
Expected: PASS (24 tests).

- [ ] **Step 5: Format and commit**

```bash
cd src-tauri && cargo fmt
cd .. && git add src-tauri/src/mcp/tools/library.rs
git commit -m "feat(mcp): bound preview payloads and build the manifest (imageview-9k1u.14)"
```

---

### Task 4: Register the tool, wire capability and audit, document the contract

**Files:**
- Modify: `src-tauri/src/mcp/tools/library.rs` (the `#[tool]` handler, `preview_error`, `preview_tool_result`)
- Modify: `src-tauri/src/mcp/auth.rs` (`ALL_TOOLS`, `READ_TOOLS`, plus a capability test)
- Modify: `src-tauri/src/services/tokens.rs` (`tool_capability` `library:read` arm)
- Modify: `docs/agents.md` (new section)

**Interfaces:**
- Consumes: `validate_preview_request`, `resolve_preview_items`, `build_preview_response`, `PreviewSelector`, `CullMcp::token_scope()`, `CullMcp::app_handle`, `AuthContext::Local`, `tokens::folder_in_scope`, `Database::list_images_by_folder`.
- Produces: registered MCP tool `get_image_previews` returning `rmcp::model::CallToolResult`.

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/mcp/auth.rs`, inside the existing `mod tests`, add near the other capability tests:

```rust
    #[test]
    fn test_thumbnail_preview_tool_is_read_only_and_viewer_accessible() {
        assert!(ALL_TOOLS.contains(&"get_image_previews"));
        assert!(READ_TOOLS.contains(&"get_image_previews"));
        assert_eq!(
            tokens::tool_capability("get_image_previews"),
            "library:read"
        );
        assert!(require_capability(&viewer_auth(), "get_image_previews").is_ok());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test --lib mcp::auth::tests::test_thumbnail_preview_tool_is_read_only_and_viewer_accessible`
Expected: FAIL — `ALL_TOOLS.contains("get_image_previews")` is false.

- [ ] **Step 3: Wire the capability, tool lists, and handler**

In `src-tauri/src/services/tokens.rs`, add `| "get_image_previews"` to the `library:read` arm, immediately after `| "list_folder_images"`:

```rust
        "list_images"
        | "get_image"
        | "list_folders"
        | "list_folder_images"
        | "get_image_previews"
```

In `src-tauri/src/mcp/auth.rs`, add `"get_image_previews",` to `ALL_TOOLS` immediately after `"list_folder_images",`, and to `READ_TOOLS` immediately after `"list_folder_images",`.

In `src-tauri/src/mcp/tools/library.rs`, add the handler and helpers at the end of the `#[tool_router(router = library_router)] impl CullMcp` block (after `list_folder_images`):

```rust
    #[tool(
        description = "Return bounded preview thumbnails for explicit image IDs (max 20, deduplicated) or a bounded folder page. Local stdio returns generated thumbnail file paths; authenticated callers receive inline base64 image/jpeg blocks with no filesystem paths. Sizes: 64, 128, 256 (default), 800. Never exposes originals or RAW files."
    )]
    fn get_image_previews(
        &self,
        Parameters(params): Parameters<GetImagePreviewsParams>,
    ) -> CallToolResult {
        let (selector, size) = match validate_preview_request(&params) {
            Ok(valid) => valid,
            Err(e) => return preview_error(&e),
        };

        let state = self.app_handle.state::<AppState>();
        let local = matches!(self.auth, AuthContext::Local);
        let scope = self.token_scope();

        let image_ids: Vec<String> = match selector {
            PreviewSelector::Ids(ids) => ids,
            PreviewSelector::Folder {
                path,
                offset,
                limit,
            } => {
                // A folder outside the token scope is an explicit error; the
                // per-image scope check below stays the single source of truth
                // for item authorization in both modes.
                if !tokens::folder_in_scope(&scope, &path) {
                    return preview_error("folder is not available in this token scope");
                }
                match state.db.list_images_by_folder(&path, limit, offset) {
                    Ok(images) => images
                        .into_iter()
                        .map(|image| image.image.id)
                        .collect(),
                    Err(e) => return preview_error(&e.to_string()),
                }
            }
        };

        let items = match resolve_preview_items(
            &state.db,
            &state.app_data_dir,
            &scope,
            local,
            &image_ids,
            size,
        ) {
            Ok(items) => items,
            Err(e) => return preview_error(&e),
        };

        preview_tool_result(build_preview_response(items, local, size))
    }
}

fn preview_error(message: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(format!("Error: {}", message))])
}

fn preview_tool_result(response: PreviewResponse) -> CallToolResult {
    let mut content = vec![ContentBlock::text(response.manifest.to_string())];
    content.extend(response.image_blocks.iter().map(|bytes| {
        ContentBlock::image(
            base64::engine::general_purpose::STANDARD.encode(bytes),
            "image/jpeg",
        )
    }));
    CallToolResult::success(content)
}
```

Note: `preview_error` and `preview_tool_result` are free functions placed after the `impl` block; the existing `router()` function stays at the bottom of the file, after them.

Also add the two `rmcp` model types to the imports at the top of `library.rs`:

```rust
use rmcp::model::{CallToolResult, ContentBlock};
```

- [ ] **Step 4: Run the test and the module suite**

Run: `cd src-tauri && cargo test --lib mcp::`
Expected: PASS, including `test_thumbnail_preview_tool_is_read_only_and_viewer_accessible`, `test_all_defined_tools_have_explicit_capability_mapping`, and the viewer/curator/operator/admin matrices.

Run: `cd src-tauri && cargo test --lib mcp::tools::library`
Expected: PASS (24 tests).

- [ ] **Step 5: Document the contract in `docs/agents.md`**

Insert a new section after section 5 ("The agent_snapshots demo loop") and before "## Launch demo (the keep-anyway loop)":

````markdown
## 6. Thumbnail previews (evidence-led shortlisting)

`get_image_previews` gives an agent a bounded visual view of candidates from
Cull's own generated thumbnails, without a UI snapshot and without access to
originals. Use it after `list_folder_images` to shortlist, then act with
`set_rating` / `set_decision`.

```jsonc
// MCP call_tool: get_image_previews — explicit IDs
{ "image_ids": ["img_a", "img_b"], "size": 256 }

// MCP call_tool: get_image_previews — a bounded folder page
{ "folder_path": "/Users/me/renders", "offset": 0, "limit": 20, "size": 256 }
```

Exactly one of `image_ids` / `folder_path` per call. `offset` and `limit` apply
to folder pages only and are ignored for `image_ids`.

### Sizes

| `size` | Longest edge | Notes |
| --- | --- | --- |
| 64 | 64 px | cheapest |
| 128 | 128 px | coarse comparison |
| 256 | 256 px | **default** |
| 800 | 800 px | best fidelity, largest payload |

Only these already-generated sizes are served (JPEG, quality 90). If the
requested size is absent, the next larger generated size is served and the
manifest reports the real one in `thumbnail_size`. No thumbnail is ever
re-encoded, resized, copied, or read from an original.

### Transport contract

| | Local stdio | Authenticated (HTTP/token, any role) |
| --- | --- | --- |
| `transport` | `local_paths` | `inline_base64` |
| `thumbnail_path` | absolute path to the generated file | never present |
| Image blocks | none | one `image/jpeg` block per preview |
| Source paths (original / RAW) | never present | never present |

An authenticated caller gets the manifest as the first content block and one
image block per returned preview, linked by `content_index`. Paths are never
sent to an authenticated caller — including an admin token.

### Bounds

- At most **20** previews per call. More than 20 distinct `image_ids` is an
  error, not a silent truncation.
- At most **2 MB** per generated thumbnail file; larger files are reported as
  `skipped_too_large`.
- At most **8 MB** of base64 payload per authenticated response; once the budget
  is spent, that item and every later `ok` item are reported as
  `skipped_budget`.

### Per-image statuses

| `status` | Meaning |
| --- | --- |
| `ok` | Generated thumbnail found and returned |
| `missing` | Image is authorized and present, but has no generated thumbnail |
| `not_found` | Unknown image ID (local stdio only) |
| `unavailable` | Unknown **or** outside the token scope (authenticated only) |
| `skipped_too_large` | Thumbnail exceeds the 2 MB per-image cap |
| `skipped_budget` | The 8 MB response budget was already spent |

One bad item never fails the call; the rest are returned normally. `unavailable`
deliberately does not distinguish unknown from out-of-scope, so a scoped token
cannot probe library existence by ID. A RAF file previews from its generated
JPEG thumbnail like any other image.

`get_image_previews` is scope-authorized and audit-logged like every other tool
(see `get_audit_log`). It is **MCP-only**: it is not part of the headless CLI
slice, so `cull --json call_tool get_image_previews` reports it as unsupported.
````

- [ ] **Step 6: Run the full verification for this feature**

```bash
cd src-tauri && cargo fmt --all -- --check
cd src-tauri && cargo test --lib
cd .. && npm run check
cd .. && npm test
```

Expected: all pass. `cargo test --lib` must be the FULL suite (per `AGENTS.md`,
a scoped run misses cross-module regressions). `npm test` includes
`src/lib/open-source-release-contract.test.ts`, which reads `docs/agents.md`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/mcp/tools/library.rs src-tauri/src/mcp/auth.rs src-tauri/src/services/tokens.rs docs/agents.md
git commit -m "feat(mcp): expose get_image_previews tool and document the contract (imageview-9k1u.14)"
```

---

## Post-Implementation Checklist

- [ ] `cd src-tauri && cargo fmt --all -- --check` passes.
- [ ] `cd src-tauri && cargo clippy --all-targets` produces no new warnings.
- [ ] `cd src-tauri && cargo test --lib` (full suite) passes.
- [ ] `npm run check` passes.
- [ ] `npm test` passes, including the docs contract test.
- [ ] `git log --oneline origin/main..HEAD` shows only the spec, plan, and four feature commits.
- [ ] `grep -rn "thumbnail_path" src-tauri/src/mcp/` shows the field only inside the preview code paths and always gated on `local`.
- [ ] Close the beads issue: `npm run bd -- close imageview-9k1u.14`
- [ ] Hand off with `npm run land:feature -- feat/mcp-thumbnail-previews` when the maintainer asks to land it.
