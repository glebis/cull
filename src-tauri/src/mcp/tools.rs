// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
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

fn redact_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "[redacted]".to_string())
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

fn is_valid_decision(decision: &str) -> bool {
    normalize_decision(decision).is_some()
}

fn required_module_for_tool(tool_name: &str) -> Option<&'static str> {
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
            tool_router: Self::tool_router(),
        }
    }

    fn token_scope(&self) -> Option<TokenScope> {
        match &self.auth {
            AuthContext::Local => None,
            AuthContext::Authenticated(token) => tokens::parse_scope(&token.scope_json),
        }
    }

    fn check_image_scope(&self, image_path: &str) -> bool {
        let scope = self.token_scope();
        tokens::image_in_scope(&scope, image_path, &[])
    }

    #[allow(dead_code)]
    fn filter_images_by_scope(
        &self,
        images: Vec<serde_json::Value>,
        paths: &[String],
    ) -> Vec<serde_json::Value> {
        let scope = self.token_scope();
        if scope.is_none() {
            return images;
        }
        images
            .into_iter()
            .zip(paths.iter())
            .filter(|(_, path)| tokens::image_in_scope(&scope, path, &[]))
            .map(|(img, _)| img)
            .collect()
    }

    fn check_image_id_scope(&self, image_id: &str) -> Result<bool, String> {
        let scope = self.token_scope();
        if scope.is_none() {
            return Ok(true);
        }
        let state = self.app_handle.state::<AppState>();
        let id_refs = vec![image_id];
        match state.db.get_images_by_ids(&id_refs) {
            Ok(images) => match images.first() {
                Some(img) => Ok(tokens::image_in_scope(&scope, &img.path, &[])),
                None => Ok(false),
            },
            Err(e) => Err(e.to_string()),
        }
    }

    fn is_remote(&self) -> bool {
        !matches!(self.auth, AuthContext::Local)
    }

    fn maybe_redact_path(&self, path: &str) -> serde_json::Value {
        if self.is_remote() {
            serde_json::Value::String(redact_path(path))
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

// --- Token management param structs ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateTokenParams {
    #[schemars(description = "Human-readable name for the token")]
    pub name: String,
    #[schemars(description = "Role: 'viewer', 'curator', 'operator', or 'admin'")]
    pub role: String,
    #[schemars(description = "Optional scope restriction")]
    pub scope: Option<crate::db_core::models::TokenScope>,
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

#[tool_router]
impl CullMcp {
    #[tool(description = "Get library statistics: image count, folder count, collection count")]
    fn get_library_stats(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let image_count = state.db.image_count().unwrap_or(0);
        let folders = state.db.list_folders().unwrap_or_default();
        let collections = state.db.list_collections().unwrap_or_default();

        serde_json::json!({
            "image_count": image_count,
            "folder_count": folders.len(),
            "collection_count": collections.len(),
        })
        .to_string()
    }

    #[tool(
        description = "List images with pagination. Returns id, path, dimensions, format, rating, decision."
    )]
    fn list_images(&self, Parameters(params): Parameters<ListImagesParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let offset = params.offset.unwrap_or(0);
        let limit = clamp_limit(params.limit.unwrap_or(50));
        let scope = self.token_scope();
        let fetch_limit = if scope.is_some() { limit * 3 } else { limit };

        match state.db.list_images(fetch_limit, offset) {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images
                    .iter()
                    .filter(|img| tokens::image_in_scope(&scope, &img.path, &[]))
                    .take(limit as usize)
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
                    if !self.check_image_scope(&img.path) {
                        return format!("Error: Image '{}' not found", params.image_id);
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

    #[tool(description = "List all collections with image counts")]
    fn list_collections(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let scope = self.token_scope();
        match state.db.list_collections() {
            Ok(collections) => {
                let result: Vec<serde_json::Value> = collections.iter()
                    .filter(|(id, _, _)| {
                        match &scope {
                            None => true,
                            Some(s) => match &s.collections {
                                None => true,
                                Some(allowed) => allowed.contains(id),
                            }
                        }
                    })
                    .map(|(id, name, count)| {
                        serde_json::json!({"id": id, "name": name, "image_count": count})
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

    // --- Curation tools ---

    #[tool(description = "Rate an image from 0 (unrated) to 5 stars")]
    fn set_rating(&self, Parameters(params): Parameters<SetRatingParams>) -> String {
        if !is_valid_rating(params.rating) {
            return "Error: Rating must be 0-5".to_string();
        }
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.set_rating(&params.image_id, params.rating) {
            Ok(()) => serde_json::json!({"status": "ok", "image_id": params.image_id, "rating": params.rating}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Set selection decision on an image: 'accept', 'reject', or 'undecided'. Legacy aliases 'selected', 'rejected', and 'none' are accepted."
    )]
    fn set_decision(&self, Parameters(params): Parameters<SetDecisionParams>) -> String {
        let Some(decision) = normalize_decision(&params.decision) else {
            return "Error: Decision must be 'accept', 'reject', or 'undecided'".to_string();
        };
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.set_decision(&params.image_id, decision) {
            Ok(()) => serde_json::json!({"status": "ok", "image_id": params.image_id, "decision": decision}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Create a new manual collection and return its ID")]
    fn create_collection(&self, Parameters(params): Parameters<CreateCollectionParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.create_collection(&params.name) {
            Ok(id) => serde_json::json!({"collection_id": id, "name": params.name}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Add images to an existing collection")]
    fn add_to_collection(&self, Parameters(params): Parameters<AddToCollectionParams>) -> String {
        // Check target collection is in scope
        if let Some(ref scope) = self.token_scope() {
            if let Some(ref allowed) = scope.collections {
                if !allowed.contains(&params.collection_id) {
                    return "Error: Access denied — collection outside token scope".to_string();
                }
            } else {
                // Token has no collection scope — deny collection mutations
                return "Error: Access denied — collection outside token scope".to_string();
            }
        }
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => {
                    return format!(
                        "Error: Access denied — image '{}' outside token scope",
                        image_id
                    )
                }
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();
        let refs: Vec<&str> = params.image_ids.iter().map(|s| s.as_str()).collect();
        match state.db.add_to_collection(&params.collection_id, &refs) {
            Ok(()) => {
                serde_json::json!({"status": "ok", "added": params.image_ids.len()}).to_string()
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Delete a collection (does not delete the images)")]
    fn delete_collection(&self, Parameters(params): Parameters<CollectionIdParams>) -> String {
        if let Some(ref scope) = self.token_scope() {
            let allowed = scope
                .collections
                .as_ref()
                .map(|c| c.contains(&params.collection_id))
                .unwrap_or(false);
            if !allowed {
                return "Error: Access denied — collection outside token scope".to_string();
            }
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.delete_collection(&params.collection_id) {
            Ok(()) => serde_json::json!({"status": "ok"}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List images in a collection with full metadata")]
    fn list_collection_images(&self, Parameters(params): Parameters<CollectionIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let scope = self.token_scope();
        match state.db.list_collection_images(&params.collection_id) {
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

    #[tool(
        description = "Create a smart collection from a natural language query (e.g. 'landscape photos rated 4+ from Midjourney')"
    )]
    fn create_smart_collection(
        &self,
        Parameters(params): Parameters<CreateSmartCollectionParams>,
    ) -> String {
        let filter = crate::db_core::nl_parser::parse_query(&params.query);
        let filter_json = match serde_json::to_string(&filter) {
            Ok(j) => j,
            Err(e) => return format!("Error parsing query: {}", e),
        };
        let state = self.app_handle.state::<AppState>();
        match state.db.create_smart_collection(
            &params.name,
            &filter_json,
            Some(&params.query),
            false,
        ) {
            Ok(id) => serde_json::json!({
                "collection_id": id,
                "name": params.name,
                "filter": filter_json,
                "query": params.query,
            })
            .to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Search / AI tools ---

    #[tool(
        description = "Find visually similar images using CLIP embeddings. Requires embeddings to be generated first."
    )]
    fn find_similar(&self, Parameters(params): Parameters<FindSimilarParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        let top_k = clamp_limit(params.limit.unwrap_or(10)) as usize;
        let model_id = params.model.as_deref().unwrap_or("clip-vit-b32");
        if crate::db_core::embeddings::embedding_model_spec(model_id).is_none() {
            return format!("Error: Unsupported embedding model '{}'", model_id);
        }

        let query = match state.db.get_embedding_vector(&params.image_id, model_id) {
            Ok(Some(vector)) => vector,
            Err(e) => return format!("Error: {}", e),
            Ok(None) => {
                return format!(
                    "Error: Image '{}' has no '{}' embedding. Run generate_embeddings first.",
                    params.image_id, model_id
                )
            }
        };
        match state.db.find_similar(&query, model_id, top_k * 2) {
            Ok(results) => {
                let r: Vec<serde_json::Value> = results
                    .iter()
                    .filter(|(id, _)| self.check_image_id_scope(id).unwrap_or(false))
                    .take(top_k)
                    .map(|(id, score)| serde_json::json!({"image_id": id, "similarity": score, "model": model_id}))
                    .collect();
                serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Search for images containing a detected object class (e.g. 'person', 'car', 'dog'). Requires object detection to have been run."
    )]
    fn search_by_object(&self, Parameters(params): Parameters<SearchByObjectParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let limit = clamp_limit(params.limit.unwrap_or(50));
        match state.db.search_by_class(&params.class_name, limit * 2) {
            Ok(results) => {
                let r: Vec<serde_json::Value> = results.iter()
                    .filter(|(id, _)| self.check_image_id_scope(id).unwrap_or(false))
                    .take(limit as usize)
                    .map(|(id, confidence)| {
                        serde_json::json!({"image_id": id, "confidence": confidence})
                    }).collect();
                serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get object detections for an image (bounding boxes, classes, confidence scores)"
    )]
    fn get_detections(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.get_detections(&params.image_id, None) {
            Ok(detections) => {
                serde_json::to_string(&detections).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get AI vision descriptions for an image (generated by Ollama vision models)"
    )]
    fn get_vision_metadata(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.get_vision_metadata(&params.image_id) {
            Ok(fields) => {
                let r: Vec<serde_json::Value> = fields.iter().map(|(key, value, source)| {
                    serde_json::json!({"field": key, "value": value, "source": source})
                }).collect();
                serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get blur, focus, and exposure metrics for an image")]
    fn get_image_quality(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.get_image_quality_metrics(&params.image_id) {
            Ok(metrics) => serde_json::to_string(&metrics).unwrap_or_else(|_| "null".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get count of images with stored blur, focus, and exposure metrics")]
    fn get_quality_count(&self) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.quality_metrics_count() {
            Ok(count) => serde_json::json!({ "count": count }).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get AI generation metadata (prompt, model, seed, provider) for an image")]
    fn get_generation_run(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.get_generation_run_for_image(&params.image_id) {
            Ok(Some(run)) => serde_json::to_string(&run).unwrap_or_else(|_| "null".to_string()),
            Ok(None) => "null".to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Manually attach AI generation metadata to an image (creates a generation run record)"
    )]
    fn set_generation_metadata(
        &self,
        Parameters(params): Parameters<SetGenerationMetadataParams>,
    ) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        let run = crate::db_core::models::GenerationRun {
            id: uuid::Uuid::new_v4().to_string(),
            prompt: Some(params.prompt),
            negative_prompt: None,
            provider: params.provider,
            model: params.model,
            settings_json: params.settings_json.unwrap_or_else(|| "{}".to_string()),
            seed: params.seed,
            parent_run_id: None,
            source_type: "manual".to_string(),
            source_path: None,
            raw_metadata_json: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            imported_at: chrono::Utc::now().to_rfc3339(),
        };
        let run_id = run.id.clone();
        if let Err(e) = state.db.insert_generation_run(&run) {
            return format!("Error creating run: {}", e);
        }
        if let Err(e) = state.db.link_image_to_run(&params.image_id, &run_id) {
            return format!("Error linking image: {}", e);
        }
        format!(
            "Created generation run {} for image {}",
            run_id, params.image_id
        )
    }

    #[tool(
        description = "Rescan all images for sidecar JSON files and backfill generation metadata. Returns the number of images linked."
    )]
    fn rescan_sidecars(&self) -> String {
        let state = self.app_handle.state::<AppState>();
        let images = match state.db.get_images_without_generation_run() {
            Ok(v) => v,
            Err(e) => return format!("Error: {}", e),
        };
        let mut linked = 0u32;
        for (image_id, file_path) in &images {
            let path = std::path::Path::new(file_path);
            if let Some(sidecar_path) = crate::db_core::sidecar::find_sidecar(path) {
                if let Ok(sc) = crate::db_core::sidecar::parse_sidecar(&sidecar_path) {
                    let run_id = uuid::Uuid::new_v4().to_string();
                    let run = crate::db_core::models::GenerationRun {
                        id: run_id.clone(),
                        prompt: sc.prompt,
                        negative_prompt: sc.negative_prompt,
                        provider: sc.provider,
                        model: sc.model,
                        settings_json: sc.settings_json,
                        seed: sc.seed,
                        parent_run_id: None,
                        source_type: "sidecar".to_string(),
                        source_path: Some(sidecar_path.to_string_lossy().to_string()),
                        raw_metadata_json: Some(sc.raw_json),
                        created_at: sc.created_at,
                        imported_at: chrono::Utc::now().to_rfc3339(),
                    };
                    if state.db.insert_generation_run(&run).is_ok() {
                        if state.db.link_image_to_run(image_id, &run_id).is_ok() {
                            linked += 1;
                        }
                    }
                }
            }
        }
        format!(
            "Rescanned {} images, linked {} sidecars",
            images.len(),
            linked
        )
    }

    // --- Display / Navigation tools ---

    #[tool(description = "Open an image in the loupe (fullscreen detail) view on the local app")]
    fn show_image(&self, Parameters(params): Parameters<ShowImageParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        match crate::services::display::show_image(&self.app_handle, &params.image_id) {
            Ok(()) => serde_json::json!({"status": "ok", "action": "opened in loupe"}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Navigate the local app to a folder in grid view")]
    fn navigate_to_folder(&self, Parameters(params): Parameters<NavigateToFolderParams>) -> String {
        let scope = self.token_scope();
        if !tokens::folder_in_scope(&scope, &params.folder_path) {
            return "Error: Access denied — folder outside token scope".to_string();
        }
        match crate::services::display::navigate_to_folder(&self.app_handle, &params.folder_path) {
            Ok(()) => {
                serde_json::json!({"status": "ok", "action": "navigated to folder"}).to_string()
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Display a collection in the local app grid view")]
    fn show_collection(&self, Parameters(params): Parameters<CollectionIdParams>) -> String {
        if let Some(ref scope) = self.token_scope() {
            if let Some(ref allowed) = scope.collections {
                if !allowed.contains(&params.collection_id) {
                    return "Error: Access denied — collection outside token scope".to_string();
                }
            }
        }
        match crate::services::display::show_collection(&self.app_handle, &params.collection_id) {
            Ok(()) => {
                serde_json::json!({"status": "ok", "action": "showing collection"}).to_string()
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get Clipboard Monitor status and active collection ID")]
    fn get_clipboard_monitor_status(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let monitor = state.clipboard_monitor.lock();
        let capture_dir = monitor
            .capture_dir
            .as_ref()
            .map(|path| self.maybe_redact_path(&path.to_string_lossy()))
            .unwrap_or(serde_json::Value::Null);
        serde_json::json!({
            "running": monitor.running,
            "collection_id": monitor.collection_id,
            "collection_name": monitor.collection_name,
            "capture_dir": capture_dir,
            "captured_count": monitor.captured_count,
            "last_error": monitor.last_error,
        })
        .to_string()
    }

    #[tool(description = "Show the active Clipboard Monitor collection in the local app grid")]
    fn show_clipboard_collection(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let collection_id = state
            .clipboard_monitor
            .lock()
            .collection_id
            .clone()
            .or_else(|| {
                state
                    .db
                    .get_setting(crate::services::clipboard_monitor::LAST_COLLECTION_SETTING)
                    .ok()
                    .flatten()
            });
        let Some(collection_id) = collection_id else {
            return "Error: No clipboard collection is available".to_string();
        };
        if let Some(ref scope) = self.token_scope() {
            if let Some(ref allowed) = scope.collections {
                if !allowed.contains(&collection_id) {
                    return "Error: Access denied — collection outside token scope".to_string();
                }
            }
        }
        match crate::services::display::show_collection(&self.app_handle, &collection_id) {
            Ok(()) => serde_json::json!({"status":"ok","collection_id":collection_id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Publish a Clipboard Monitor collection as a local static site and return the URL"
    )]
    async fn publish_clipboard_collection(
        &self,
        Parameters(params): Parameters<OptionalCollectionIdParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        let collection_id = params
            .collection_id
            .or_else(|| state.clipboard_monitor.lock().collection_id.clone())
            .or_else(|| {
                state
                    .db
                    .get_setting(crate::services::clipboard_monitor::LAST_COLLECTION_SETTING)
                    .ok()
                    .flatten()
            });
        let Some(collection_id) = collection_id else {
            return "Error: No clipboard collection is available".to_string();
        };

        let scope = self.token_scope();
        if let Some(ref scope) = scope {
            match &scope.collections {
                Some(allowed) if allowed.contains(&collection_id) => {}
                Some(_) => {
                    return "Error: Access denied — collection outside token scope".to_string()
                }
                None => match state.db.list_collection_images(&collection_id) {
                    Ok(images)
                        if images.iter().all(|img| {
                            tokens::image_in_scope(&Some(scope.clone()), &img.path, &[])
                        }) => {}
                    Ok(_) => {
                        return "Error: Access denied — collection outside token scope".to_string()
                    }
                    Err(e) => return format!("Error: {}", e),
                },
            }
        }

        let export =
            match crate::commands::static_publishing::export_static_publish_collection_inner(
                state.inner(),
                collection_id.clone(),
                None,
                None,
            ) {
                Ok(result) => result,
                Err(e) => return format!("Error: {}", e),
            };
        let server = match crate::commands::static_publishing::serve_static_publish_package_inner(
            state.inner(),
            export.site_dir.clone(),
            Some("127.0.0.1".to_string()),
            None,
        )
        .await
        {
            Ok(result) => result,
            Err(e) => return format!("Error: {}", e),
        };
        let result = serde_json::json!({
            "collection_id": collection_id,
            "image_count": export.image_count,
            "site_dir": export.site_dir,
            "url": server.url,
            "manifest_path": export.manifest_path,
            "instructions_path": export.instructions_path,
        });
        let _ = state
            .db
            .set_setting("clipboard_monitor_last_publish", &result.to_string());
        result.to_string()
    }

    #[tool(description = "Return the last successful Clipboard Monitor publish result")]
    fn get_last_clipboard_publish(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        state
            .db
            .get_setting("clipboard_monitor_last_publish")
            .ok()
            .flatten()
            .unwrap_or_else(|| serde_json::json!({"status":"none"}).to_string())
    }

    // --- Import tools ---

    #[tool(
        description = "Import all images from a folder into the library. Returns imported/skipped/error counts."
    )]
    fn import_folder(
        &self,
        Parameters(params): Parameters<crate::services::import::ImportFolderParams>,
    ) -> String {
        let scope = self.token_scope();
        if !tokens::folder_in_scope(&scope, &params.folder_path) {
            return "Error: Access denied — folder outside token scope".to_string();
        }
        let state = self.app_handle.state::<AppState>();
        match crate::services::import::import_folder(&state.db, &state.app_data_dir, params) {
            Ok(result) => serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Import specific image files into the library. Returns imported/skipped/error counts."
    )]
    fn import_files(
        &self,
        Parameters(params): Parameters<crate::services::import::ImportFilesParams>,
    ) -> String {
        let scope = self.token_scope();
        if params
            .file_paths
            .iter()
            .any(|path| !tokens::image_in_scope(&scope, path, &[]))
        {
            return "Error: Access denied — file outside token scope".to_string();
        }
        let state = self.app_handle.state::<AppState>();
        match crate::services::import::import_files(&state.db, &state.app_data_dir, params) {
            Ok(result) => serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Rescan all imported source folders for new/changed/missing files. Returns count of updated metadata."
    )]
    fn rescan_sources(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let app = self.app_handle.clone();

        let images = match state.db.list_images(100000, 0) {
            Ok(imgs) => imgs,
            Err(e) => return format!("Error: {}", e),
        };

        let total = images.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("rescan", total);
        let job_id_ret = job_id.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let mut updated = 0u32;

            for (i, img) in images.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    state.jobs.mark_cancelled(&job_id);
                    state.jobs.persist_terminal(&job_id, &state.db);
                    let _ = app.emit(
                        "job-status-changed",
                        serde_json::json!({"job_id": &job_id, "status": "cancelled"}),
                    );
                    return;
                }

                let path = std::path::Path::new(&img.path);
                if !path.exists() {
                    state.jobs.update_progress(&job_id, (i + 1) as u32, None);
                    continue;
                }

                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                let png_chunks = if ext == "png" {
                    crate::db_core::source_detection::read_png_text_chunks(path).unwrap_or_default()
                } else {
                    vec![]
                };

                let detection =
                    crate::db_core::source_detection::detect_source(filename, &png_chunks, path);
                if detection.source_label.is_some() {
                    let aspect_ratio = img.image.width as f64 / img.image.height.max(1) as f64;
                    let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
                        "square"
                    } else if aspect_ratio > 1.0 {
                        "landscape"
                    } else {
                        "portrait"
                    };
                    let megapixels =
                        (img.image.width as f64 * img.image.height as f64) / 1_000_000.0;

                    let _ = state.db.update_source_detection(
                        &img.image.id,
                        detection.source_label.as_deref(),
                        detection.confidence,
                        &detection.to_evidence_json(),
                        detection.is_ai_generated,
                        detection.ai_prompt.as_deref(),
                        aspect_ratio,
                        orientation,
                        megapixels,
                    );
                    updated += 1;
                }
                state
                    .jobs
                    .update_progress(&job_id, (i + 1) as u32, Some(filename));
            }

            state.jobs.complete(&job_id);
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit(
                "job-status-changed",
                serde_json::json!({"job_id": &job_id, "status": "completed", "updated": updated}),
            );
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total}).to_string()
    }

    // --- AI / Processing tools ---

    #[tool(
        description = "Download a local embedding model. Supports 'clip-vit-b32' and 'dinov2-vits14'. Returns a background job id."
    )]
    fn download_embedding_model(
        &self,
        Parameters(params): Parameters<DownloadEmbeddingModelParams>,
    ) -> String {
        let model_id = params
            .model
            .unwrap_or_else(|| crate::db_core::embeddings::CLIP_MODEL_ID.to_string());
        let Some(spec) = crate::db_core::embeddings::embedding_model_spec(&model_id) else {
            return format!("Error: Unsupported embedding model '{}'", model_id);
        };
        let state = self.app_handle.state::<AppState>();
        let model_path = {
            let engine = state.embedding_engine.lock();
            match engine.model_path_for(spec.model_id) {
                Ok(path) => path,
                Err(e) => return format!("Error: {}", e),
            }
        };

        if model_path.exists() {
            return serde_json::json!({
                "status": "already_downloaded",
                "model": spec.model_id,
                "model_path": model_path,
            })
            .to_string();
        }

        let (job_id, _cancel_token) = state
            .jobs
            .create_job(&format!("{}-download", spec.model_id), 0);
        let Some(control) = state.jobs.control_for(&job_id) else {
            return format!("Error: Download job '{}' not found", job_id);
        };
        let app = self.app_handle.clone();
        let job_id_ret = job_id.clone();
        let model_path_for_task = model_path.clone();
        let url = spec.url.to_string();
        let model_id_for_task = spec.model_id.to_string();
        let display_name = spec.display_name.to_string();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let client = reqwest::Client::new();
            let result = crate::services::model_download::download_model_file_controlled(
                &client,
                &url,
                &model_path_for_task,
                &control,
                |progress| {
                    state.jobs.update_progress(
                        &job_id,
                        progress.downloaded.min(u32::MAX as u64) as u32,
                        Some(&format!("Downloading {}", display_name)),
                    );
                    let _ = app.emit(
                        "model-download-progress",
                        serde_json::json!({
                            "job_id": &job_id,
                            "model": &model_id_for_task,
                            "downloaded": progress.downloaded,
                            "total": progress.total,
                            "status": progress.status,
                            "resumable": progress.resumable,
                        }),
                    );
                },
            )
            .await;

            match result {
                Ok(_) => {
                    let load_result = {
                        let mut engine = state.embedding_engine.lock();
                        engine.load_model_for(&model_id_for_task)
                    };
                    match load_result {
                        Ok(()) => state.jobs.complete(&job_id),
                        Err(e) => state.jobs.fail(&job_id, &e),
                    }
                }
                Err(e) => {
                    if control.cancellation_token().is_cancelled() {
                        state.jobs.mark_cancelled(&job_id);
                    } else {
                        state.jobs.fail(&job_id, &e);
                    }
                }
            }
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit(
                "job-status-changed",
                serde_json::json!({"job_id": &job_id, "status": state.jobs.get(&job_id).map(|j| j.status)}),
            );
        });

        serde_json::json!({
            "status": "started",
            "job_id": job_id_ret,
            "model": spec.model_id,
            "model_path": model_path,
        })
        .to_string()
    }

    #[tool(
        description = "Generate visual embeddings for images (required for find_similar). Supports CLIP and DINOv2. Returns a background job id."
    )]
    fn generate_embeddings(
        &self,
        Parameters(params): Parameters<GenerateEmbeddingsParams>,
    ) -> String {
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => {
                    return format!(
                        "Error: Access denied — image '{}' outside token scope",
                        image_id
                    )
                }
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();
        let model_id = params
            .model
            .unwrap_or_else(|| crate::db_core::embeddings::CLIP_MODEL_ID.to_string());
        if crate::db_core::embeddings::embedding_model_spec(&model_id).is_none() {
            return format!("Error: Unsupported embedding model '{}'", model_id);
        }

        {
            let mut engine = state.embedding_engine.lock();
            if let Err(e) = engine.load_model_for(&model_id) {
                return format!("Error loading model: {}", e);
            }
        }

        let total = params.image_ids.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("embeddings", total);
        let job_id_ret = job_id.clone();
        let app = self.app_handle.clone();
        let image_ids = params.image_ids.clone();
        let model_id_for_task = model_id.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let result = crate::services::model_pipeline::run_embedding_model(
                crate::services::model_pipeline::EmbeddingRunRequest {
                    db: &state.db,
                    app_data_dir: &state.app_data_dir,
                    embedding_engine: &state.embedding_engine,
                    jobs: Some(&state.jobs),
                    job_id: Some(&job_id),
                    cancel: Some(&cancel_token),
                    app: Some(&app),
                    model_id: &model_id_for_task,
                    image_ids: &image_ids,
                },
            );
            match result {
                Ok(result) if result.status == "cancelled" => state.jobs.mark_cancelled(&job_id),
                Ok(_) => state.jobs.complete(&job_id),
                Err(e) => state.jobs.fail(&job_id, &e),
            }
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit(
                "job-status-changed",
                serde_json::json!({"job_id": &job_id, "status": state.jobs.get(&job_id).map(|j| j.status)}),
            );
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total, "model": model_id})
            .to_string()
    }

    #[tool(
        description = "Analyze blur, focus, and exposure for images. Returns a background job id."
    )]
    fn analyze_image_quality(
        &self,
        Parameters(params): Parameters<AnalyzeImageQualityParams>,
    ) -> String {
        if params.image_ids.is_empty() {
            return "Error: analyze_image_quality requires at least one image_id".to_string();
        }
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => {
                    return format!(
                        "Error: Access denied — image '{}' outside token scope",
                        image_id
                    )
                }
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }

        let state = self.app_handle.state::<AppState>();
        let total = params.image_ids.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("quality", total);
        let job_id_ret = job_id.clone();
        let app = self.app_handle.clone();
        let image_ids = params.image_ids.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let mut analyzed = 0u32;

            for (i, image_id) in image_ids.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    state.jobs.mark_cancelled(&job_id);
                    state.jobs.persist_terminal(&job_id, &state.db);
                    let _ = app.emit(
                        "job-status-changed",
                        serde_json::json!({"job_id": &job_id, "status": "cancelled"}),
                    );
                    return;
                }

                let ctx =
                    crate::services::ServiceContext::from_app_state(&state, Some(app.clone()));
                match crate::services::ai::analyze_image_quality(&ctx, image_id) {
                    Ok(_) => analyzed += 1,
                    Err(e) => {
                        crate::safe_eprintln!("Quality analysis error for {}: {}", image_id, e)
                    }
                }
                state
                    .jobs
                    .update_progress(&job_id, (i + 1) as u32, Some(image_id));
                let _ = app.emit(
                    "quality-progress",
                    serde_json::json!({
                        "job_id": &job_id,
                        "current": i + 1,
                        "total": total,
                        "analyzer": crate::db_core::quality::QUALITY_ANALYZER_VERSION,
                    }),
                );
            }

            state.jobs.complete(&job_id);
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit(
                "job-status-changed",
                serde_json::json!({"job_id": &job_id, "status": "completed", "processed": analyzed}),
            );
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total}).to_string()
    }

    #[tool(description = "Run YOLO object detection on images. Returns count processed.")]
    fn detect_objects(&self, Parameters(params): Parameters<DetectObjectsParams>) -> String {
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => {
                    return format!(
                        "Error: Access denied — image '{}' outside token scope",
                        image_id
                    )
                }
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();

        let variant = match params.variant.as_deref() {
            Some(v) => match crate::db_core::detection::YoloVariant::from_str(v) {
                Some(var) => var,
                None => {
                    return format!(
                        "Error: Invalid variant '{}'. Use 'nano', 'small', or 'medium'.",
                        v
                    )
                }
            },
            None => crate::db_core::detection::YoloVariant::Medium,
        };

        {
            let mut engine = state.detection_engine.lock();
            let needs_load = engine.session.is_none() || engine.loaded_variant != Some(variant);
            if needs_load {
                if !engine.is_variant_available(variant) {
                    return format!("Error: Model {} not downloaded", variant.model_name());
                }
                if let Err(e) = engine.load_yolo(variant) {
                    return format!("Error loading model: {}", e);
                }
            }
        }

        let total = params.image_ids.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("detection", total);
        let job_id_ret = job_id.clone();
        let app = self.app_handle.clone();
        let image_ids = params.image_ids.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let mut detected = 0u32;

            for (i, image_id) in image_ids.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    state.jobs.mark_cancelled(&job_id);
                    state.jobs.persist_terminal(&job_id, &state.db);
                    let _ = app.emit(
                        "job-status-changed",
                        serde_json::json!({"job_id": &job_id, "status": "cancelled"}),
                    );
                    return;
                }

                let id_refs: Vec<&str> = vec![image_id.as_str()];
                let images = match state.db.get_images_by_ids(&id_refs) {
                    Ok(imgs) => imgs,
                    Err(_) => {
                        state.jobs.update_progress(&job_id, (i + 1) as u32, None);
                        continue;
                    }
                };
                let img = match images.first() {
                    Some(img) => img,
                    None => {
                        state.jobs.update_progress(&job_id, (i + 1) as u32, None);
                        continue;
                    }
                };

                let engine = state.detection_engine.lock();
                match engine.detect(std::path::Path::new(&img.path)) {
                    Ok(detections) => {
                        drop(engine);
                        let _ =
                            state
                                .db
                                .store_detections(image_id, variant.model_name(), &detections);
                        detected += 1;
                    }
                    Err(e) => {
                        drop(engine);
                        crate::safe_eprintln!("Detection error for {}: {}", image_id, e);
                    }
                }
                state
                    .jobs
                    .update_progress(&job_id, (i + 1) as u32, Some(image_id));
                let _ = app.emit("detection-progress", serde_json::json!({"current": i + 1, "total": total, "model": variant.model_name()}));
            }

            state.jobs.complete(&job_id);
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "completed", "processed": detected}));
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total}).to_string()
    }

    #[tool(
        description = "Analyze images with Ollama vision model for natural language descriptions. Returns count processed."
    )]
    fn analyze_images(&self, Parameters(params): Parameters<AnalyzeImagesParams>) -> String {
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => {
                    return format!(
                        "Error: Access denied — image '{}' outside token scope",
                        image_id
                    )
                }
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();

        let url = state
            .db
            .get_setting("ollama_url")
            .unwrap_or(None)
            .unwrap_or_else(|| crate::db_core::vision::default_ollama_url().to_string());
        let model = state
            .db
            .get_setting("ollama_model")
            .unwrap_or(None)
            .unwrap_or_else(|| "minicpm-v".to_string());

        let total = params.image_ids.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("vision", total);
        let job_id_ret = job_id.clone();
        let app = self.app_handle.clone();
        let image_ids = params.image_ids.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let mut analyzed = 0u32;

            for (i, image_id) in image_ids.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    state.jobs.mark_cancelled(&job_id);
                    state.jobs.persist_terminal(&job_id, &state.db);
                    let _ = app.emit(
                        "job-status-changed",
                        serde_json::json!({"job_id": &job_id, "status": "cancelled"}),
                    );
                    return;
                }

                let id_refs: Vec<&str> = vec![image_id.as_str()];
                let images = match state.db.get_images_by_ids(&id_refs) {
                    Ok(imgs) => imgs,
                    Err(_) => {
                        state.jobs.update_progress(&job_id, (i + 1) as u32, None);
                        continue;
                    }
                };
                let img = match images.first() {
                    Some(img) => img,
                    None => {
                        state.jobs.update_progress(&job_id, (i + 1) as u32, None);
                        continue;
                    }
                };

                let path = img.path.clone();
                let url_clone = url.clone();
                let model_clone = model.clone();

                let result = crate::db_core::vision::analyze_image(
                    std::path::Path::new(&path),
                    &url_clone,
                    &model_clone,
                )
                .await;

                if cancel_token.is_cancelled() {
                    state.jobs.mark_cancelled(&job_id);
                    state.jobs.persist_terminal(&job_id, &state.db);
                    return;
                }

                match result {
                    Ok(fields) => {
                        let _ = state.db.store_vision_metadata(image_id, &model, &fields);
                        analyzed += 1;
                    }
                    Err(e) => crate::safe_eprintln!("Vision error for {}: {}", image_id, e),
                }
                state
                    .jobs
                    .update_progress(&job_id, (i + 1) as u32, Some(image_id));
                let _ = app.emit(
                    "vision-progress",
                    serde_json::json!({"current": i + 1, "total": total, "model": model}),
                );
            }

            state.jobs.complete(&job_id);
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "completed", "processed": analyzed}));
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total}).to_string()
    }

    // --- Job management tools ---

    #[tool(description = "Get status and progress of a background job by ID")]
    fn get_job(&self, Parameters(params): Parameters<GetJobParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.jobs.get(&params.job_id) {
            Some(snapshot) => serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string()),
            None => format!("Error: Job '{}' not found", params.job_id),
        }
    }

    #[tool(
        description = "List all background jobs (running and recent completed/failed/cancelled)"
    )]
    fn list_jobs(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let jobs = state.jobs.list();
        serde_json::to_string(&jobs).unwrap_or_else(|_| "[]".to_string())
    }

    #[tool(
        description = "Cancel a running background job. Sets status to 'cancelling', job stops after current item."
    )]
    fn cancel_job(&self, Parameters(params): Parameters<CancelJobParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.jobs.cancel(&params.job_id) {
            Ok(()) => {
                serde_json::json!({"status": "cancelling", "job_id": params.job_id}).to_string()
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Token management tools ---

    #[tool(
        description = "Create a new MCP access token. Returns token ID and secret (shown only once)."
    )]
    fn create_token(&self, Parameters(params): Parameters<CreateTokenParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::create_token(&ctx, &params.name, &params.role, params.scope)
        {
            Ok((token, secret)) => serde_json::json!({
                "token_id": token.id,
                "name": token.name,
                "role": token.role,
                "secret": secret,
                "warning": "Store the secret securely — it cannot be retrieved again"
            })
            .to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List all active (non-revoked) MCP tokens")]
    fn list_tokens(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::list_tokens(&ctx) {
            Ok(tokens_list) => {
                let result: Vec<serde_json::Value> = tokens_list
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "id": t.id, "name": t.name, "role": t.role,
                            "created_at": t.created_at, "last_used_at": t.last_used_at,
                        })
                    })
                    .collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Revoke an MCP token permanently")]
    fn revoke_token(&self, Parameters(params): Parameters<TokenIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::revoke_token(&ctx, &params.token_id) {
            Ok(()) => serde_json::json!({"status": "ok", "revoked": params.token_id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Rotate a token's secret. Returns new secret (old becomes invalid).")]
    fn rotate_token(&self, Parameters(params): Parameters<TokenIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::rotate_token(&ctx, &params.token_id) {
            Ok(new_secret) => serde_json::json!({
                "token_id": params.token_id,
                "new_secret": new_secret,
                "warning": "Store the new secret securely"
            })
            .to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Audit tools ---

    #[tool(description = "Get recent MCP audit log entries")]
    fn get_audit_log(&self, Parameters(params): Parameters<AuditLogParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        let limit = params.limit.unwrap_or(50).min(500);
        match crate::services::tokens::get_recent_audit(&ctx, limit) {
            Ok(entries) => {
                let result: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id, "token_id": e.token_id,
                            "tool_name": e.tool_name, "result_status": e.result_status,
                            "timestamp": e.timestamp,
                        })
                    })
                    .collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Delete old audit log entries. Returns count deleted.")]
    fn prune_audit_log(&self, Parameters(params): Parameters<PruneAuditParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        let days = params.retention_days.unwrap_or(30);
        match crate::services::tokens::prune_audit_log(&ctx, days) {
            Ok(deleted) => {
                serde_json::json!({"deleted": deleted, "retention_days": days}).to_string()
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Export tools ---

    #[tool(description = "List available export presets (platforms, sizes, formats)")]
    fn list_export_presets(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let presets = crate::services::export::list_presets();
        serde_json::to_string(&presets).unwrap_or_else(|_| "[]".to_string())
    }

    #[tool(
        description = "List saved Canvas records for a session, optionally filtered by exact canvas_name. Use this to resolve a canvas_id before exporting a saved Canvas."
    )]
    fn list_session_canvases(
        &self,
        Parameters(params): Parameters<ListSessionCanvasesParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.list_canvases(&params.session_id) {
            Ok(canvases) => {
                let result = canvas_summaries_for_mcp(canvases, params.canvas_name.as_deref());
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get a saved Canvas v1 layout document by canvas_id, including item transforms, crops, annotations, and comments."
    )]
    fn get_canvas_layout(&self, Parameters(params): Parameters<GetCanvasLayoutParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let scope = self.token_scope();
        match state.db.get_canvas(&params.canvas_id) {
            Ok(Some(canvas)) => match canvas_layout_for_mcp(&canvas) {
                Ok(mut result) => {
                    // Filter canvas items by token scope
                    if scope.is_some() {
                        if let Some(layout) = result.get_mut("layout") {
                            if let Some(items) = layout.get_mut("items") {
                                if let Some(arr) = items.as_array() {
                                    let image_ids: Vec<&str> = arr
                                        .iter()
                                        .filter_map(|item| item.get("imageId")?.as_str())
                                        .collect();
                                    // Batch-resolve image paths for scope checking
                                    let id_refs: Vec<&str> = image_ids.clone();
                                    let images_by_id =
                                        state.db.get_images_by_ids(&id_refs).unwrap_or_default();
                                    let allowed_ids: std::collections::HashSet<&str> = images_by_id
                                        .iter()
                                        .filter(|img| {
                                            tokens::image_in_scope(&scope, &img.path, &[])
                                        })
                                        .map(|img| img.image.id.as_str())
                                        .collect();
                                    let filtered: Vec<serde_json::Value> = arr
                                        .iter()
                                        .filter(|item| {
                                            item.get("imageId")
                                                .and_then(|v| v.as_str())
                                                .map(|id| allowed_ids.contains(id))
                                                .unwrap_or(false)
                                        })
                                        .cloned()
                                        .collect();
                                    *items = serde_json::Value::Array(filtered);
                                }
                            }
                        }
                    }
                    serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
                }
                Err(e) => format!("Error: {}", e),
            },
            Ok(None) => format!("Error: Canvas '{}' was not found", params.canvas_id),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Export images selected by image_ids, collection_id, or folder_path to an output directory."
    )]
    fn export_images(
        &self,
        Parameters(params): Parameters<crate::services::export::ExportImagesParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        let scope = self.token_scope();

        if let Some(image_ids) = params.image_ids.as_ref() {
            for image_id in image_ids {
                match self.check_image_id_scope(image_id) {
                    Ok(false) => {
                        return format!(
                            "Error: Access denied — image '{}' outside token scope",
                            image_id
                        )
                    }
                    Err(e) => return format!("Error: {}", e),
                    _ => {}
                }
            }
        }
        if let Some(folder_path) = params.folder_path.as_ref() {
            if !tokens::folder_in_scope(&scope, folder_path) {
                return "Error: Access denied — folder outside token scope".to_string();
            }
        }
        if let Some(collection_id) = params.collection_id.as_ref() {
            if scope.is_some() {
                let collection_allowed = scope
                    .as_ref()
                    .and_then(|s| s.collections.as_ref())
                    .map(|allowed| allowed.contains(collection_id))
                    .unwrap_or(false);
                if !collection_allowed {
                    match state.db.list_collection_images(collection_id) {
                        Ok(images)
                            if images
                                .iter()
                                .all(|img| tokens::image_in_scope(&scope, &img.path, &[])) => {}
                        Ok(_) => {
                            return "Error: Access denied — collection outside token scope"
                                .to_string()
                        }
                        Err(e) => return format!("Error: {}", e),
                    }
                }
            }
        }

        match crate::services::export::export_images(&state.db, &state.app_data_dir, params) {
            Ok(result) => serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Export a read-only static presentation package from canvas image IDs. Requires the Static Publishing module to be enabled."
    )]
    fn export_static_publish_package(
        &self,
        Parameters(params): Parameters<crate::commands::static_publishing::StaticPublishRequest>,
    ) -> String {
        for item in &params.items {
            match self.check_image_id_scope(&item.image_id) {
                Ok(false) => {
                    return format!(
                        "Error: Access denied — image '{}' outside token scope",
                        item.image_id
                    )
                }
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }

        let state = self.app_handle.state::<AppState>();
        match crate::commands::static_publishing::export_static_publish_package_inner(
            state.inner(),
            params,
        ) {
            Ok(result) => serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Export a read-only static presentation package from a saved Canvas record by canvas_id. Uses persisted canvas layout_json, not live UI state. Requires the Static Publishing module to be enabled."
    )]
    fn export_static_publish_canvas(
        &self,
        Parameters(params): Parameters<
            crate::commands::static_publishing::StaticPublishCanvasRequest,
        >,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        let canvas = match state.db.get_canvas(&params.canvas_id) {
            Ok(Some(canvas)) => canvas,
            Ok(None) => return format!("Error: Canvas '{}' was not found", params.canvas_id),
            Err(e) => return format!("Error: {}", e),
        };
        let document = match crate::db_core::canvas_document::CanvasDocument::from_layout_json(
            &canvas.layout_json,
        ) {
            Ok(document) => document,
            Err(e) => return format!("Error: Invalid canvas layout_json: {}", e),
        };
        let id_refs: Vec<&str> = document
            .items
            .iter()
            .map(|item| item.image_id.as_str())
            .collect();

        match state.db.get_images_by_ids(&id_refs) {
            Ok(images) => {
                for image in images {
                    if !self.check_image_scope(&image.path) {
                        return format!(
                            "Error: Access denied — image '{}' outside token scope",
                            image.image.id
                        );
                    }
                }
            }
            Err(e) => return format!("Error: {}", e),
        }

        match crate::commands::static_publishing::export_static_publish_canvas_inner(
            state.inner(),
            params,
        ) {
            Ok(result) => serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Serve a generated static publishing site over a local read-only HTTP server. Requires the Static Publishing module and settings/admin permission."
    )]
    async fn serve_static_publish_package(
        &self,
        Parameters(params): Parameters<ServeStaticPublishParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match crate::commands::static_publishing::serve_static_publish_package_inner(
            state.inner(),
            params.site_dir,
            params.host,
            params.port,
        )
        .await
        {
            Ok(result) => serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }
}

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

    // --- Path redaction (tests production `redact_path`) ---

    #[test]
    fn test_redact_path_extracts_filename() {
        assert_eq!(
            super::redact_path("/Users/gleb/art/midjourney/image_001.png"),
            "image_001.png"
        );
    }

    #[test]
    fn test_redact_path_preserves_extension() {
        assert_eq!(super::redact_path("/some/deep/path/photo.CR2"), "photo.CR2");
    }

    #[test]
    fn test_redact_path_handles_root() {
        assert_eq!(super::redact_path("/"), "[redacted]");
    }

    #[test]
    fn test_redact_path_handles_empty() {
        assert_eq!(super::redact_path(""), "[redacted]");
    }

    #[test]
    fn test_redact_path_handles_spaces() {
        assert_eq!(
            super::redact_path("/Users/gleb/My Art/image 001.png"),
            "image 001.png"
        );
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
                super::is_valid_decision(value),
                "Decision '{}' should be valid",
                value
            );
        }
    }

    #[test]
    fn test_decision_invalid_values() {
        assert!(!super::is_valid_decision("maybe"));
        assert!(!super::is_valid_decision(""));
        assert!(!super::is_valid_decision("SELECTED"));
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
