use super::*;

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
        description = "List images with pagination. Returns id, path, dimensions, format, rating, decision, prompt, and generation metadata."
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
                let result = images
                    .iter()
                    .map(|img| {
                        let generation_run = state
                            .db
                            .get_generation_run_for_image(&img.image.id)
                            .map_err(|e| e.to_string())?;
                        Ok(image_value_for_mcp(
                            img,
                            generation_run.as_ref(),
                            &self.auth,
                            self.maybe_redact_path(&img.path),
                            false,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>();
                let result = match result {
                    Ok(result) => result,
                    Err(e) => return error_for_mcp(&e, &self.auth),
                };
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => error_for_mcp(&e.to_string(), &self.auth),
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
                    let generation_run = match state.db.get_generation_run_for_image(&img.image.id)
                    {
                        Ok(run) => run,
                        Err(e) => return error_for_mcp(&e.to_string(), &self.auth),
                    };
                    image_value_for_mcp(
                        &img,
                        generation_run.as_ref(),
                        &self.auth,
                        self.maybe_redact_path(&img.path),
                        true,
                    )
                    .to_string()
                }
                None => format!("Error: Image '{}' not found", params.image_id),
            },
            Err(e) => error_for_mcp(&e.to_string(), &self.auth),
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

    #[tool(
        description = "List images in a specific folder with pagination, prompts, and generation metadata"
    )]
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
                let result = images
                    .iter()
                    .filter(|img| tokens::image_in_scope(&scope, &img.path, &[]))
                    .map(|img| {
                        let generation_run = state
                            .db
                            .get_generation_run_for_image(&img.image.id)
                            .map_err(|e| e.to_string())?;
                        Ok(image_value_for_mcp(
                            img,
                            generation_run.as_ref(),
                            &self.auth,
                            self.maybe_redact_path(&img.path),
                            false,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>();
                let result = match result {
                    Ok(result) => result,
                    Err(e) => return error_for_mcp(&e, &self.auth),
                };
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => error_for_mcp(&e.to_string(), &self.auth),
        }
    }
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::library_router()
}
