use super::*;

use base64::Engine as _;
use std::path::Path;

use crate::db_core::db::Database;
use crate::db_core::models::TokenScope;
use crate::db_core::thumbnails;

/// Hard cap on previews returned by a single `get_image_previews` call.
pub(crate) const PREVIEW_MAX_ITEMS: usize = 20;
/// Documented default thumbnail size for previews.
pub(crate) const PREVIEW_DEFAULT_SIZE: u32 = 256;

#[derive(Debug)]
pub(crate) enum PreviewSelector {
    Ids(Vec<String>),
    Folder {
        path: String,
        offset: u32,
        limit: u32,
    },
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
            items.push(PreviewItem::unresolved(
                image_id,
                PreviewStatus::Unavailable,
            ));
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

#[tool_router(router = library_router)]
impl CullMcp {
    #[tool(description = "Get library statistics: image count, folder count, collection count")]
    fn get_library_stats(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let scoped_counts = match self.scoped_library_counts(&state) {
            Ok(counts) => counts,
            Err(e) => return format!("Error: {}", e),
        };
        let image_count = state.db.image_count().unwrap_or(0);
        let folders = state.db.list_folders().unwrap_or_default();
        let collections = state.db.list_collections().unwrap_or_default();

        library_stats_for_mcp(image_count, folders.len(), collections.len(), scoped_counts)
            .to_string()
    }

    #[tool(
        description = "List images with pagination. Returns id, path, dimensions, format, rating, decision."
    )]
    fn list_images(&self, Parameters(params): Parameters<ListImagesParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let offset = params.offset.unwrap_or(0);
        let limit = clamp_limit(params.limit.unwrap_or(50));

        // Scoped tokens filter and paginate at the SQL level (folder/collection/
        // tag union), so pages are correct for sparse scopes and large libraries
        // without the old `limit * 3` heuristic. Unscoped (local) tokens list
        // the whole library.
        let images = match self.token_scope() {
            Some(scope) => {
                let (folders, collections, tag_norms) = Self::scope_dimensions(&scope);
                state
                    .db
                    .list_images_in_scope(&folders, &collections, &tag_norms, limit, offset)
            }
            None => state.db.list_images(limit, offset),
        };

        match images {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images
                    .iter()
                    .map(|img| {
                        serde_json::json!({
                            "id": img.image.id,
                            "path": self.maybe_redact_path(&img.path),
                            "width": img.image.width,
                            "height": img.image.height,
                            "format": img.image.format,
                            "file_size": img.image.file_size,
                            "rating": img.selection.as_ref().and_then(|s| s.star_rating),
                            "decision": img.selection.as_ref().map(|s| &s.decision),
                        })
                    })
                    .collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get a single image with all metadata by ID")]
    fn get_image(&self, Parameters(params): Parameters<GetImageParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let id_refs = vec![params.image_id.as_str()];

        match state.db.get_images_by_ids(&id_refs) {
            Ok(images) => match images.into_iter().next() {
                Some(img) => {
                    match self.check_image_id_scope(&params.image_id) {
                        Ok(true) => {}
                        Ok(false) => {
                            return format!("Error: Image '{}' not found", params.image_id)
                        }
                        Err(e) => return format!("Error: {}", e),
                    }
                    serde_json::json!({
                        "id": img.image.id,
                        "path": self.maybe_redact_path(&img.path),
                        "width": img.image.width,
                        "height": img.image.height,
                        "format": img.image.format,
                        "file_size": img.image.file_size,
                        "created_at": img.image.created_at,
                        "imported_at": img.image.imported_at,
                        "rating": img.selection.as_ref().and_then(|s| s.star_rating),
                        "decision": img.selection.as_ref().map(|s| &s.decision),
                    })
                    .to_string()
                }
                None => format!("Error: Image '{}' not found", params.image_id),
            },
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List all imported folders with image counts")]
    fn list_folders(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let scope = self.token_scope();
        match state.db.list_folders() {
            Ok(folders) => {
                let result: Vec<serde_json::Value> = folders.iter()
                .filter(|(path, _)| tokens::folder_in_scope(&scope, path))
                .map(|(path, count)| {
                    serde_json::json!({"path": self.maybe_redact_path(path), "image_count": count})
                }).collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List images in a specific folder with pagination")]
    fn list_folder_images(&self, Parameters(params): Parameters<ListFolderImagesParams>) -> String {
        let scope = self.token_scope();
        if !tokens::folder_in_scope(&scope, &params.folder_path) {
            return "[]".to_string();
        }
        let state = self.app_handle.state::<AppState>();
        let offset = params.offset.unwrap_or(0);
        let limit = clamp_limit(params.limit.unwrap_or(50));

        match state
            .db
            .list_images_by_folder(&params.folder_path, limit, offset)
        {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images
                    .iter()
                    .filter(|img| tokens::image_in_scope(&scope, &img.path, &[]))
                    .map(|img| {
                        serde_json::json!({
                            "id": img.image.id,
                            "path": self.maybe_redact_path(&img.path),
                            "width": img.image.width,
                            "height": img.image.height,
                            "format": img.image.format,
                            "rating": img.selection.as_ref().and_then(|s| s.star_rating),
                            "decision": img.selection.as_ref().map(|s| &s.decision),
                        })
                    })
                    .collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::library_router()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn ids_params(ids: &[&str]) -> GetImagePreviewsParams {
        GetImagePreviewsParams {
            image_ids: Some(ids.iter().map(|id| id.to_string()).collect()),
            folder_path: None,
            offset: None,
            limit: None,
            size: None,
        }
    }

    fn folder_params(
        path: &str,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> GetImagePreviewsParams {
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

    #[test]
    fn resolve_reports_not_found_locally_and_unavailable_for_tokens() {
        let db = test_db();
        let tmp = tempfile::tempdir().unwrap();
        let ids = vec!["ghost".to_string()];

        let local = resolve_preview_items(&db, tmp.path(), &None, true, &ids, 256).unwrap();
        assert_eq!(local[0].status, PreviewStatus::NotFound);

        let scoped =
            resolve_preview_items(&db, tmp.path(), &folder_scope("/lib"), false, &ids, 256)
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
        let items = resolve_preview_items(&db, tmp.path(), &folder_scope("/lib"), false, &ids, 256)
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
}
