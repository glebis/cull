use super::*;

#[tool_router(router = sessions_router)]
impl CullMcp {
    #[tool(
        description = "Capture the currently visible Cull view as raw.png, annotated.png, and manifest.json for multimodal analysis. Local stdio only in v1."
    )]
    async fn capture_current_view_snapshot(
        &self,
        Parameters(params): Parameters<CaptureCurrentViewSnapshotParams>,
    ) -> String {
        if let Err(e) = self.require_local_agent_snapshot_tool("capture_current_view_snapshot") {
            return format!("Error: {}", e);
        }

        let state = self.app_handle.state::<AppState>();
        let request_id = format!("req_{}", uuid::Uuid::new_v4().simple());
        let snapshot_id = format!("snap_{}", uuid::Uuid::new_v4().simple());
        let (sender, receiver) = tokio::sync::oneshot::channel::<
            crate::services::agent_snapshots::AgentSnapshotPackage,
        >();
        state
            .agent_snapshot_requests
            .lock()
            .insert(request_id.clone(), sender);
        let payload = serde_json::json!({
            "request_id": request_id.clone(),
            "snapshot_id": snapshot_id,
            "clipboard": params.clipboard.unwrap_or(false),
            "capture_reason": "mcp",
        });

        if let Err(e) = self.app_handle.emit("agent-view-snapshot:request", payload) {
            state.agent_snapshot_requests.lock().remove(&request_id);
            return format!("Error: Failed to request frontend snapshot: {}", e);
        }

        match tokio::time::timeout(std::time::Duration::from_secs(15), receiver).await {
            Ok(Ok(package)) => {
                crate::services::agent_snapshots::snapshot_response_value(&package, false)
                    .to_string()
            }
            Ok(Err(_)) => "Error: Agent snapshot request was cancelled".to_string(),
            Err(_) => {
                state.agent_snapshot_requests.lock().remove(&request_id);
                "Error: Timed out waiting for the visible app to capture an agent snapshot"
                    .to_string()
            }
        }
    }

    #[tool(
        description = "Return the latest agent view snapshot manifest and local file paths. Local stdio only in v1."
    )]
    fn get_last_view_snapshot(
        &self,
        Parameters(params): Parameters<GetLastViewSnapshotParams>,
    ) -> String {
        if let Err(e) = self.require_local_agent_snapshot_tool("get_last_view_snapshot") {
            return format!("Error: {}", e);
        }

        let state = self.app_handle.state::<AppState>();
        let registry = state.agent_snapshots.lock();
        let package = match params.snapshot_id.as_deref() {
            Some(snapshot_id) => registry.get_snapshot(snapshot_id),
            None => registry.latest_snapshot(),
        };
        match package {
            Some(package) => {
                crate::services::agent_snapshots::snapshot_response_value(package, false)
                    .to_string()
            }
            None => "null".to_string(),
        }
    }

    #[tool(
        description = "Select visible images by the numbered labels from an agent view snapshot. Local stdio only in v1."
    )]
    fn select_snapshot_labels(
        &self,
        Parameters(params): Parameters<SelectSnapshotLabelsParams>,
    ) -> String {
        if let Err(e) = self.require_local_agent_snapshot_tool("select_snapshot_labels") {
            return format!("Error: {}", e);
        }
        let mode = match normalize_snapshot_selection_mode(params.mode.as_deref()) {
            Ok(mode) => mode,
            Err(e) => return format!("Error: {}", e),
        };
        let state = self.app_handle.state::<AppState>();
        let package = {
            let registry = state.agent_snapshots.lock();
            match params.snapshot_id.as_deref() {
                Some(snapshot_id) => registry.get_snapshot(snapshot_id).cloned(),
                None => registry.latest_snapshot().cloned(),
            }
        };
        let Some(package) = package else {
            return "Error: No captured agent snapshot is available".to_string();
        };

        let image_ids = match crate::services::agent_snapshots::image_ids_for_snapshot_labels(
            &package.manifest,
            &params.labels,
        ) {
            Ok(ids) => ids,
            Err(e) => return format!("Error: {}", e),
        };
        if let Err(e) = self.validate_snapshot_image_ids_exist(&state, &image_ids) {
            return format!("Error: {}", e);
        }
        if let Err(e) = self.emit_agent_snapshot_selection(
            image_ids.clone(),
            mode,
            params.focus_first.unwrap_or(true),
        ) {
            return format!("Error: {}", e);
        }
        serde_json::json!({
            "status": "ok",
            "snapshot_id": package.snapshot_id,
            "selected": image_ids.len(),
            "image_ids": image_ids,
            "mode": mode,
        })
        .to_string()
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
            Ok(Some(canvas)) => match canvas_layout_for_mcp_auth(&canvas, &self.auth) {
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
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::sessions_router()
}
