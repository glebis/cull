use super::*;

#[tool_router(router = curation_router)]
impl CullMcp {
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

    #[tool(
        description = "Select image IDs that are visible in the latest agent view snapshot. Local stdio only in v1."
    )]
    fn select_images_in_view(
        &self,
        Parameters(params): Parameters<SelectImagesInViewParams>,
    ) -> String {
        if let Err(e) = self.require_local_agent_snapshot_tool("select_images_in_view") {
            return format!("Error: {}", e);
        }
        let mode = match normalize_snapshot_selection_mode(params.mode.as_deref()) {
            Ok(mode) => mode,
            Err(e) => return format!("Error: {}", e),
        };
        let state = self.app_handle.state::<AppState>();
        let package = { state.agent_snapshots.lock().latest_snapshot().cloned() };
        let Some(package) = package else {
            return "Error: No captured agent snapshot is available".to_string();
        };
        let visible_ids: std::collections::BTreeSet<String> = package
            .manifest
            .get("visible_images")
            .and_then(serde_json::Value::as_array)
            .map(|images| {
                images
                    .iter()
                    .filter_map(|image| {
                        image
                            .get("image_id")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default();
        for image_id in &params.image_ids {
            if !visible_ids.contains(image_id) {
                return format!(
                    "Error: Image '{}' is not visible in the latest agent snapshot",
                    image_id
                );
            }
        }
        if let Err(e) = self.validate_snapshot_image_ids_exist(&state, &params.image_ids) {
            return format!("Error: {}", e);
        }
        if let Err(e) = self.emit_agent_snapshot_selection(
            params.image_ids.clone(),
            mode,
            params.focus_first.unwrap_or(true),
        ) {
            return format!("Error: {}", e);
        }
        serde_json::json!({
            "status": "ok",
            "snapshot_id": package.snapshot_id,
            "selected": params.image_ids.len(),
            "image_ids": params.image_ids,
            "mode": mode,
        })
        .to_string()
    }
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::curation_router()
}
