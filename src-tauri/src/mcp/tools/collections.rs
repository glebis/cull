use super::*;

#[tool_router(router = collections_router)]
impl CullMcp {
    #[tool(description = "List all collections with image counts")]
    fn list_collections(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let scoped_counts = match self.scoped_collection_counts(&state) {
            Ok(counts) => counts,
            Err(e) => return format!("Error: {}", e),
        };
        match state.db.list_collections() {
            Ok(collections) => {
                let result = collection_summaries_for_mcp(collections, scoped_counts.as_ref());
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
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
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::collections_router()
}
