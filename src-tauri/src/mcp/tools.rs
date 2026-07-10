// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{CallToolRequestParams, CallToolResult, ServerCapabilities, ServerInfo},
    schemars,
    service::RequestContext,
    tool, tool_router, ErrorData, RoleServer, ServerHandler,
};
use tauri::{Emitter, Manager};

use super::auth::{require_capability, AuthContext};
use crate::db_core::canvas_document::CanvasDocument;
use crate::db_core::models::{Canvas, TokenScope};
use crate::services::tokens;
use crate::AppState;

fn can_expose_private_metadata(auth: &AuthContext) -> bool {
    match auth {
        AuthContext::Local => true,
        AuthContext::Authenticated(token) => token.role == tokens::ROLE_ADMIN,
        // Plugins never see private metadata regardless of grants.
        AuthContext::Plugin { .. } => false,
    }
}

fn generation_run_for_mcp(
    run: &crate::db_core::models::GenerationRun,
    auth: &AuthContext,
) -> serde_json::Value {
    if can_expose_private_metadata(auth) {
        return serde_json::to_value(run).unwrap_or(serde_json::Value::Null);
    }

    serde_json::json!({
        "id": &run.id,
        "prompt": serde_json::Value::Null,
        "negative_prompt": serde_json::Value::Null,
        "provider": &run.provider,
        "model": &run.model,
        "seed": &run.seed,
        "parent_run_id": &run.parent_run_id,
        "source_type": &run.source_type,
        "created_at": &run.created_at,
        "imported_at": &run.imported_at,
    })
}

fn library_stats_for_mcp(
    image_count: u32,
    folder_count: usize,
    collection_count: usize,
    scoped_counts: Option<(u32, usize, usize)>,
) -> serde_json::Value {
    match scoped_counts {
        Some((images, folders, collections)) => serde_json::json!({
            "image_count": images,
            "folder_count": folders,
            "collection_count": collections,
            "scope": "scoped",
        }),
        None => serde_json::json!({
            "image_count": image_count,
            "folder_count": folder_count,
            "collection_count": collection_count,
            "scope": "global",
        }),
    }
}

fn quality_count_for_mcp(global_count: u32, scoped_count: Option<u32>) -> serde_json::Value {
    match scoped_count {
        Some(count) => serde_json::json!({ "count": count, "scope": "scoped" }),
        None => serde_json::json!({ "count": global_count, "scope": "global" }),
    }
}

fn remote_safe_publish_value(mut value: serde_json::Value) -> serde_json::Value {
    redact_publish_paths(&mut value);
    value
}

fn json_response_for_mcp(value: serde_json::Value, auth: &AuthContext) -> serde_json::Value {
    if can_expose_private_metadata(auth) {
        value
    } else {
        remote_safe_publish_value(value)
    }
}

fn redact_publish_paths(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if is_publish_path_key(key) {
                    *child = serde_json::Value::String("[redacted:path]".to_string());
                } else {
                    redact_publish_paths(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_publish_paths(item);
            }
        }
        serde_json::Value::String(text) if string_contains_local_path(text) => {
            *value = serde_json::Value::String("[redacted:path]".to_string());
        }
        _ => {}
    }
}

fn is_publish_path_key(key: &str) -> bool {
    matches!(
        key,
        "capture_dir"
            | "export_dir"
            | "output_dir"
            | "output_path"
            | "site_dir"
            | "source_path"
            | "lastKnownPath"
            | "last_known_path"
            | "manifest_path"
            | "instructions_path"
            | "qr_svg_path"
            | "snapshot_png_path"
            | "snapshot_pdf_path"
    )
}

fn string_contains_local_path(value: &str) -> bool {
    value
        .split(|ch: char| {
            ch.is_whitespace() || matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',')
        })
        .any(|part| part.starts_with('/') || part.starts_with("~/"))
}

fn redact_remote_string(value: Option<String>, auth: &AuthContext) -> Option<String> {
    match value {
        Some(text) if !can_expose_private_metadata(auth) && string_contains_local_path(&text) => {
            Some("[redacted:path]".to_string())
        }
        other => other,
    }
}

fn error_for_mcp(message: &str, auth: &AuthContext) -> String {
    let safe_message = redact_remote_string(Some(message.to_string()), auth)
        .unwrap_or_else(|| message.to_string());
    format!("Error: {}", safe_message)
}

fn collection_summaries_for_mcp(
    collections: Vec<(String, String, u32)>,
    scoped_counts: Option<&std::collections::BTreeMap<String, u32>>,
) -> Vec<serde_json::Value> {
    collections
        .into_iter()
        .filter_map(|(id, name, global_count)| {
            let image_count = match scoped_counts {
                Some(counts) => *counts.get(&id)?,
                None => global_count,
            };
            Some(serde_json::json!({
                "id": id,
                "name": name,
                "image_count": image_count,
            }))
        })
        .collect()
}

fn clamp_limit(limit: u32) -> u32 {
    limit.min(100).max(1)
}

fn is_valid_rating(rating: u8) -> bool {
    rating <= 5
}

fn normalize_decision(decision: &str) -> Option<&'static str> {
    match decision {
        "accept" | "selected" => Some("accept"),
        "reject" | "rejected" => Some("reject"),
        "undecided" | "none" => Some("undecided"),
        _ => None,
    }
}

fn normalize_snapshot_selection_mode(mode: Option<&str>) -> Result<&'static str, String> {
    match mode.unwrap_or("replace") {
        "replace" => Ok("replace"),
        "add" => Ok("add"),
        "toggle" => Ok("toggle"),
        other => Err(format!(
            "Invalid selection mode '{}'. Use 'replace', 'add', or 'toggle'.",
            other
        )),
    }
}

/// Module setting key a tool is gated behind. `pub(crate)` because the plugin
/// runtime derives its `module:<key>` grant requirement from the same table.
pub(crate) fn required_module_for_tool(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "export_static_publish_package"
        | "export_static_publish_canvas"
        | "publish_clipboard_collection"
        | "serve_static_publish_package" => Some("module_static_publishing"),
        _ => None,
    }
}

fn canvas_summaries_for_mcp(
    canvases: Vec<Canvas>,
    canvas_name: Option<&str>,
) -> Vec<serde_json::Value> {
    let name_filter = canvas_name.map(str::trim).filter(|name| !name.is_empty());
    canvases
        .into_iter()
        .filter(|canvas| name_filter.map(|name| canvas.name == name).unwrap_or(true))
        .map(|canvas| {
            let item_count = crate::db_core::canvas_document::CanvasDocument::from_layout_json(
                &canvas.layout_json,
            )
            .map(|document| document.items.len())
            .unwrap_or(0);
            serde_json::json!({
                "id": canvas.id,
                "session_id": canvas.session_id,
                "name": canvas.name,
                "type": canvas.canvas_type,
                "sort_order": canvas.sort_order,
                "created_at": canvas.created_at,
                "updated_at": canvas.updated_at,
                "item_count": item_count,
            })
        })
        .collect()
}

fn canvas_layout_for_mcp(canvas: &Canvas) -> Result<serde_json::Value, String> {
    let document = CanvasDocument::from_layout_json(&canvas.layout_json)
        .map_err(|e| format!("Invalid canvas layout_json: {}", e))?;
    let layout = serde_json::to_value(document)
        .map_err(|e| format!("Failed to serialize canvas layout: {}", e))?;

    Ok(serde_json::json!({
        "id": canvas.id,
        "session_id": canvas.session_id,
        "name": canvas.name,
        "type": canvas.canvas_type,
        "sort_order": canvas.sort_order,
        "created_at": canvas.created_at,
        "updated_at": canvas.updated_at,
        "layout": layout,
    }))
}

