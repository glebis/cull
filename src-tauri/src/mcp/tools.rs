// Copyright (c) 2025-2026 Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo, CallToolRequestParams, CallToolResult},
    schemars, tool, tool_router, ErrorData,
    service::RequestContext, RoleServer,
};
use tauri::{Manager, Emitter};

use crate::AppState;
use crate::db_core::models::TokenScope;
use crate::services::tokens;
use super::auth::{AuthContext, require_capability};

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

fn is_valid_decision(decision: &str) -> bool {
    matches!(decision, "selected" | "rejected" | "none")
}

#[derive(Debug, Clone)]
pub struct ImageViewMcp {
    pub app_handle: tauri::AppHandle,
    pub auth: AuthContext,
    tool_router: ToolRouter<Self>,
}

impl ImageViewMcp {
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
    fn filter_images_by_scope(&self, images: Vec<serde_json::Value>, paths: &[String]) -> Vec<serde_json::Value> {
        let scope = self.token_scope();
        if scope.is_none() {
            return images;
        }
        images.into_iter()
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
        let ctx = crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        let token_id = match &self.auth {
            AuthContext::Local => None,
            AuthContext::Authenticated(t) => Some(t.id.as_str()),
        };
        let _ = crate::services::tokens::log_audit(&ctx, token_id, tool_name, params_json, status);
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EmptyParams {}

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
    #[schemars(description = "Decision: 'selected', 'rejected', or 'none'")]
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
pub struct CreateSmartCollectionParams {
    #[schemars(description = "Name for the smart collection")]
    pub name: String,
    #[schemars(description = "Natural language query like 'landscape photos rated 4+' or raw filter JSON")]
    pub query: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FindSimilarParams {
    #[schemars(description = "Image ID to find similar images for")]
    pub image_id: String,
    #[schemars(description = "Number of results to return (default 10)")]
    pub limit: Option<u32>,
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

// --- Import param structs ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ImportFolderParams {
    #[schemars(description = "Absolute path to folder to import")]
    pub folder_path: String,
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
    #[schemars(description = "List of image IDs to generate CLIP embeddings for")]
    pub image_ids: Vec<String>,
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

#[tool_router]
impl ImageViewMcp {
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
        }).to_string()
    }

    #[tool(description = "List images with pagination. Returns id, path, dimensions, format, rating, decision.")]
    fn list_images(&self, Parameters(params): Parameters<ListImagesParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let offset = params.offset.unwrap_or(0);
        let limit = clamp_limit(params.limit.unwrap_or(50));
        let scope = self.token_scope();
        let fetch_limit = if scope.is_some() { limit * 3 } else { limit };

        match state.db.list_images(fetch_limit, offset) {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images.iter()
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
                }).collect();
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
                    }).to_string()
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

        match state.db.list_images_by_folder(&params.folder_path, limit, offset) {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images.iter()
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
                }).collect();
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

    #[tool(description = "Set selection decision on an image: 'selected', 'rejected', or 'none'")]
    fn set_decision(&self, Parameters(params): Parameters<SetDecisionParams>) -> String {
        if !is_valid_decision(&params.decision) {
            return "Error: Decision must be 'selected', 'rejected', or 'none'".to_string();
        }
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.set_decision(&params.image_id, &params.decision) {
            Ok(()) => serde_json::json!({"status": "ok", "image_id": params.image_id, "decision": params.decision}).to_string(),
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
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => return format!("Error: Access denied — image '{}' outside token scope", image_id),
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();
        let refs: Vec<&str> = params.image_ids.iter().map(|s| s.as_str()).collect();
        match state.db.add_to_collection(&params.collection_id, &refs) {
            Ok(()) => serde_json::json!({"status": "ok", "added": params.image_ids.len()}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Delete a collection (does not delete the images)")]
    fn delete_collection(&self, Parameters(params): Parameters<CollectionIdParams>) -> String {
        if let Some(ref scope) = self.token_scope() {
            if let Some(ref allowed) = scope.collections {
                if !allowed.contains(&params.collection_id) {
                    return "Error: Access denied — collection outside token scope".to_string();
                }
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
                let result: Vec<serde_json::Value> = images.iter()
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
                }).collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Create a smart collection from a natural language query (e.g. 'landscape photos rated 4+ from Midjourney')")]
    fn create_smart_collection(&self, Parameters(params): Parameters<CreateSmartCollectionParams>) -> String {
        let filter = crate::db_core::nl_parser::parse_query(&params.query);
        let filter_json = match serde_json::to_string(&filter) {
            Ok(j) => j,
            Err(e) => return format!("Error parsing query: {}", e),
        };
        let state = self.app_handle.state::<AppState>();
        match state.db.create_smart_collection(&params.name, &filter_json, Some(&params.query), false) {
            Ok(id) => serde_json::json!({
                "collection_id": id,
                "name": params.name,
                "filter": filter_json,
                "query": params.query,
            }).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Search / AI tools ---

    #[tool(description = "Find visually similar images using CLIP embeddings. Requires embeddings to be generated first.")]
    fn find_similar(&self, Parameters(params): Parameters<FindSimilarParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        let top_k = clamp_limit(params.limit.unwrap_or(10)) as usize;

        let all = match state.db.get_all_embeddings("clip-vit-b32") {
            Ok(a) => a,
            Err(e) => return format!("Error: {}", e),
        };
        let query = match all.iter().find(|(id, _)| id == &params.image_id) {
            Some(q) => q,
            None => return format!("Error: Image '{}' has no embedding. Run generate_embeddings first.", params.image_id),
        };
        match state.db.find_similar(&query.1, "clip-vit-b32", top_k * 2) {
            Ok(results) => {
                let r: Vec<serde_json::Value> = results.iter()
                    .filter(|(id, _)| {
                        self.check_image_id_scope(id).unwrap_or(false)
                    })
                    .take(top_k)
                    .map(|(id, score)| {
                        serde_json::json!({"image_id": id, "similarity": score})
                    }).collect();
                serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Search for images containing a detected object class (e.g. 'person', 'car', 'dog'). Requires object detection to have been run.")]
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

    #[tool(description = "Get object detections for an image (bounding boxes, classes, confidence scores)")]
    fn get_detections(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.get_detections(&params.image_id, None) {
            Ok(detections) => serde_json::to_string(&detections).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get AI vision descriptions for an image (generated by Ollama vision models)")]
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

    #[tool(description = "Manually attach AI generation metadata to an image (creates a generation run record)")]
    fn set_generation_metadata(&self, Parameters(params): Parameters<SetGenerationMetadataParams>) -> String {
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
        format!("Created generation run {} for image {}", run_id, params.image_id)
    }

    #[tool(description = "Rescan all images for sidecar JSON files and backfill generation metadata. Returns the number of images linked.")]
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
        format!("Rescanned {} images, linked {} sidecars", images.len(), linked)
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
            Ok(()) => serde_json::json!({"status": "ok", "action": "navigated to folder"}).to_string(),
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
            Ok(()) => serde_json::json!({"status": "ok", "action": "showing collection"}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Import tools ---

    #[tool(description = "Import all images from a folder into the library. Returns imported/skipped/error counts.")]
    fn import_folder(&self, Parameters(params): Parameters<ImportFolderParams>) -> String {
        let scope = self.token_scope();
        if !tokens::folder_in_scope(&scope, &params.folder_path) {
            return "Error: Access denied — folder outside token scope".to_string();
        }
        let state = self.app_handle.state::<AppState>();
        let app = self.app_handle.clone();

        let extensions = ["jpg", "jpeg", "png", "webp", "gif"];
        let entries: Vec<std::path::PathBuf> = walkdir::WalkDir::new(&params.folder_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path().extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| extensions.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        let total = entries.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("import", total);
        let job_id_ret = job_id.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let mut imported = 0u32;
            let mut skipped = 0u32;
            let mut errors = 0u32;

            for (i, path) in entries.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    state.jobs.mark_cancelled(&job_id);
                    state.jobs.persist_terminal(&job_id, &state.db);
                    let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "cancelled"}));
                    return;
                }

                let filename = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                match crate::db_core::import::import_file(&state.db, path, &state.app_data_dir) {
                    Ok(Some(_)) => imported += 1,
                    Ok(None) => skipped += 1,
                    Err(_) => errors += 1,
                }
                state.jobs.update_progress(&job_id, (i + 1) as u32, Some(&filename));
                let _ = app.emit("import-progress", serde_json::json!({"current": i + 1, "total": total, "filename": filename}));
            }

            state.jobs.complete(&job_id);
                state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit("job-status-changed", serde_json::json!({
                "job_id": &job_id, "status": "completed",
                "imported": imported, "skipped": skipped, "errors": errors,
            }));
        });

        serde_json::json!({"job_id": job_id_ret, "total_files": total}).to_string()
    }

    #[tool(description = "Rescan all imported source folders for new/changed/missing files. Returns count of updated metadata.")]
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
                    let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "cancelled"}));
                    return;
                }

                let path = std::path::Path::new(&img.path);
                if !path.exists() {
                    state.jobs.update_progress(&job_id, (i + 1) as u32, None);
                    continue;
                }

                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
                let png_chunks = if ext == "png" {
                    crate::db_core::source_detection::read_png_text_chunks(path).unwrap_or_default()
                } else {
                    vec![]
                };

                let detection = crate::db_core::source_detection::detect_source(filename, &png_chunks, path);
                if detection.source_label.is_some() {
                    let aspect_ratio = img.image.width as f64 / img.image.height.max(1) as f64;
                    let orientation = if (aspect_ratio - 1.0).abs() < 0.05 { "square" }
                        else if aspect_ratio > 1.0 { "landscape" }
                        else { "portrait" };
                    let megapixels = (img.image.width as f64 * img.image.height as f64) / 1_000_000.0;

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
                state.jobs.update_progress(&job_id, (i + 1) as u32, Some(filename));
            }

            state.jobs.complete(&job_id);
                state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "completed", "updated": updated}));
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total}).to_string()
    }

    // --- AI / Processing tools ---

    #[tool(description = "Generate CLIP visual embeddings for images (required for find_similar). Returns count processed.")]
    fn generate_embeddings(&self, Parameters(params): Parameters<GenerateEmbeddingsParams>) -> String {
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => return format!("Error: Access denied — image '{}' outside token scope", image_id),
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();

        {
            let mut engine = state.embedding_engine.lock();
            if engine.session.is_none() {
                if !engine.is_model_available() {
                    return "Error: Model not downloaded. Run download_clip_model first.".to_string();
                }
                if let Err(e) = engine.load_model() {
                    return format!("Error loading model: {}", e);
                }
            }
        }

        let total = params.image_ids.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("embeddings", total);
        let job_id_ret = job_id.clone();
        let app = self.app_handle.clone();
        let image_ids = params.image_ids.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let mut generated = 0u32;

            for (i, image_id) in image_ids.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    state.jobs.mark_cancelled(&job_id);
                    state.jobs.persist_terminal(&job_id, &state.db);
                    let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "cancelled"}));
                    return;
                }

                let id_refs: Vec<&str> = vec![image_id.as_str()];
                let images = match state.db.get_images_by_ids(&id_refs) {
                    Ok(imgs) => imgs,
                    Err(_) => { state.jobs.update_progress(&job_id, (i + 1) as u32, None); continue; },
                };
                let img = match images.first() {
                    Some(img) => img,
                    None => { state.jobs.update_progress(&job_id, (i + 1) as u32, None); continue; },
                };

                let engine = state.embedding_engine.lock();
                match engine.generate_embedding(std::path::Path::new(&img.path)) {
                    Ok(embedding) => {
                        drop(engine);
                        let _ = state.db.store_embedding(image_id, "clip-vit-b32", &embedding);
                        generated += 1;
                    }
                    Err(e) => {
                        drop(engine);
                        eprintln!("Embedding error for {}: {}", image_id, e);
                    }
                }
                state.jobs.update_progress(&job_id, (i + 1) as u32, Some(image_id));
                let _ = app.emit("embedding-progress", serde_json::json!({"current": i + 1, "total": total}));
            }

            state.jobs.complete(&job_id);
                state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "completed", "processed": generated}));
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total}).to_string()
    }

    #[tool(description = "Run YOLO object detection on images. Returns count processed.")]
    fn detect_objects(&self, Parameters(params): Parameters<DetectObjectsParams>) -> String {
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => return format!("Error: Access denied — image '{}' outside token scope", image_id),
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();

        let variant = match params.variant.as_deref() {
            Some(v) => match crate::db_core::detection::YoloVariant::from_str(v) {
                Some(var) => var,
                None => return format!("Error: Invalid variant '{}'. Use 'nano', 'small', or 'medium'.", v),
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
                    let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "cancelled"}));
                    return;
                }

                let id_refs: Vec<&str> = vec![image_id.as_str()];
                let images = match state.db.get_images_by_ids(&id_refs) {
                    Ok(imgs) => imgs,
                    Err(_) => { state.jobs.update_progress(&job_id, (i + 1) as u32, None); continue; },
                };
                let img = match images.first() {
                    Some(img) => img,
                    None => { state.jobs.update_progress(&job_id, (i + 1) as u32, None); continue; },
                };

                let engine = state.detection_engine.lock();
                match engine.detect(std::path::Path::new(&img.path)) {
                    Ok(detections) => {
                        drop(engine);
                        let _ = state.db.store_detections(image_id, variant.model_name(), &detections);
                        detected += 1;
                    }
                    Err(e) => {
                        drop(engine);
                        eprintln!("Detection error for {}: {}", image_id, e);
                    }
                }
                state.jobs.update_progress(&job_id, (i + 1) as u32, Some(image_id));
                let _ = app.emit("detection-progress", serde_json::json!({"current": i + 1, "total": total, "model": variant.model_name()}));
            }

            state.jobs.complete(&job_id);
                state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "completed", "processed": detected}));
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total}).to_string()
    }

    #[tool(description = "Analyze images with Ollama vision model for natural language descriptions. Returns count processed.")]
    fn analyze_images(&self, Parameters(params): Parameters<AnalyzeImagesParams>) -> String {
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => return format!("Error: Access denied — image '{}' outside token scope", image_id),
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();

        let url = state.db.get_setting("ollama_url")
            .unwrap_or(None)
            .unwrap_or_else(|| crate::db_core::vision::default_ollama_url().to_string());
        let model = state.db.get_setting("ollama_model")
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
                    let _ = app.emit("job-status-changed", serde_json::json!({"job_id": &job_id, "status": "cancelled"}));
                    return;
                }

                let id_refs: Vec<&str> = vec![image_id.as_str()];
                let images = match state.db.get_images_by_ids(&id_refs) {
                    Ok(imgs) => imgs,
                    Err(_) => { state.jobs.update_progress(&job_id, (i + 1) as u32, None); continue; },
                };
                let img = match images.first() {
                    Some(img) => img,
                    None => { state.jobs.update_progress(&job_id, (i + 1) as u32, None); continue; },
                };

                let path = img.path.clone();
                let url_clone = url.clone();
                let model_clone = model.clone();

                let result = crate::db_core::vision::analyze_image(
                    std::path::Path::new(&path), &url_clone, &model_clone
                ).await;

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
                    Err(e) => eprintln!("Vision error for {}: {}", image_id, e),
                }
                state.jobs.update_progress(&job_id, (i + 1) as u32, Some(image_id));
                let _ = app.emit("vision-progress", serde_json::json!({"current": i + 1, "total": total, "model": model}));
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

    #[tool(description = "List all background jobs (running and recent completed/failed/cancelled)")]
    fn list_jobs(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let jobs = state.jobs.list();
        serde_json::to_string(&jobs).unwrap_or_else(|_| "[]".to_string())
    }

    #[tool(description = "Cancel a running background job. Sets status to 'cancelling', job stops after current item.")]
    fn cancel_job(&self, Parameters(params): Parameters<CancelJobParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.jobs.cancel(&params.job_id) {
            Ok(()) => serde_json::json!({"status": "cancelling", "job_id": params.job_id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Token management tools ---

    #[tool(description = "Create a new MCP access token. Returns token ID and secret (shown only once).")]
    fn create_token(&self, Parameters(params): Parameters<CreateTokenParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx = crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::create_token(&ctx, &params.name, &params.role, params.scope) {
            Ok((token, secret)) => serde_json::json!({
                "token_id": token.id,
                "name": token.name,
                "role": token.role,
                "secret": secret,
                "warning": "Store the secret securely — it cannot be retrieved again"
            }).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List all active (non-revoked) MCP tokens")]
    fn list_tokens(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx = crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::list_tokens(&ctx) {
            Ok(tokens_list) => {
                let result: Vec<serde_json::Value> = tokens_list.iter().map(|t| {
                    serde_json::json!({
                        "id": t.id, "name": t.name, "role": t.role,
                        "created_at": t.created_at, "last_used_at": t.last_used_at,
                    })
                }).collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Revoke an MCP token permanently")]
    fn revoke_token(&self, Parameters(params): Parameters<TokenIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx = crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::revoke_token(&ctx, &params.token_id) {
            Ok(()) => serde_json::json!({"status": "ok", "revoked": params.token_id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Rotate a token's secret. Returns new secret (old becomes invalid).")]
    fn rotate_token(&self, Parameters(params): Parameters<TokenIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx = crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::rotate_token(&ctx, &params.token_id) {
            Ok(new_secret) => serde_json::json!({
                "token_id": params.token_id,
                "new_secret": new_secret,
                "warning": "Store the new secret securely"
            }).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Audit tools ---

    #[tool(description = "Get recent MCP audit log entries")]
    fn get_audit_log(&self, Parameters(params): Parameters<AuditLogParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx = crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        let limit = params.limit.unwrap_or(50).min(500);
        match crate::services::tokens::get_recent_audit(&ctx, limit) {
            Ok(entries) => {
                let result: Vec<serde_json::Value> = entries.iter().map(|e| {
                    serde_json::json!({
                        "id": e.id, "token_id": e.token_id,
                        "tool_name": e.tool_name, "result_status": e.result_status,
                        "timestamp": e.timestamp,
                    })
                }).collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Delete old audit log entries. Returns count deleted.")]
    fn prune_audit_log(&self, Parameters(params): Parameters<PruneAuditParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx = crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        let days = params.retention_days.unwrap_or(30);
        match crate::services::tokens::prune_audit_log(&ctx, days) {
            Ok(deleted) => serde_json::json!({"deleted": deleted, "retention_days": days}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    // --- Export tools ---

    #[tool(description = "List available export presets (platforms, sizes, formats)")]
    fn list_export_presets(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let presets = crate::services::export::list_presets();
        serde_json::to_string(&presets).unwrap_or_else(|_| "[]".to_string())
    }
}

impl ServerHandler for ImageViewMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("ImageView MCP server — browse, curate, and manage an AI art image library")
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, ErrorData> {
        let tools = self.tool_router.list_all();
        eprintln!("MCP list_tools: returning {} tools", tools.len());
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
            let params_json = request.arguments.as_ref()
                .and_then(|args| serde_json::to_string(args).ok());

            if let Err(msg) = require_capability(&self.auth, &tool_name) {
                self.log_tool_call(&tool_name, None, "denied");
                return Err(ErrorData::invalid_request(msg, None));
            }

            let call_context = ToolCallContext::new(self, request, context);
            let result = self.tool_router.call(call_context).await;

            let status = match &result {
                Err(_) => "error",
                Ok(r) => {
                    if r.is_error.unwrap_or(false) { "error" } else { "ok" }
                }
            };
            self.log_tool_call(&tool_name, params_json.as_deref(), status);

            result
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::db_core::models::{McpToken, TokenScope};
    use crate::services::tokens;
    use super::AuthContext;

    // --- Path redaction (tests production `redact_path`) ---

    #[test]
    fn test_redact_path_extracts_filename() {
        assert_eq!(super::redact_path("/Users/gleb/art/midjourney/image_001.png"), "image_001.png");
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
        assert_eq!(super::redact_path("/Users/gleb/My Art/image 001.png"), "image 001.png");
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
        assert!(tokens::image_in_scope(&scope, "/art/midjourney/img1.png", &[]));
        assert!(tokens::image_in_scope(&scope, "/art/midjourney/sub/img2.png", &[]));
        assert!(!tokens::image_in_scope(&scope, "/art/dalle/img3.png", &[]));
        assert!(!tokens::image_in_scope(&scope, "/photos/vacation.jpg", &[]));
    }

    #[test]
    fn test_scope_multiple_folders() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art/midjourney".to_string(), "/art/dalle".to_string()]),
            collections: None,
            tags: None,
        });
        assert!(tokens::image_in_scope(&scope, "/art/midjourney/img.png", &[]));
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
        assert!(tokens::image_in_scope(&scope, "/any/path.jpg", &["col_abc".to_string()]));
        assert!(tokens::image_in_scope(&scope, "/any/path.jpg", &["col_def".to_string()]));
        assert!(!tokens::image_in_scope(&scope, "/any/path.jpg", &["col_xyz".to_string()]));
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
        assert!(tokens::image_in_scope(&scope, "/photos/img.png", &["col_abc".to_string()]));
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
        assert!(super::is_valid_decision("selected"));
        assert!(super::is_valid_decision("rejected"));
        assert!(super::is_valid_decision("none"));
    }

    #[test]
    fn test_decision_invalid_values() {
        assert!(!super::is_valid_decision("maybe"));
        assert!(!super::is_valid_decision(""));
        assert!(!super::is_valid_decision("SELECTED"));
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
        assert!(tokens::image_in_scope(&scope, "/any.jpg", &["col_abc".to_string()]));
        assert!(!tokens::image_in_scope(&scope, "/any.jpg", &["col_def".to_string()]));
        assert!(!tokens::image_in_scope(&scope, "/any.jpg", &["col_ghi".to_string()]));
    }

    #[test]
    fn test_no_collection_scope_allows_all() {
        let scope: Option<TokenScope> = None;
        assert!(tokens::image_in_scope(&scope, "/any.jpg", &["col_abc".to_string()]));
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

    // --- Tool capability mapping completeness ---

    #[test]
    fn test_read_tools_map_to_library_read() {
        let read_tools = [
            "list_images", "get_image", "list_folders", "list_folder_images",
            "list_collections", "list_collection_images", "get_library_stats",
            "get_detections", "get_vision_metadata",
        ];
        for tool in &read_tools {
            assert_eq!(
                tokens::tool_capability(tool), "library:read",
                "Tool '{}' should map to library:read", tool
            );
        }
    }

    #[test]
    fn test_search_tools_map_to_library_search() {
        let search_tools = ["find_similar", "search_by_object", "search_images"];
        for tool in &search_tools {
            assert_eq!(
                tokens::tool_capability(tool), "library:search",
                "Tool '{}' should map to library:search", tool
            );
        }
    }

    #[test]
    fn test_curation_tools_map_to_curation_write() {
        let curation_tools = [
            "set_rating", "set_decision", "create_collection",
            "add_to_collection", "delete_collection", "create_smart_collection",
        ];
        for tool in &curation_tools {
            assert_eq!(
                tokens::tool_capability(tool), "curation:write",
                "Tool '{}' should map to curation:write", tool
            );
        }
    }

    #[test]
    fn test_import_tools_map_to_import_write() {
        assert_eq!(tokens::tool_capability("import_folder"), "import:write");
        assert_eq!(tokens::tool_capability("import_files"), "import:write");
    }

    #[test]
    fn test_display_tools_map_to_display_navigate() {
        let display_tools = ["show_image", "navigate_to_folder", "show_collection"];
        for tool in &display_tools {
            assert_eq!(
                tokens::tool_capability(tool), "display:navigate",
                "Tool '{}' should map to display:navigate", tool
            );
        }
    }

    #[test]
    fn test_ai_tools_map_to_ai_run() {
        let ai_tools = ["generate_embeddings", "detect_objects", "analyze_images"];
        for tool in &ai_tools {
            assert_eq!(
                tokens::tool_capability(tool), "ai:run",
                "Tool '{}' should map to ai:run", tool
            );
        }
    }

    #[test]
    fn test_token_tools_map_to_tokens_manage() {
        let token_tools = [
            "create_token", "list_tokens", "revoke_token", "rotate_token",
            "get_audit_log", "prune_audit_log",
        ];
        for tool in &token_tools {
            assert_eq!(
                tokens::tool_capability(tool), "tokens:manage",
                "Tool '{}' should map to tokens:manage", tool
            );
        }
    }

    #[test]
    fn test_admin_only_tools_map_to_settings_manage() {
        let admin_tools = ["rescan_sources", "get_job", "list_jobs", "cancel_job"];
        for tool in &admin_tools {
            assert_eq!(
                tokens::tool_capability(tool), "settings:manage",
                "Tool '{}' should map to settings:manage", tool
            );
        }
    }

    #[test]
    fn test_unknown_tool_maps_to_settings_manage() {
        assert_eq!(tokens::tool_capability("nonexistent_tool"), "settings:manage");
        assert_eq!(tokens::tool_capability(""), "settings:manage");
        assert_eq!(tokens::tool_capability("drop_database"), "settings:manage");
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
