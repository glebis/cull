use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo, CallToolRequestParams, CallToolResult},
    schemars, tool, tool_router, ErrorData,
    service::RequestContext, RoleServer,
};
use tauri::Manager;

use crate::AppState;
use crate::db_core::models::TokenScope;
use crate::services::tokens;
use super::auth::{AuthContext, require_capability};

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
        let limit = params.limit.unwrap_or(50).min(100).max(1);
        let scope = self.token_scope();

        match state.db.list_images(limit, offset) {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images.iter()
                    .filter(|img| tokens::image_in_scope(&scope, &img.path, &[]))
                    .map(|img| {
                    serde_json::json!({
                        "id": img.image.id,
                        "path": img.path,
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
                        "path": img.path,
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
                        serde_json::json!({"path": path, "image_count": count})
                    }).collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List all collections with image counts")]
    fn list_collections(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.list_collections() {
            Ok(collections) => {
                let result: Vec<serde_json::Value> = collections.iter().map(|(id, name, count)| {
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
        let limit = params.limit.unwrap_or(50).min(100).max(1);

        match state.db.list_images_by_folder(&params.folder_path, limit, offset) {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images.iter()
                    .filter(|img| tokens::image_in_scope(&scope, &img.path, &[]))
                    .map(|img| {
                    serde_json::json!({
                        "id": img.image.id,
                        "path": img.path,
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
        if params.rating > 5 {
            return "Error: Rating must be 0-5".to_string();
        }
        let state = self.app_handle.state::<AppState>();
        if let Ok(images) = state.db.get_images_by_ids(&[params.image_id.as_str()]) {
            if let Some(img) = images.first() {
                if !self.check_image_scope(&img.path) {
                    return "Error: Access denied — image outside token scope".to_string();
                }
            }
        }
        match state.db.set_rating(&params.image_id, params.rating) {
            Ok(()) => serde_json::json!({"status": "ok", "image_id": params.image_id, "rating": params.rating}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Set selection decision on an image: 'selected', 'rejected', or 'none'")]
    fn set_decision(&self, Parameters(params): Parameters<SetDecisionParams>) -> String {
        if !matches!(params.decision.as_str(), "selected" | "rejected" | "none") {
            return "Error: Decision must be 'selected', 'rejected', or 'none'".to_string();
        }
        let state = self.app_handle.state::<AppState>();
        if let Ok(images) = state.db.get_images_by_ids(&[params.image_id.as_str()]) {
            if let Some(img) = images.first() {
                if !self.check_image_scope(&img.path) {
                    return "Error: Access denied — image outside token scope".to_string();
                }
            }
        }
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
        let state = self.app_handle.state::<AppState>();
        let refs: Vec<&str> = params.image_ids.iter().map(|s| s.as_str()).collect();
        match state.db.add_to_collection(&params.collection_id, &refs) {
            Ok(()) => serde_json::json!({"status": "ok", "added": params.image_ids.len()}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Delete a collection (does not delete the images)")]
    fn delete_collection(&self, Parameters(params): Parameters<CollectionIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.delete_collection(&params.collection_id) {
            Ok(()) => serde_json::json!({"status": "ok"}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List images in a collection with full metadata")]
    fn list_collection_images(&self, Parameters(params): Parameters<CollectionIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.list_collection_images(&params.collection_id) {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images.iter().map(|img| {
                    serde_json::json!({
                        "id": img.image.id,
                        "path": img.path,
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
        let state = self.app_handle.state::<AppState>();
        let top_k = params.limit.unwrap_or(10) as usize;

        let all = match state.db.get_all_embeddings("clip-vit-b32") {
            Ok(a) => a,
            Err(e) => return format!("Error: {}", e),
        };
        let query = match all.iter().find(|(id, _)| id == &params.image_id) {
            Some(q) => q,
            None => return format!("Error: Image '{}' has no embedding. Run generate_embeddings first.", params.image_id),
        };
        match state.db.find_similar(&query.1, "clip-vit-b32", top_k) {
            Ok(results) => {
                let r: Vec<serde_json::Value> = results.iter().map(|(id, score)| {
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
        let limit = params.limit.unwrap_or(50);
        match state.db.search_by_class(&params.class_name, limit) {
            Ok(results) => {
                let r: Vec<serde_json::Value> = results.iter().map(|(id, confidence)| {
                    serde_json::json!({"image_id": id, "confidence": confidence})
                }).collect();
                serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get object detections for an image (bounding boxes, classes, confidence scores)")]
    fn get_detections(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.get_detections(&params.image_id, None) {
            Ok(detections) => serde_json::to_string(&detections).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get AI vision descriptions for an image (generated by Ollama vision models)")]
    fn get_vision_metadata(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
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

    // --- Display / Navigation tools ---

    #[tool(description = "Open an image in the loupe (fullscreen detail) view on the local app")]
    fn show_image(&self, Parameters(params): Parameters<ShowImageParams>) -> String {
        use tauri::Emitter;
        let params_json = serde_json::json!({
            "focus": params.image_id,
            "view": "loupe",
        });
        match self.app_handle.emit("open-with-params", params_json) {
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
        use tauri::Emitter;
        let params_json = serde_json::json!({
            "folder": params.folder_path,
            "view": "grid",
        });
        match self.app_handle.emit("open-with-params", params_json) {
            Ok(()) => serde_json::json!({"status": "ok", "action": "navigated to folder"}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Display a collection in the local app grid view")]
    fn show_collection(&self, Parameters(params): Parameters<CollectionIdParams>) -> String {
        use tauri::Emitter;
        match self.app_handle.emit("navigate-collection", serde_json::json!({"collection_id": params.collection_id})) {
            Ok(()) => serde_json::json!({"status": "ok", "action": "showing collection"}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }
}

impl ServerHandler for ImageViewMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("ImageView MCP server — browse, curate, and manage an AI art image library")
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
        async move {
            let tool_name: &str = &request.name;

            if let Err(msg) = require_capability(&self.auth, tool_name) {
                return Err(ErrorData::invalid_request(msg, None));
            }

            let call_context = ToolCallContext::new(self, request, context);
            self.tool_router.call(call_context).await
        }
    }
}