fn canvas_layout_for_mcp_auth(
    canvas: &Canvas,
    auth: &AuthContext,
) -> Result<serde_json::Value, String> {
    let mut value = canvas_layout_for_mcp(canvas)?;
    if !can_expose_private_metadata(auth) {
        redact_publish_paths(&mut value);
    }
    Ok(value)
}

fn clipboard_monitor_status_for_mcp(
    running: bool,
    collection_id: Option<String>,
    collection_name: Option<String>,
    capture_dir: Option<std::path::PathBuf>,
    captured_count: u32,
    last_error: Option<String>,
    auth: &AuthContext,
) -> serde_json::Value {
    let capture_dir = match capture_dir {
        Some(path) if can_expose_private_metadata(auth) => {
            serde_json::Value::String(path.to_string_lossy().to_string())
        }
        Some(_) => serde_json::Value::String("[redacted:path]".to_string()),
        None => serde_json::Value::Null,
    };
    serde_json::json!({
        "running": running,
        "collection_id": collection_id,
        "collection_name": collection_name,
        "capture_dir": capture_dir,
        "captured_count": captured_count,
        "last_error": redact_remote_string(last_error, auth),
    })
}

fn model_download_response_for_mcp(
    status: &str,
    job_id: Option<String>,
    spec: &crate::db_core::embeddings::EmbeddingModelSpec,
    model_path: &std::path::Path,
    auth: &AuthContext,
) -> serde_json::Value {
    let model_path = if can_expose_private_metadata(auth) {
        model_path.to_string_lossy().to_string()
    } else {
        "[redacted:path]".to_string()
    };
    let mut value = serde_json::json!({
        "status": status,
        "model": spec.model_id,
        "model_path": model_path,
        "expected_sha256": spec.expected_sha256,
        "expected_size_bytes": spec.expected_size_bytes,
        "spdx_license": spec.spdx_license,
        "source_repo": spec.source_repo,
        "model_card_url": spec.model_card_url,
    });
    if let Some(job_id) = job_id {
        value["job_id"] = serde_json::Value::String(job_id);
    }
    value
}

#[derive(Debug, Clone)]
pub struct CullMcp {
    pub app_handle: tauri::AppHandle,
    pub auth: AuthContext,
    tool_router: ToolRouter<Self>,
}

