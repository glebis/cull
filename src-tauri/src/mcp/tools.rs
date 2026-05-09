use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_router,
};
use tauri::Manager;

use crate::AppState;

#[derive(Debug, Clone)]
pub struct ImageViewMcp {
    pub app_handle: tauri::AppHandle,
    tool_router: ToolRouter<Self>,
}

impl ImageViewMcp {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            app_handle,
            tool_router: Self::tool_router(),
        }
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

        match state.db.list_images(limit, offset) {
            Ok(images) => {
                let result: Vec<serde_json::Value> = images.iter().map(|img| {
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
                Some(img) => serde_json::json!({
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
                }).to_string(),
                None => format!("Error: Image '{}' not found", params.image_id),
            },
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List all imported folders with image counts")]
    fn list_folders(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.list_folders() {
            Ok(folders) => {
                let result: Vec<serde_json::Value> = folders.iter().map(|(path, count)| {
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
        let state = self.app_handle.state::<AppState>();
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(50).min(100).max(1);

        match state.db.list_images_by_folder(&params.folder_path, limit, offset) {
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
}

impl ServerHandler for ImageViewMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("ImageView MCP server — browse, curate, and manage an AI art image library")
    }
}
