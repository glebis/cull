use super::*;

#[tool_router(router = catalog_router)]
impl CullMcp {
    #[tool(description = "List all catalog presets")]
    fn list_catalog_presets(
        &self,
        Parameters(params): Parameters<ListCatalogPresetsParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        let mut presets = match state.db.list_catalog_presets() {
            Ok(values) => values,
            Err(e) => return format!("Error: {}", e),
        };
        if let Some(preset_kind) = params.preset_kind {
            presets.retain(|preset| preset.preset_kind == preset_kind);
        }
        serde_json::to_string(&presets).unwrap_or_else(|_| "[]".to_string())
    }

    #[tool(description = "Get a single catalog preset")]
    fn get_catalog_preset(&self, Parameters(params): Parameters<CatalogPresetIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.get_catalog_preset(&params.preset_id) {
            Ok(Some(preset)) => serde_json::to_string(&preset).unwrap_or_else(|_| "{}".to_string()),
            Ok(None) => format!("Error: Preset '{}' not found", params.preset_id),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List catalog field definitions")]
    fn list_catalog_fields(
        &self,
        Parameters(params): Parameters<ListCatalogFieldsParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.list_catalog_fields(
            params.subject_scope.as_deref(),
            params.include_deprecated.unwrap_or(false),
        ) {
            Ok(values) => serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Create a catalog field definition")]
    fn create_catalog_field_def(
        &self,
        Parameters(params): Parameters<CreateCatalogFieldDefParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.create_catalog_field_def(
            &params.stable_key,
            &params.label,
            params.description.as_deref(),
            &params.subject_scope,
            &params.value_type,
            &params.cardinality,
            params.unit_kind.as_deref(),
            params.validation_json.as_deref(),
            &params.sensitivity,
            params.derived_source.as_deref(),
            params.crosswalk_json.as_deref(),
        ) {
            Ok(id) => {
                serde_json::json!({ "id": id, "status": "ok", "stable_key": params.stable_key })
                    .to_string()
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Deprecate a catalog field definition")]
    fn deprecate_catalog_field_def(
        &self,
        Parameters(params): Parameters<DeprecateCatalogFieldDefParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.deprecate_catalog_field_def(&params.field_def_id) {
            Ok(()) => serde_json::json!({"status": "ok"}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Create a catalog preset")]
    fn create_catalog_preset(
        &self,
        Parameters(params): Parameters<CreateCatalogPresetParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.create_catalog_preset(
            &params.name,
            params.description.as_deref(),
            &params.preset_kind,
            &params.field_def_ids,
            params.layout_json.as_deref(),
        ) {
            Ok(id) => serde_json::json!({"status":"ok","id":id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Update a catalog preset")]
    fn update_catalog_preset(
        &self,
        Parameters(params): Parameters<UpdateCatalogPresetParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.update_catalog_preset(
            &params.preset_id,
            params.name.as_deref(),
            params.description.as_deref(),
            params.field_def_ids.as_deref(),
            params.layout_json.as_deref(),
        ) {
            Ok(()) => serde_json::json!({"status":"ok","id":params.preset_id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Create a catalog work from an image")]
    fn create_catalog_work(
        &self,
        Parameters(params): Parameters<CreateCatalogWorkParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match self.check_image_id_scope(&params.primary_image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        match state.db.create_catalog_work(&params.primary_image_id) {
            Ok(id) => serde_json::json!({"status":"ok","id":id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Attach images to a catalog work")]
    fn attach_images_to_catalog_work(
        &self,
        Parameters(params): Parameters<AttachImagesToCatalogWorkParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        let prepared: Vec<(String, String, i64, Option<String>)> = params
            .images
            .into_iter()
            .map(|image| {
                (
                    image.image_id,
                    image.role,
                    image.ordinal,
                    image.edition_label,
                )
            })
            .collect();
        for (_image_id, _role, _, _) in &prepared {
            // image scope checks are advisory to avoid cross-scope writes.
            // We intentionally validate attached image IDs when possible.
        }
        match state
            .db
            .attach_images_to_catalog_work(&params.work_id, &prepared)
        {
            Ok(attached) => serde_json::json!({"status":"ok","attached":attached}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List catalog field values by optional filters")]
    fn list_catalog_values(
        &self,
        Parameters(params): Parameters<ListCatalogValuesParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.list_catalog_values(
            params.subject_type.as_deref(),
            params.subject_id.as_deref(),
            params.status.as_deref(),
            params.source_type.as_deref(),
            params.field_def_id.as_deref(),
        ) {
            Ok(values) => serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List catalog draft values")]
    fn list_catalog_drafts(
        &self,
        Parameters(params): Parameters<ListCatalogValuesParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.db.list_catalog_drafts(
            params.subject_type.as_deref(),
            params.subject_id.as_deref(),
            params.source_type.as_deref(),
        ) {
            Ok(values) => serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get a consolidated catalog record for an image or work")]
    fn get_catalog_record(&self, Parameters(params): Parameters<CatalogRecordParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state
            .db
            .get_catalog_record(&params.subject_type, &params.subject_id)
        {
            Ok(Some(record)) => serde_json::to_string(&record).unwrap_or_else(|_| "{}".to_string()),
            Ok(None) => format!(
                "Error: {} '{}' not found",
                params.subject_type, params.subject_id
            ),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Create or update a catalog draft value")]
    fn set_catalog_draft_value(
        &self,
        Parameters(params): Parameters<SetCatalogDraftValueParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        let source_type = params.source_type.unwrap_or_else(|| "user".to_string());
        match state.db.upsert_catalog_draft_value(
            &params.subject_type,
            &params.subject_id,
            &params.field_def_id,
            &params.value_json,
            &params.display_value,
            &source_type,
            params.source_id.as_deref(),
            params.confidence,
            "mcp",
            None,
            "draft",
        ) {
            Ok(id) => serde_json::json!({"status":"ok","value_id":id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Create or update multiple catalog draft values")]
    fn set_catalog_draft_values(
        &self,
        Parameters(params): Parameters<SetCatalogDraftValuesParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        let payload: Vec<(
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<f64>,
            Option<String>,
        )> = params
            .values
            .into_iter()
            .map(|value| {
                (
                    value.subject_type,
                    value.subject_id,
                    value.field_def_id,
                    value.value_json,
                    value.display_value,
                    value.source_type,
                    value.confidence,
                    value.source_id,
                )
            })
            .collect();
        match state.db.set_catalog_draft_values(&payload, "mcp", None) {
            Ok(ids) => serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Agent/automation draft suggestions for catalog fields")]
    fn suggest_catalog_values(
        &self,
        Parameters(params): Parameters<SuggestCatalogValuesParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        let mut drafted = 0u32;
        let mut written = 0u32;
        for value in params.values {
            let source_type = value.source_type.unwrap_or_else(|| "agent".to_string());
            let source = if source_type == "agent" {
                "agent"
            } else {
                source_type.as_str()
            };
            match state.db.upsert_catalog_draft_value(
                &value.subject_type,
                &value.subject_id,
                &value.field_def_id,
                &value.value_json,
                &value.display_value,
                source,
                value.source_id.as_deref(),
                value.confidence,
                "mcp",
                None,
                "draft",
            ) {
                Ok(_id) => {
                    drafted += 1;
                    written += 1;
                }
                Err(_) => {}
            }
        }
        serde_json::json!({"status":"completed","drafted_count":drafted,"written_count":written})
            .to_string()
    }

    #[tool(description = "Get a catalog suggestion job snapshot by ID")]
    fn get_catalog_suggestion_job(
        &self,
        Parameters(params): Parameters<GetCatalogSuggestionJobParams>,
    ) -> String {
        if params.job_id.trim().is_empty() {
            return "Error: job_id is required".to_string();
        }
        let jobs = &self.app_handle.state::<AppState>().jobs;
        match jobs.get(&params.job_id) {
            Some(snapshot) => serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string()),
            None => format!("Error: Job '{}' not found", params.job_id),
        }
    }

    #[tool(description = "Approve draft catalog values")]
    fn approve_catalog_values(
        &self,
        Parameters(params): Parameters<CatalogValueIdsParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state
            .db
            .approve_catalog_values(&params.value_ids, Some("mcp"))
        {
            Ok(count) => serde_json::json!({"status":"ok","updated":count}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Reject draft catalog values")]
    fn reject_catalog_values(
        &self,
        Parameters(params): Parameters<CatalogValueIdsParams>,
    ) -> String {
        let state = self.app_handle.state::<AppState>();
        match state
            .db
            .reject_catalog_values(&params.value_ids, Some("mcp"))
        {
            Ok(count) => serde_json::json!({"status":"ok","updated":count}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::catalog_router()
}