impl CullMcp {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self::with_auth(app_handle, AuthContext::Local)
    }

    pub fn with_auth(app_handle: tauri::AppHandle, auth: AuthContext) -> Self {
        Self {
            app_handle,
            auth,
            tool_router: Self::compose_tool_router(),
        }
    }

    fn compose_tool_router() -> ToolRouter<Self> {
        ToolRouter::<Self>::new()
            + catalog::router()
            + clipboard::router()
            + collections::router()
            + curation::router()
            + export::router()
            + library::router()
            + ml::router()
            + sessions::router()
            + system::router()
            + vision::router()
    }

    fn require_local_agent_snapshot_tool(&self, tool_name: &str) -> Result<(), String> {
        if matches!(&self.auth, AuthContext::Local) {
            Ok(())
        } else {
            Err(format!(
                "{} is local-only in v1; use the local stdio MCP bridge from the Cull desktop session",
                tool_name
            ))
        }
    }

    fn validate_snapshot_image_ids_exist(
        &self,
        state: &AppState,
        image_ids: &[String],
    ) -> Result<(), String> {
        if image_ids.is_empty() {
            return Ok(());
        }
        let refs: Vec<&str> = image_ids.iter().map(String::as_str).collect();
        let found = state
            .db
            .get_images_by_ids(&refs)
            .map_err(|e| e.to_string())?;
        let found_ids: std::collections::BTreeSet<&str> =
            found.iter().map(|image| image.image.id.as_str()).collect();
        for image_id in image_ids {
            if !found_ids.contains(image_id.as_str()) {
                return Err(format!(
                    "Snapshot image '{}' is no longer in the library",
                    image_id
                ));
            }
        }
        Ok(())
    }

    fn emit_agent_snapshot_selection(
        &self,
        image_ids: Vec<String>,
        mode: &str,
        focus_first: bool,
    ) -> Result<(), String> {
        self.app_handle
            .emit(
                "agent-view-snapshot:select-images",
                serde_json::json!({
                    "image_ids": image_ids,
                    "mode": mode,
                    "focus_first": focus_first,
                }),
            )
            .map_err(|e| format!("Failed to select images in the app: {}", e))
    }

    fn token_scope(&self) -> Option<TokenScope> {
        match &self.auth {
            AuthContext::Local => None,
            AuthContext::Authenticated(token) => tokens::parse_scope(&token.scope_json),
            // Plugin grants are capability-only in v1; no folder/collection scope.
            AuthContext::Plugin { .. } => None,
        }
    }

    fn check_image_id_scope(&self, image_id: &str) -> Result<bool, String> {
        let scope = self.token_scope();
        let state = self.app_handle.state::<AppState>();
        // Shared DB-backed check loads path + collection membership so
        // collection-scoped tokens authorize per-image tools consistently.
        tokens::image_id_in_scope(&state.db, &scope, image_id)
    }

    fn is_remote(&self) -> bool {
        !can_expose_private_metadata(&self.auth)
    }

    fn maybe_redact_path(&self, path: &str) -> serde_json::Value {
        if self.is_remote() {
            serde_json::Value::String("[redacted:path]".to_string())
        } else {
            serde_json::Value::String(path.to_string())
        }
    }

    fn log_tool_call(&self, tool_name: &str, params_json: Option<&str>, status: &str) {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        let token_id = match &self.auth {
            AuthContext::Local => None,
            AuthContext::Authenticated(t) => Some(t.id.as_str()),
            AuthContext::Plugin { actor, .. } => Some(actor.as_str()),
        };
        let _ = crate::services::tokens::log_audit(&ctx, token_id, tool_name, params_json, status);
    }

    fn module_enabled(&self, module_key: &str) -> bool {
        let state = self.app_handle.state::<AppState>();
        state
            .db
            .get_setting(module_key)
            .ok()
            .flatten()
            .map(|value| value == "true")
            .unwrap_or(false)
    }

    fn tool_enabled_by_module(&self, tool_name: &str) -> bool {
        required_module_for_tool(tool_name)
            .map(|module_key| self.module_enabled(module_key))
            .unwrap_or(true)
    }

    fn require_tool_module_enabled(&self, tool_name: &str) -> Result<(), String> {
        match required_module_for_tool(tool_name) {
            Some(module_key) if !self.module_enabled(module_key) => Err(format!(
                "Tool '{}' is disabled because module '{}' is not enabled in Settings",
                tool_name, module_key
            )),
            _ => Ok(()),
        }
    }

    /// Split a token scope into the three SQL filter dimensions. Tag names are
    /// normalized to match `tags.normalized_name`. Absent dimensions map to
    /// empty vecs; an empty dimension contributes NO clause to the OR filter
    /// (it neither broadens nor matches), so scope isolation is preserved.
    fn scope_dimensions(scope: &TokenScope) -> (Vec<String>, Vec<String>, Vec<String>) {
        let folders = scope.folders.clone().unwrap_or_default();
        let collections = scope.collections.clone().unwrap_or_default();
        let tag_norms = scope
            .tags
            .as_ref()
            .map(|tags| {
                tags.iter()
                    .filter_map(|t| crate::db_core::tags::normalize_tag_name(t))
                    .collect()
            })
            .unwrap_or_default();
        (folders, collections, tag_norms)
    }

    fn scoped_images(
        &self,
        state: &AppState,
    ) -> Result<Option<Vec<crate::db_core::models::ImageWithFile>>, String> {
        let Some(scope) = self.token_scope() else {
            return Ok(None);
        };

        // SQL-level union (folder/collection/tag) with no in-memory cap; the
        // query already returns a deduped DISTINCT set ordered stably.
        let (folders, collections, tag_norms) = Self::scope_dimensions(&scope);
        let images = state
            .db
            .list_images_in_scope(&folders, &collections, &tag_norms, u32::MAX, 0)
            .map_err(|e| e.to_string())?;

        Ok(Some(images))
    }

    fn scoped_library_counts(
        &self,
        state: &AppState,
    ) -> Result<Option<(u32, usize, usize)>, String> {
        let Some(scope) = self.token_scope() else {
            return Ok(None);
        };
        let Some(images) = self.scoped_images(state)? else {
            return Ok(None);
        };

        let mut folders = std::collections::BTreeSet::new();
        for image in &images {
            if let Some(parent) = std::path::Path::new(&image.path).parent() {
                folders.insert(parent.to_string_lossy().to_string());
            }
        }

        let collections = if let Some(collection_ids) = &scope.collections {
            collection_ids.len()
        } else {
            let mut count = 0usize;
            for (collection_id, _, _) in state.db.list_collections().map_err(|e| e.to_string())? {
                let collection_images = state
                    .db
                    .list_collection_images(&collection_id)
                    .map_err(|e| e.to_string())?;
                if collection_images
                    .iter()
                    .any(|image| tokens::image_in_scope(&Some(scope.clone()), &image.path, &[]))
                {
                    count += 1;
                }
            }
            count
        };

        Ok(Some((images.len() as u32, folders.len(), collections)))
    }

    fn scoped_collection_counts(
        &self,
        state: &AppState,
    ) -> Result<Option<std::collections::BTreeMap<String, u32>>, String> {
        let Some(scope) = self.token_scope() else {
            return Ok(None);
        };
        let mut counts = std::collections::BTreeMap::new();
        let scope_ref = Some(scope.clone());

        for (collection_id, _, _) in state.db.list_collections().map_err(|e| e.to_string())? {
            let explicitly_allowed = scope
                .collections
                .as_ref()
                .map(|allowed| allowed.contains(&collection_id))
                .unwrap_or(false);
            if scope.collections.is_some() && !explicitly_allowed {
                continue;
            }

            let collection_images = state
                .db
                .list_collection_images(&collection_id)
                .map_err(|e| e.to_string())?;
            let image_count = collection_images
                .iter()
                .filter(|image| {
                    tokens::image_in_scope(&scope_ref, &image.path, &[collection_id.clone()])
                })
                .count() as u32;

            if image_count > 0 || explicitly_allowed {
                counts.insert(collection_id, image_count);
            }
        }

        Ok(Some(counts))
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EmptyParams {}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListSessionCanvasesParams {
    #[schemars(description = "Session ID whose saved canvases should be listed")]
    pub session_id: String,
    #[schemars(description = "Optional exact Canvas name filter")]
    pub canvas_name: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetCanvasLayoutParams {
    #[schemars(description = "Saved Canvas ID whose v1 layout document should be returned")]
    pub canvas_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListImagesParams {
    #[schemars(description = "Pagination offset (default 0)")]
    pub offset: Option<u32>,
    #[schemars(description = "Pagination limit, max 100 (default 50)")]
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetImageParams {
    #[schemars(description = "The image ID to retrieve")]
    pub image_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListFolderImagesParams {
    #[schemars(description = "Folder path")]
    pub folder_path: String,
    #[schemars(description = "Pagination offset (default 0)")]
    pub offset: Option<u32>,
    #[schemars(description = "Pagination limit, max 100 (default 50)")]
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetRatingParams {
    #[schemars(description = "The image ID to rate")]
    pub image_id: String,
    #[schemars(description = "Rating from 0 (unrated) to 5")]
    pub rating: u8,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetDecisionParams {
    #[schemars(description = "The image ID")]
    pub image_id: String,
    #[schemars(
        description = "Decision: 'accept', 'reject', or 'undecided'. Legacy aliases 'selected', 'rejected', and 'none' are accepted."
    )]
    pub decision: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateCollectionParams {
    #[schemars(description = "Name for the new collection")]
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddToCollectionParams {
    #[schemars(description = "Collection ID to add images to")]
    pub collection_id: String,
    #[schemars(description = "List of image IDs to add")]
    pub image_ids: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CollectionIdParams {
    #[schemars(description = "Collection ID")]
    pub collection_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct OptionalCollectionIdParams {
    #[schemars(
        description = "Optional collection ID. Defaults to the active or last Clipboard Monitor collection."
    )]
    collection_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateSmartCollectionParams {
    #[schemars(description = "Name for the smart collection")]
    pub name: String,
    #[schemars(
        description = "Natural language query like 'landscape photos rated 4+' or raw filter JSON"
    )]
    pub query: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FindSimilarParams {
    #[schemars(description = "Image ID to find similar images for")]
    pub image_id: String,
    #[schemars(description = "Number of results to return (default 10)")]
    pub limit: Option<u32>,
    #[schemars(description = "Embedding model: 'clip-vit-b32' or 'dinov2-vits14'")]
    pub model: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchByObjectParams {
    #[schemars(description = "Object class to search for, e.g. 'person', 'car', 'dog'")]
    pub class_name: String,
    #[schemars(description = "Max results (default 50)")]
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowImageParams {
    #[schemars(description = "Image ID to display in loupe view")]
    pub image_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct NavigateToFolderParams {
    #[schemars(description = "Folder path to navigate to in grid view")]
    pub folder_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CaptureCurrentViewSnapshotParams {
    #[schemars(description = "When true, also copy the annotated PNG to the local clipboard")]
    pub clipboard: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetLastViewSnapshotParams {
    #[schemars(description = "Optional snapshot ID. Defaults to the latest captured snapshot")]
    pub snapshot_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SelectSnapshotLabelsParams {
    #[schemars(description = "Optional snapshot ID. Defaults to the latest captured snapshot")]
    pub snapshot_id: Option<String>,
    #[schemars(description = "Visible labels to select, e.g. ['1','4','7']")]
    pub labels: Vec<String>,
    #[schemars(description = "Selection mode: replace, add, or toggle. Defaults to replace")]
    pub mode: Option<String>,
    #[schemars(description = "Whether to focus the first selected image. Defaults to true")]
    pub focus_first: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SelectImagesInViewParams {
    #[schemars(description = "Image IDs to select in the currently visible app view")]
    pub image_ids: Vec<String>,
    #[schemars(description = "Selection mode: replace, add, or toggle. Defaults to replace")]
    pub mode: Option<String>,
    #[schemars(description = "Whether to focus the first selected image. Defaults to true")]
    pub focus_first: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ImageIdParams {
    #[schemars(description = "Image ID")]
    pub image_id: String,
}

// --- Generation metadata param structs ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetGenerationMetadataParams {
    image_id: String,
    prompt: String,
    model: Option<String>,
    provider: Option<String>,
    seed: Option<String>,
    settings_json: Option<String>,
}

// --- Job param structs ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetJobParams {
    #[schemars(description = "Job ID to query")]
    pub job_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CancelJobParams {
    #[schemars(description = "Job ID to cancel")]
    pub job_id: String,
}

// --- AI param structs ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GenerateEmbeddingsParams {
    #[schemars(description = "List of image IDs to generate embeddings for")]
    pub image_ids: Vec<String>,
    #[schemars(description = "Embedding model: 'clip-vit-b32' or 'dinov2-vits14'")]
    pub model: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DownloadEmbeddingModelParams {
    #[schemars(description = "Embedding model: 'clip-vit-b32' or 'dinov2-vits14'")]
    pub model: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DetectObjectsParams {
    #[schemars(description = "List of image IDs to run YOLO object detection on")]
    pub image_ids: Vec<String>,
    #[schemars(description = "YOLO variant: 'nano', 'small', or 'medium' (default: 'medium')")]
    pub variant: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnalyzeImagesParams {
    #[schemars(description = "List of image IDs to analyze with Ollama vision model")]
    pub image_ids: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnalyzeImageQualityParams {
    #[schemars(description = "List of image IDs to analyze for blur, focus, and exposure")]
    pub image_ids: Vec<String>,
}

// --- Catalog param structs ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListCatalogPresetsParams {
    #[schemars(description = "Optional preset kind filter")]
    pub preset_kind: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CatalogPresetIdParams {
    #[schemars(description = "Catalog preset ID")]
    pub preset_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListCatalogFieldsParams {
    #[schemars(description = "Filter by subject_scope: image, work, or both")]
    pub subject_scope: Option<String>,
    #[schemars(description = "Whether to include deprecated field definitions")]
    pub include_deprecated: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateCatalogFieldDefParams {
    #[schemars(description = "Stable dotted key, e.g. artist_inventory.height")]
    pub stable_key: String,
    #[schemars(description = "Human-readable label")]
    pub label: String,
    #[schemars(description = "Optional field description")]
    pub description: Option<String>,
    #[schemars(description = "Allowed subject scope: image, work, or both")]
    pub subject_scope: String,
    #[schemars(
        description = "Value type: text, long_text, number, integer, money, dimension, date, boolean, enum, reference, json"
    )]
    pub value_type: String,
    #[schemars(description = "single or multi")]
    pub cardinality: String,
    #[schemars(description = "Unit kind for numeric values (optional)")]
    pub unit_kind: Option<String>,
    #[schemars(description = "Optional JSON validation rule object")]
    pub validation_json: Option<String>,
    #[schemars(description = "normal, private, or commercial")]
    pub sensitivity: String,
    #[schemars(description = "Optional source attribution pointer")]
    pub derived_source: Option<String>,
    #[schemars(description = "Optional crosswalk map to standards (optional JSON string)")]
    pub crosswalk_json: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeprecateCatalogFieldDefParams {
    #[schemars(description = "Field definition ID")]
    pub field_def_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateCatalogPresetParams {
    #[schemars(description = "Preset display name")]
    pub name: String,
    #[schemars(description = "Optional preset description")]
    pub description: Option<String>,
    #[schemars(
        description = "Preset kind: generative_art, artist_inventory, photo_dam, asset_delivery, or custom"
    )]
    pub preset_kind: String,
    #[schemars(description = "Ordered list of field definition IDs")]
    pub field_def_ids: Vec<String>,
    #[schemars(description = "Optional layout JSON")]
    pub layout_json: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateCatalogPresetParams {
    #[schemars(description = "Preset ID to update")]
    pub preset_id: String,
    #[schemars(description = "Optional new preset name")]
    pub name: Option<String>,
    #[schemars(description = "Optional new description")]
    pub description: Option<String>,
    #[schemars(description = "Optional replacement field list")]
    pub field_def_ids: Option<Vec<String>>,
    #[schemars(description = "Optional new layout JSON")]
    pub layout_json: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateCatalogWorkParams {
    #[schemars(description = "Primary image ID for the work")]
    pub primary_image_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CatalogWorkImageInput {
    #[schemars(description = "Image ID")]
    pub image_id: String,
    #[schemars(
        description = "Role: primary, alternate, detail, source, reference, rendition, or other"
    )]
    pub role: String,
    #[schemars(description = "Ordering for display")]
    pub ordinal: i64,
    #[schemars(description = "Optional edition label")]
    pub edition_label: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AttachImagesToCatalogWorkParams {
    #[schemars(description = "Work ID")]
    pub work_id: String,
    #[schemars(description = "Images to attach")]
    pub images: Vec<CatalogWorkImageInput>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListCatalogValuesParams {
    #[schemars(description = "Optional subject_type filter (image/work)")]
    pub subject_type: Option<String>,
    #[schemars(description = "Optional subject_id filter")]
    pub subject_id: Option<String>,
    #[schemars(description = "Optional status filter (draft/approved/rejected/superseded)")]
    pub status: Option<String>,
    #[schemars(description = "Optional source_type filter")]
    pub source_type: Option<String>,
    #[schemars(description = "Optional field_def_id filter")]
    pub field_def_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CatalogRecordParams {
    #[schemars(description = "subject image or work")]
    pub subject_type: String,
    #[schemars(description = "subject ID")]
    pub subject_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetCatalogDraftValueParams {
    #[schemars(description = "subject: image or work")]
    pub subject_type: String,
    #[schemars(description = "subject ID")]
    pub subject_id: String,
    #[schemars(description = "field definition ID")]
    pub field_def_id: String,
    #[schemars(description = "JSON encoded value payload")]
    pub value_json: String,
    #[schemars(description = "Display string for humans")]
    pub display_value: String,
    #[schemars(description = "optional source type")]
    pub source_type: Option<String>,
    #[schemars(description = "optional source ID")]
    pub source_id: Option<String>,
    #[schemars(description = "optional confidence score")]
    pub confidence: Option<f64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CatalogDraftValueInput {
    #[schemars(description = "subject: image or work")]
    pub subject_type: String,
    #[schemars(description = "subject ID")]
    pub subject_id: String,
    #[schemars(description = "field definition ID")]
    pub field_def_id: String,
    #[schemars(description = "JSON encoded value payload")]
    pub value_json: String,
    #[schemars(description = "Display string for humans")]
    pub display_value: String,
    #[schemars(description = "optional confidence score")]
    pub confidence: Option<f64>,
    #[schemars(description = "optional source type")]
    pub source_type: Option<String>,
    #[schemars(description = "optional source ID")]
    pub source_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetCatalogDraftValuesParams {
    #[schemars(description = "Draft values to upsert")]
    pub values: Vec<CatalogDraftValueInput>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SuggestCatalogValuesParams {
    #[schemars(description = "Draft values to upsert as agent suggestions")]
    pub values: Vec<CatalogDraftValueInput>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetCatalogSuggestionJobParams {
    #[schemars(description = "job ID")]
    pub job_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CatalogValueIdsParams {
    #[schemars(description = "catalog field value IDs")]
    pub value_ids: Vec<String>,
}

// --- Token management param structs ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateTokenParams {
    #[schemars(description = "Human-readable name for the token")]
    pub name: String,
    #[schemars(description = "Role: 'viewer', 'curator', 'operator', or 'admin'")]
    pub role: String,
    #[schemars(description = "Optional scope restriction")]
    pub scope: Option<crate::db_core::models::TokenScope>,
    #[schemars(
        description = "Optional RFC 3339 expiry timestamp. Defaults to 90 days from creation."
    )]
    pub expires_at: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TokenIdParams {
    #[schemars(description = "Token ID (e.g. tok_abc123)")]
    pub token_id: String,
}

// --- Audit param structs ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AuditLogParams {
    #[schemars(description = "Max entries to return (default 50, max 500)")]
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PruneAuditParams {
    #[schemars(description = "Delete entries older than this many days (default 30)")]
    pub retention_days: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ServeStaticPublishParams {
    #[schemars(description = "Absolute path to a generated static publishing site directory")]
    pub site_dir: String,
    #[schemars(description = "Host to bind, e.g. 127.0.0.1 or 0.0.0.0 (default 127.0.0.1)")]
    pub host: Option<String>,
    #[schemars(description = "Port to bind (default 8000)")]
    pub port: Option<u16>,
}

mod catalog;
mod clipboard;
mod collections;
mod curation;
mod export;
mod library;
mod ml;
mod sessions;
mod system;
mod vision;

impl ServerHandler for CullMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Cull MCP server — browse, curate, and manage an AI art image library",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, ErrorData> {
        let tools: Vec<_> = self
            .tool_router
            .list_all()
            .into_iter()
            .filter(|tool| self.tool_enabled_by_module(tool.name.as_ref()))
            .collect();
        crate::safe_eprintln!("MCP list_tools: returning {} tools", tools.len());
        Ok(rmcp::model::ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
        async move {
            let tool_name = request.name.to_string();
            let params_json = request
                .arguments
                .as_ref()
                .and_then(|args| serde_json::to_string(args).ok());

            if let Err(msg) = require_capability(&self.auth, &tool_name) {
                self.log_tool_call(&tool_name, None, "denied");
                return Err(ErrorData::invalid_request(msg, None));
            }

            if let Err(msg) = self.require_tool_module_enabled(&tool_name) {
                self.log_tool_call(&tool_name, None, "disabled");
                return Err(ErrorData::invalid_request(msg, None));
            }

            let call_context = ToolCallContext::new(self, request, context);
            let result = self.tool_router.call(call_context).await;

            let status = match &result {
                Err(_) => "error",
                Ok(r) => {
                    if r.is_error.unwrap_or(false) {
                        "error"
                    } else {
                        "ok"
                    }
                }
            };
            self.log_tool_call(&tool_name, params_json.as_deref(), status);

            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AuthContext;
    use crate::db_core::models::{Canvas, McpToken, TokenScope};
    use crate::services::tokens;

    #[test]
    fn test_scoped_generation_run_redacts_private_fields_by_default() {
        let auth = AuthContext::Authenticated(make_token(
            "viewer",
            Some(r#"{"folders":["/Users/gleb/art/share"]}"#.to_string()),
        ));
        let run = generation_run_fixture();

        let value = super::generation_run_for_mcp(&run, &auth);
        let json = value.to_string();

        assert_eq!(value["id"], "run-1");
        assert_eq!(value["prompt"], serde_json::Value::Null);
        assert!(value.get("source_path").is_none());
        assert!(value.get("raw_metadata_json").is_none());
        assert!(!json.contains("/Users"));
        assert!(!json.contains("private prompt"));
        assert!(!json.contains("source_path"));
        assert!(!json.contains("raw_metadata_json"));
    }

    #[test]
    fn test_local_generation_run_preserves_private_fields() {
        let run = generation_run_fixture();

        let value = super::generation_run_for_mcp(&run, &AuthContext::Local);

        assert_eq!(value["prompt"], "private prompt");
        assert_eq!(value["source_path"], "/Users/gleb/art/share/image.json");
        assert_eq!(value["raw_metadata_json"], r#"{"prompt":"private prompt"}"#);
    }

    #[test]
    fn test_scoped_library_stats_do_not_return_global_counts() {
        let stats = super::library_stats_for_mcp(12, 4, 3, Some((2, 1, 1)));

        assert_eq!(stats["image_count"], 2);
        assert_eq!(stats["folder_count"], 1);
        assert_eq!(stats["collection_count"], 1);
        assert_eq!(stats["scope"], "scoped");
        assert_ne!(stats["image_count"], 12);
    }

    #[test]
    fn test_scoped_quality_count_does_not_return_global_count() {
        let count = super::quality_count_for_mcp(9, Some(2));

        assert_eq!(count["count"], 2);
        assert_eq!(count["scope"], "scoped");
        assert_ne!(count["count"], 9);
    }

    #[test]
    fn test_remote_clipboard_publish_value_redacts_local_paths() {
        let value = serde_json::json!({
            "collection_id": "col-1",
            "image_count": 2,
            "site_dir": "/Users/gleb/Library/Application Support/com.glebkalinin.cull/static-publishing/site",
            "url": "http://127.0.0.1:8000",
            "manifest_path": "/Users/gleb/Library/Application Support/com.glebkalinin.cull/static-publishing/site/data/manifest.json",
            "instructions_path": "/Users/gleb/Library/Application Support/com.glebkalinin.cull/static-publishing/site/CLAUDE.md",
        });

        let redacted = super::remote_safe_publish_value(value);
        let json = redacted.to_string();

        assert_eq!(redacted["site_dir"], "[redacted:path]");
        assert_eq!(redacted["manifest_path"], "[redacted:path]");
        assert_eq!(redacted["instructions_path"], "[redacted:path]");
        assert!(!json.contains("/Users"));
        assert!(!json.contains("manifest.json"));
        assert!(!json.contains("CLAUDE.md"));
    }

    #[test]
    fn test_folder_scoped_collection_summaries_filter_and_scope_counts() {
        let collections = vec![
            ("col-a".to_string(), "In scope".to_string(), 12),
            ("col-b".to_string(), "Out of scope".to_string(), 8),
        ];
        let scoped_counts = std::collections::BTreeMap::from([("col-a".to_string(), 2u32)]);

        let result = super::collection_summaries_for_mcp(collections, Some(&scoped_counts));

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["id"], "col-a");
        assert_eq!(result[0]["name"], "In scope");
        assert_eq!(result[0]["image_count"], 2);
    }

    #[test]
    fn test_remote_canvas_layout_redacts_last_known_path() {
        let mut canvas = canvas_fixture("canvas-path", "Board", 0);
        canvas.layout_json = r#"{
            "version": 1,
            "viewport": { "panX": 0, "panY": 0, "zoom": 1 },
            "items": [{
                "id": "item-a",
                "imageId": "img-a",
                "x": 0,
                "y": 0,
                "width": 200,
                "height": 120,
                "z": 0,
                "hidden": false,
                "label": null,
                "groupId": null,
                "transform": { "crop": null, "rotationDegrees": 0, "fit": "contain" },
                "source": {
                    "contentHash": "hash-a",
                    "lastKnownPath": "/Users/gleb/art/private/full-file-name.png"
                }
            }],
            "groups": [],
            "connectors": [],
            "annotations": [],
            "export": { "defaultPresetId": null, "background": "transparent", "bounds": "content" }
        }"#
        .to_string();
        let auth = AuthContext::Authenticated(make_token("viewer", None));

        let result = super::canvas_layout_for_mcp_auth(&canvas, &auth).unwrap();
        let json = result.to_string();

        assert_eq!(
            result["layout"]["items"][0]["source"]["lastKnownPath"],
            "[redacted:path]"
        );
        assert!(!json.contains("/Users"));
        assert!(!json.contains("full-file-name.png"));
    }

    #[test]
    fn test_remote_safe_publish_value_redacts_path_bearing_strings() {
        let value = serde_json::json!({
            "warnings": [
                "Failed to read /Users/gleb/Library/Application Support/com.glebkalinin.cull/site/manifest.json"
            ],
            "nested": {
                "message": "Wrote /tmp/cull/private/output.png"
            }
        });

        let redacted = super::remote_safe_publish_value(value);
        let json = redacted.to_string();

        assert_eq!(redacted["warnings"][0], "[redacted:path]");
        assert_eq!(redacted["nested"]["message"], "[redacted:path]");
        assert!(!json.contains("/Users"));
        assert!(!json.contains("/tmp/cull"));
        assert!(!json.contains("manifest.json"));
        assert!(!json.contains("output.png"));
    }

    #[test]
    fn test_remote_json_response_redacts_import_error_paths() {
        let auth = AuthContext::Authenticated(make_token("operator", None));
        let value = serde_json::json!({
            "imported": 0,
            "skipped": 1,
            "errors": [
                "/Users/gleb/art/private/broken.png: Unsupported image format"
            ],
            "batch_id": null,
            "image_ids": [],
        });

        let redacted = super::json_response_for_mcp(value.clone(), &auth);

        assert_eq!(redacted["errors"][0], "[redacted:path]");
        assert!(!redacted.to_string().contains("/Users"));
        assert!(!redacted.to_string().contains("broken.png"));

        let local = super::json_response_for_mcp(value, &AuthContext::Local);
        assert!(local
            .to_string()
            .contains("/Users/gleb/art/private/broken.png"));
    }

    #[test]
    fn test_remote_error_message_redacts_path_bearing_text() {
        let auth = AuthContext::Authenticated(make_token("viewer", None));

        let message = super::error_for_mcp(
            "Failed to write /Users/gleb/Library/Application Support/com.glebkalinin.cull/site/index.html",
            &auth,
        );

        assert_eq!(message, "Error: [redacted:path]");
        assert!(!message.contains("/Users"));
        assert!(!message.contains("index.html"));
    }

    #[test]
    fn test_remote_clipboard_status_redacts_last_error_paths() {
        let auth = AuthContext::Authenticated(make_token("viewer", None));

        let value = super::clipboard_monitor_status_for_mcp(
            true,
            Some("col-1".to_string()),
            Some("Clipboard".to_string()),
            Some(std::path::PathBuf::from(
                "/Users/gleb/Library/Application Support/com.glebkalinin.cull/clipboard",
            )),
            3,
            Some("Failed to copy /Users/gleb/Desktop/private.png".to_string()),
            &auth,
        );
        let json = value.to_string();

        assert_eq!(value["capture_dir"], "[redacted:path]");
        assert_eq!(value["last_error"], "[redacted:path]");
        assert!(!json.contains("/Users"));
        assert!(!json.contains("private.png"));
    }

    #[test]
    fn test_remote_model_download_response_redacts_model_path() {
        let auth = AuthContext::Authenticated(make_token("operator", None));
        let spec = crate::db_core::embeddings::CLIP_MODEL_SPEC;

        let value = super::model_download_response_for_mcp(
            "started",
            Some("job-1".to_string()),
            &spec,
            std::path::Path::new("/Users/gleb/Library/Application Support/com.glebkalinin.cull/models/clip-vit-b32.onnx"),
            &auth,
        );
        let json = value.to_string();

        assert_eq!(value["model_path"], "[redacted:path]");
        assert!(!json.contains("/Users"));
        assert!(!json.contains("clip-vit-b32.onnx"));
    }

    // --- is_remote logic ---

    #[test]
    fn test_local_is_not_remote() {
        assert!(!matches!(AuthContext::Local, AuthContext::Authenticated(_)));
    }

    #[test]
    fn test_authenticated_is_remote() {
        let token = make_token("viewer", None);
        let auth = AuthContext::Authenticated(token);
        assert!(matches!(auth, AuthContext::Authenticated(_)));
    }

    // --- Scope filtering on images ---

    #[test]
    fn test_scope_none_allows_all_images() {
        assert!(tokens::image_in_scope(&None, "/any/path.jpg", &[]));
        assert!(tokens::image_in_scope(&None, "", &[]));
    }

    #[test]
    fn test_scope_single_folder() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art/midjourney".to_string()]),
            collections: None,
            tags: None,
        });
        assert!(tokens::image_in_scope(
            &scope,
            "/art/midjourney/img1.png",
            &[]
        ));
        assert!(tokens::image_in_scope(
            &scope,
            "/art/midjourney/sub/img2.png",
            &[]
        ));
        assert!(!tokens::image_in_scope(&scope, "/art/dalle/img3.png", &[]));
        assert!(!tokens::image_in_scope(&scope, "/photos/vacation.jpg", &[]));
    }

    #[test]
    fn test_scope_multiple_folders() {
        let scope = Some(TokenScope {
            folders: Some(vec![
                "/art/midjourney".to_string(),
                "/art/dalle".to_string(),
            ]),
            collections: None,
            tags: None,
        });
        assert!(tokens::image_in_scope(
            &scope,
            "/art/midjourney/img.png",
            &[]
        ));
        assert!(tokens::image_in_scope(&scope, "/art/dalle/img.png", &[]));
        assert!(!tokens::image_in_scope(&scope, "/art/stable/img.png", &[]));
    }

    #[test]
    fn test_scope_path_traversal_blocked() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art".to_string()]),
            collections: None,
            tags: None,
        });
        assert!(tokens::image_in_scope(&scope, "/art/image.jpg", &[]));
        assert!(!tokens::image_in_scope(&scope, "/artifacts/image.jpg", &[]));
        assert!(!tokens::image_in_scope(&scope, "/artisan/image.jpg", &[]));
    }

    #[test]
    fn test_scope_collection_match() {
        let scope = Some(TokenScope {
            folders: None,
            collections: Some(vec!["col_abc".to_string(), "col_def".to_string()]),
            tags: None,
        });
        assert!(tokens::image_in_scope(
            &scope,
            "/any/path.jpg",
            &["col_abc".to_string()]
        ));
        assert!(tokens::image_in_scope(
            &scope,
            "/any/path.jpg",
            &["col_def".to_string()]
        ));
        assert!(!tokens::image_in_scope(
            &scope,
            "/any/path.jpg",
            &["col_xyz".to_string()]
        ));
        assert!(!tokens::image_in_scope(&scope, "/any/path.jpg", &[]));
    }

    #[test]
    fn test_scope_union_folder_or_collection() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art".to_string()]),
            collections: Some(vec!["col_abc".to_string()]),
            tags: None,
        });
        // Folder match alone
        assert!(tokens::image_in_scope(&scope, "/art/img.png", &[]));
        // Collection match alone
        assert!(tokens::image_in_scope(
            &scope,
            "/photos/img.png",
            &["col_abc".to_string()]
        ));
        // Neither
        assert!(!tokens::image_in_scope(&scope, "/photos/img.png", &[]));
    }

    // --- Folder scope filtering ---

    #[test]
    fn test_folder_scope_none_allows_all() {
        assert!(tokens::folder_in_scope(&None, "/any/folder"));
    }

    #[test]
    fn test_folder_scope_match() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art".to_string()]),
            collections: None,
            tags: None,
        });
        assert!(tokens::folder_in_scope(&scope, "/art"));
        assert!(tokens::folder_in_scope(&scope, "/art/sub"));
        assert!(!tokens::folder_in_scope(&scope, "/photos"));
    }

    #[test]
    fn test_folder_scope_traversal_blocked() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art".to_string()]),
            collections: None,
            tags: None,
        });
        assert!(!tokens::folder_in_scope(&scope, "/artifacts"));
        assert!(!tokens::folder_in_scope(&scope, "/artisan"));
    }

    // --- Input validation (tests production helpers) ---

    #[test]
    fn test_rating_valid_range() {
        for r in 0..=5u8 {
            assert!(super::is_valid_rating(r), "Rating {} should be valid", r);
        }
    }

    #[test]
    fn test_rating_invalid() {
        assert!(!super::is_valid_rating(6));
        assert!(!super::is_valid_rating(255));
    }

    #[test]
    fn test_decision_valid_values() {
        for value in [
            "accept",
            "reject",
            "undecided",
            "selected",
            "rejected",
            "none",
        ] {
            assert!(
                super::normalize_decision(value).is_some(),
                "Decision '{}' should be valid",
                value
            );
        }
    }

    #[test]
    fn test_decision_invalid_values() {
        assert!(super::normalize_decision("maybe").is_none());
        assert!(super::normalize_decision("").is_none());
        assert!(super::normalize_decision("SELECTED").is_none());
    }

    #[test]
    fn test_decision_values_normalize_to_database_values() {
        assert_eq!(super::normalize_decision("accept"), Some("accept"));
        assert_eq!(super::normalize_decision("selected"), Some("accept"));
        assert_eq!(super::normalize_decision("reject"), Some("reject"));
        assert_eq!(super::normalize_decision("rejected"), Some("reject"));
        assert_eq!(super::normalize_decision("undecided"), Some("undecided"));
        assert_eq!(super::normalize_decision("none"), Some("undecided"));
        assert_eq!(super::normalize_decision("maybe"), None);
    }

    // --- Pagination clamping (tests production `clamp_limit`) ---

    #[test]
    fn test_limit_clamped_to_range() {
        assert_eq!(super::clamp_limit(0), 1);
        assert_eq!(super::clamp_limit(1), 1);
        assert_eq!(super::clamp_limit(50), 50);
        assert_eq!(super::clamp_limit(100), 100);
        assert_eq!(super::clamp_limit(200), 100);
        assert_eq!(super::clamp_limit(u32::MAX), 100);
    }

    // --- Collection scope (these test the same scope helpers used in production) ---

    #[test]
    fn test_collection_scope_restricts_access() {
        let scope = Some(TokenScope {
            folders: None,
            collections: Some(vec!["col_abc".to_string()]),
            tags: None,
        });
        // image_in_scope with collection membership
        assert!(tokens::image_in_scope(
            &scope,
            "/any.jpg",
            &["col_abc".to_string()]
        ));
        assert!(!tokens::image_in_scope(
            &scope,
            "/any.jpg",
            &["col_def".to_string()]
        ));
        assert!(!tokens::image_in_scope(
            &scope,
            "/any.jpg",
            &["col_ghi".to_string()]
        ));
    }

    #[test]
    fn test_no_collection_scope_allows_all() {
        let scope: Option<TokenScope> = None;
        assert!(tokens::image_in_scope(
            &scope,
            "/any.jpg",
            &["col_abc".to_string()]
        ));
        assert!(tokens::image_in_scope(&scope, "/any.jpg", &[]));
    }

    // --- Job snapshot serialization ---

    #[test]
    fn test_job_snapshot_serializes_correctly() {
        let snapshot = crate::services::jobs::JobSnapshot {
            job_id: "job_abc123def4".to_string(),
            kind: "import".to_string(),
            status: "running".to_string(),
            current: 5,
            total: 10,
            message: Some("processing image_xyz".to_string()),
            error: None,
            created_at: "2026-05-10T00:00:00Z".to_string(),
            updated_at: "2026-05-10T00:01:00Z".to_string(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"job_id\":\"job_abc123def4\""));
        assert!(json.contains("\"status\":\"running\""));
        assert!(json.contains("\"current\":5"));
        assert!(json.contains("\"total\":10"));
        assert!(json.contains("\"kind\":\"import\""));
    }

    #[test]
    fn test_job_snapshot_null_optional_fields() {
        let snapshot = crate::services::jobs::JobSnapshot {
            job_id: "job_test".to_string(),
            kind: "vision".to_string(),
            status: "completed".to_string(),
            current: 10,
            total: 10,
            message: None,
            error: None,
            created_at: "2026-05-10T00:00:00Z".to_string(),
            updated_at: "2026-05-10T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"message\":null"));
        assert!(json.contains("\"error\":null"));
    }

    #[test]
    fn test_job_snapshot_with_error() {
        let snapshot = crate::services::jobs::JobSnapshot {
            job_id: "job_fail".to_string(),
            kind: "embeddings".to_string(),
            status: "failed".to_string(),
            current: 3,
            total: 10,
            message: None,
            error: Some("Model not downloaded".to_string()),
            created_at: "2026-05-10T00:00:00Z".to_string(),
            updated_at: "2026-05-10T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"error\":\"Model not downloaded\""));
        assert!(json.contains("\"status\":\"failed\""));
    }

    #[test]
    fn test_list_session_canvases_filters_by_exact_name() {
        let canvases = vec![
            canvas_fixture("canvas-a", "Board", 0),
            canvas_fixture("canvas-b", "References", 1),
            canvas_fixture("canvas-c", "Board", 2),
        ];

        let result = super::canvas_summaries_for_mcp(canvases, Some("Board"));
        let ids: Vec<&str> = result
            .iter()
            .filter_map(|value| value["id"].as_str())
            .collect();

        assert_eq!(ids, vec!["canvas-a", "canvas-c"]);
        assert_eq!(result[0]["item_count"], 0);
        assert!(result[0].get("layout_json").is_none());
    }

    #[test]
    fn test_canvas_layout_for_mcp_exposes_transforms_and_annotations() {
        let mut canvas = canvas_fixture("canvas-a", "Board", 0);
        canvas.layout_json = r#"{
            "version": 1,
            "viewport": { "panX": 0, "panY": 0, "zoom": 1 },
            "items": [{
                "id": "item-a",
                "imageId": "img-a",
                "x": 0,
                "y": 0,
                "width": 200,
                "height": 120,
                "z": 0,
                "hidden": false,
                "label": null,
                "groupId": null,
                "transform": {
                    "crop": { "x": 0.25, "y": 0.2, "width": 0.5, "height": 0.6 },
                    "rotationDegrees": 90,
                    "fit": "contain"
                },
                "source": { "contentHash": "hash-a", "lastKnownPath": "/library/a.png" }
            }],
            "groups": [],
            "connectors": [],
            "annotations": [{
                "id": "note-a",
                "target": { "type": "item", "itemId": "item-a" },
                "body": "Use this crop",
                "x": 0.5,
                "y": 0.5,
                "createdAt": "2026-05-16T10:00:00Z",
                "author": null
            }],
            "export": { "defaultPresetId": null, "background": "transparent", "bounds": "content" }
        }"#
        .to_string();

        let result = super::canvas_layout_for_mcp(&canvas).unwrap();

        assert_eq!(result["id"], "canvas-a");
        assert_eq!(
            result["layout"]["items"][0]["transform"]["rotationDegrees"],
            90.0
        );
        assert_eq!(result["layout"]["items"][0]["transform"]["crop"]["x"], 0.25);
        assert_eq!(result["layout"]["annotations"][0]["body"], "Use this crop");
    }

    // --- Tool capability mapping completeness ---

    #[test]
    fn test_read_tools_map_to_library_read() {
        let read_tools = [
            "list_images",
            "get_image",
            "list_folders",
            "list_folder_images",
            "list_collections",
            "list_collection_images",
            "list_session_canvases",
            "get_canvas_layout",
            "get_library_stats",
            "get_detections",
            "get_vision_metadata",
            "get_image_quality",
            "get_quality_count",
        ];
        for tool in &read_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                "library:read",
                "Tool '{}' should map to library:read",
                tool
            );
        }
    }

    #[test]
    fn test_search_tools_map_to_library_search() {
        let search_tools = ["find_similar", "search_by_object", "search_images"];
        for tool in &search_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                "library:search",
                "Tool '{}' should map to library:search",
                tool
            );
        }
    }

    #[test]
    fn test_curation_tools_map_to_curation_write() {
        let curation_tools = [
            "set_rating",
            "set_decision",
            "create_collection",
            "add_to_collection",
            "delete_collection",
            "create_smart_collection",
        ];
        for tool in &curation_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                "curation:write",
                "Tool '{}' should map to curation:write",
                tool
            );
        }
    }

    #[test]
    fn test_import_tools_map_to_import_write() {
        assert_eq!(tokens::tool_capability("import_folder"), "import:write");
        assert_eq!(tokens::tool_capability("import_files"), "import:write");
    }

    #[test]
    fn test_export_tools_map_to_export_read() {
        let export_tools = [
            "list_export_presets",
            "export_static_publish_package",
            "export_static_publish_canvas",
        ];
        for tool in &export_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                "export:read",
                "Tool '{}' should map to export:read",
                tool
            );
        }
    }

    #[test]
    fn test_catalog_tools_map_to_catalog_capabilities() {
        let read_tools = [
            "list_catalog_presets",
            "get_catalog_preset",
            "list_catalog_fields",
            "get_catalog_record",
            "list_catalog_values",
            "list_catalog_drafts",
            "get_catalog_suggestion_job",
        ];
        for tool in &read_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                tokens::CAP_CATALOG_READ,
                "Tool '{}' should map to catalog:read",
                tool
            );
        }

        let write_tools = [
            "create_catalog_work",
            "attach_images_to_catalog_work",
            "set_catalog_draft_value",
            "set_catalog_draft_values",
            "suggest_catalog_values",
        ];
        for tool in &write_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                tokens::CAP_CATALOG_WRITE,
                "Tool '{}' should map to catalog:write",
                tool
            );
        }

        for tool in ["approve_catalog_values", "reject_catalog_values"] {
            assert_eq!(
                tokens::tool_capability(tool),
                tokens::CAP_CATALOG_APPROVE,
                "Tool '{}' should map to catalog:approve",
                tool
            );
        }

        let admin_tools = [
            "create_catalog_field_def",
            "deprecate_catalog_field_def",
            "create_catalog_preset",
            "update_catalog_preset",
        ];
        for tool in &admin_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                tokens::CAP_CATALOG_ADMIN,
                "Tool '{}' should map to catalog:admin",
                tool
            );
        }
    }

    fn canvas_fixture(id: &str, name: &str, sort_order: i32) -> Canvas {
        Canvas {
            id: id.to_string(),
            session_id: "session-1".to_string(),
            name: name.to_string(),
            canvas_type: "manual".to_string(),
            layout_json: "{}".to_string(),
            filter_json: None,
            grid_config_json: None,
            sort_order,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            updated_at: "2026-05-15T01:00:00Z".to_string(),
        }
    }

    fn generation_run_fixture() -> crate::db_core::models::GenerationRun {
        crate::db_core::models::GenerationRun {
            id: "run-1".to_string(),
            prompt: Some("private prompt".to_string()),
            negative_prompt: Some("private negative".to_string()),
            provider: Some("openai".to_string()),
            model: Some("gpt-image-1".to_string()),
            settings_json: r#"{"quality":"high"}"#.to_string(),
            seed: Some("123".to_string()),
            parent_run_id: None,
            source_type: "sidecar".to_string(),
            source_path: Some("/Users/gleb/art/share/image.json".to_string()),
            raw_metadata_json: Some(r#"{"prompt":"private prompt"}"#.to_string()),
            created_at: Some("2026-05-30T12:00:00Z".to_string()),
            imported_at: "2026-05-30T12:01:00Z".to_string(),
        }
    }

    #[test]
    fn test_display_tools_map_to_display_navigate() {
        let display_tools = ["show_image", "navigate_to_folder", "show_collection"];
        for tool in &display_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                "display:navigate",
                "Tool '{}' should map to display:navigate",
                tool
            );
        }
    }

    #[test]
    fn test_ai_tools_map_to_ai_run() {
        let ai_tools = [
            "download_embedding_model",
            "generate_embeddings",
            "analyze_image_quality",
            "detect_objects",
            "analyze_images",
        ];
        for tool in &ai_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                "ai:run",
                "Tool '{}' should map to ai:run",
                tool
            );
        }
    }

    #[test]
    fn test_mcp_model_download_response_includes_provenance() {
        let spec = crate::db_core::embeddings::CLIP_MODEL_SPEC;
        let auth = AuthContext::Local;
        let response = super::model_download_response_for_mcp(
            "started",
            Some("job-1".to_string()),
            &spec,
            std::path::Path::new("/tmp/clip-vit-b32-vision.onnx"),
            &auth,
        );

        assert_eq!(response["status"], "started");
        assert_eq!(response["job_id"], "job-1");
        assert_eq!(response["model"], spec.model_id);
        assert_eq!(response["expected_sha256"], spec.expected_sha256);
        assert_eq!(response["expected_size_bytes"], spec.expected_size_bytes);
        assert_eq!(response["spdx_license"], spec.spdx_license);
        assert_eq!(response["source_repo"], spec.source_repo);
        assert_eq!(response["model_card_url"], spec.model_card_url);
    }

    #[test]
    fn test_token_tools_map_to_tokens_manage() {
        let token_tools = [
            "create_token",
            "list_tokens",
            "revoke_token",
            "rotate_token",
            "get_audit_log",
            "prune_audit_log",
        ];
        for tool in &token_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                "tokens:manage",
                "Tool '{}' should map to tokens:manage",
                tool
            );
        }
    }

    #[test]
    fn test_admin_only_tools_map_to_settings_manage() {
        let admin_tools = [
            "rescan_sources",
            "get_job",
            "list_jobs",
            "cancel_job",
            "serve_static_publish_package",
        ];
        for tool in &admin_tools {
            assert_eq!(
                tokens::tool_capability(tool),
                "settings:manage",
                "Tool '{}' should map to settings:manage",
                tool
            );
        }
    }

    #[test]
    fn test_unknown_tool_maps_to_settings_manage() {
        assert_eq!(
            tokens::tool_capability("nonexistent_tool"),
            "settings:manage"
        );
        assert_eq!(tokens::tool_capability(""), "settings:manage");
        assert_eq!(tokens::tool_capability("drop_database"), "settings:manage");
    }

    #[test]
    fn test_static_publish_tools_are_module_gated() {
        assert_eq!(
            super::required_module_for_tool("export_static_publish_package"),
            Some("module_static_publishing")
        );
        assert_eq!(
            super::required_module_for_tool("export_static_publish_canvas"),
            Some("module_static_publishing")
        );
        assert_eq!(
            super::required_module_for_tool("serve_static_publish_package"),
            Some("module_static_publishing")
        );
        assert_eq!(super::required_module_for_tool("list_images"), None);
    }

    #[test]
    fn test_clipboard_monitor_tools_map_to_expected_capabilities() {
        assert_eq!(
            tokens::tool_capability("get_clipboard_monitor_status"),
            "library:read"
        );
        assert_eq!(
            tokens::tool_capability("get_last_clipboard_publish"),
            "library:read"
        );
        assert_eq!(
            tokens::tool_capability("show_clipboard_collection"),
            "display:navigate"
        );
        assert_eq!(
            tokens::tool_capability("publish_clipboard_collection"),
            "export:read"
        );
    }

    #[test]
    fn test_clipboard_publish_tool_is_module_gated() {
        assert_eq!(
            super::required_module_for_tool("publish_clipboard_collection"),
            Some("module_static_publishing")
        );
    }

    #[test]
    fn test_agent_snapshot_selection_mode_validation() {
        assert_eq!(
            super::normalize_snapshot_selection_mode(None).unwrap(),
            "replace"
        );
        assert_eq!(
            super::normalize_snapshot_selection_mode(Some("add")).unwrap(),
            "add"
        );
        assert_eq!(
            super::normalize_snapshot_selection_mode(Some("toggle")).unwrap(),
            "toggle"
        );
        assert!(super::normalize_snapshot_selection_mode(Some("append")).is_err());
    }

    // --- Auth + scope integration ---

    #[test]
    fn test_token_scope_parsed_from_json() {
        let scope_json = Some(r#"{"folders":["/art"],"collections":["col_1"]}"#.to_string());
        let scope = tokens::parse_scope(&scope_json);
        assert!(scope.is_some());
        let s = scope.unwrap();
        assert_eq!(s.folders.as_ref().unwrap(), &vec!["/art".to_string()]);
        assert_eq!(s.collections.as_ref().unwrap(), &vec!["col_1".to_string()]);
    }

    #[test]
    fn test_token_scope_none_when_no_json() {
        let scope = tokens::parse_scope(&None);
        assert!(scope.is_none());
    }

    #[test]
    fn test_token_scope_none_on_invalid_json() {
        let scope = tokens::parse_scope(&Some("not json".to_string()));
        assert!(scope.is_none());
    }

    #[test]
    fn test_token_scope_empty_object() {
        let scope = tokens::parse_scope(&Some("{}".to_string()));
        assert!(scope.is_some());
        let s = scope.unwrap();
        assert!(s.folders.is_none());
        assert!(s.collections.is_none());
        assert!(s.tags.is_none());
    }

    // --- Helper ---

    fn make_token(role: &str, scope_json: Option<String>) -> McpToken {
        McpToken {
            id: format!("tok_{}", role),
            name: format!("{} test token", role),
            role: role.to_string(),
            scope_json,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            expires_at: None,
            last_used_at: None,
            revoked: false,
        }
    }
}
