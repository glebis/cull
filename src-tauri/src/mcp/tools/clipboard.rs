use super::*;

#[tool_router(router = clipboard_router)]
impl CullMcp {
    #[tool(description = "Get Clipboard Monitor status and active collection ID")]
    fn get_clipboard_monitor_status(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let monitor = state.clipboard_monitor.lock();
        clipboard_monitor_status_for_mcp(
            monitor.running,
            monitor.collection_id.clone(),
            monitor.collection_name.clone(),
            monitor.capture_dir.clone(),
            monitor.captured_count,
            monitor.last_error.clone(),
            &self.auth,
        )
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
                    Err(e) => return error_for_mcp(&e.to_string(), &self.auth),
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
                Err(e) => return error_for_mcp(&e, &self.auth),
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
            Err(e) => return error_for_mcp(&e, &self.auth),
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
        if self.is_remote() {
            return remote_safe_publish_value(result).to_string();
        }
        result.to_string()
    }

    #[tool(description = "Return the last successful Clipboard Monitor publish result")]
    fn get_last_clipboard_publish(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let raw = state
            .db
            .get_setting("clipboard_monitor_last_publish")
            .ok()
            .flatten()
            .unwrap_or_else(|| serde_json::json!({"status":"none"}).to_string());
        if !self.is_remote() {
            return raw;
        }
        match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(value) => remote_safe_publish_value(value).to_string(),
            Err(_) => serde_json::json!({"status":"unavailable"}).to_string(),
        }
    }
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::clipboard_router()
}
