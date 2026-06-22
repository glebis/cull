use super::*;

#[tool_router(router = export_router)]
impl CullMcp {
    #[tool(description = "List available export presets (platforms, sizes, formats)")]
    fn list_export_presets(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let presets = crate::services::export::list_presets();
        serde_json::to_string(&presets).unwrap_or_else(|_| "[]".to_string())
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
                        Err(e) => return error_for_mcp(&e.to_string(), &self.auth),
                    }
                }
            }
        }

        match crate::services::export::export_images(&state.db, &state.app_data_dir, params) {
            Ok(result) => {
                let value = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}));
                if self.is_remote() {
                    remote_safe_publish_value(value).to_string()
                } else {
                    value.to_string()
                }
            }
            Err(e) => error_for_mcp(&e, &self.auth),
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
            Ok(result) => {
                let value = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}));
                if self.is_remote() {
                    remote_safe_publish_value(value).to_string()
                } else {
                    value.to_string()
                }
            }
            Err(e) => error_for_mcp(&e, &self.auth),
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
            Err(e) => return error_for_mcp(&e.to_string(), &self.auth),
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
                    match self.check_image_id_scope(&image.image.id) {
                        Ok(true) => {}
                        Ok(false) => {
                            return format!(
                                "Error: Access denied — image '{}' outside token scope",
                                image.image.id
                            )
                        }
                        Err(e) => return format!("Error: {}", e),
                    }
                }
            }
            Err(e) => return format!("Error: {}", e),
        }

        match crate::commands::static_publishing::export_static_publish_canvas_inner(
            state.inner(),
            params,
        ) {
            Ok(result) => {
                let value = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}));
                if self.is_remote() {
                    remote_safe_publish_value(value).to_string()
                } else {
                    value.to_string()
                }
            }
            Err(e) => error_for_mcp(&e, &self.auth),
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
            Err(e) => error_for_mcp(&e, &self.auth),
        }
    }
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::export_router()
}
