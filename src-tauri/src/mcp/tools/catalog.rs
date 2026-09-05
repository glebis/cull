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
        let image_ids: Vec<String> = prepared
            .iter()
            .map(|(image_id, _, _, _)| image_id.clone())
            .collect();
        // Enforce scope for the target work and EVERY attached image before any
        // mutation; a mixed batch is rejected atomically with no partial attach.
        if let Err(msg) = ensure_attach_images_scope_for_db(
            &state.db,
            &self.token_scope(),
            &params.work_id,
            &image_ids,
        ) {
            return format!("Error: {}", msg);
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

/// DB-backed authorization seam for `attach_images_to_catalog_work`.
///
/// Reuses the shared per-image scope semantics (`tokens::image_id_in_scope`, the
/// same DB-backed check every other image-id tool goes through): a scoped agent
/// may only attach images it can already see. The target work is authorized via
/// its primary image — the same anchor `create_catalog_work` enforces — so an
/// agent cannot smuggle images into a work outside its scope, and cannot
/// smuggle out-of-scope images into a work inside it. There is no per-work
/// ownership metadata in v1; this reuses existing scope tables only.
///
/// All attached IDs and the target work are authorized BEFORE any mutation, so
/// a mixed batch is rejected atomically: no partial attach ever lands. Unknown
/// image IDs (no DB row) and DB errors fail closed for scoped tokens.
pub(crate) fn ensure_attach_images_scope_for_db(
    db: &crate::db_core::db::Database,
    scope: &Option<TokenScope>,
    work_id: &str,
    image_ids: &[String],
) -> Result<(), String> {
    let work = db
        .get_catalog_work(work_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Work '{}' not found", work_id))?;
    match crate::services::tokens::image_id_in_scope(db, scope, &work.primary_image_id) {
        Ok(true) => {}
        Ok(false) => {
            return Err(format!(
                "Access denied — work '{}' is outside token scope",
                work_id
            ));
        }
        Err(e) => return Err(e),
    }
    for image_id in image_ids {
        match crate::services::tokens::image_id_in_scope(db, scope, image_id) {
            Ok(true) => {}
            Ok(false) => {
                return Err(format!(
                    "Access denied — image '{}' is outside token scope",
                    image_id
                ));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::catalog_router()
}

#[cfg(test)]
mod tests {
    use super::ensure_attach_images_scope_for_db;
    use crate::db_core::db::Database;
    use crate::db_core::models::{Image, ImageFile, TokenScope};

    fn open_db() -> Database {
        Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    fn insert_image_with_path(db: &Database, root: &std::path::Path, image_id: &str, path: &str) {
        let path = root.join(path.trim_start_matches('/'));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"fixture").unwrap();
        let path = path.canonicalize().unwrap();
        db.insert_image(&Image {
            id: image_id.to_string(),
            sha256_hash: format!("hash-{image_id}"),
            width: 1,
            height: 1,
            format: "png".to_string(),
            file_size: 1,
            created_at: "2026-09-01T10:00:00Z".to_string(),
            imported_at: "2026-09-01T10:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: format!("file-{image_id}"),
            image_id: image_id.to_string(),
            path: path.to_string_lossy().into_owned(),
            last_seen_at: "2026-09-01T10:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        })
        .unwrap();
    }

    fn folder_scope(root: &std::path::Path, folders: &[&str]) -> Option<TokenScope> {
        Some(TokenScope {
            folders: Some(
                folders
                    .iter()
                    .map(|folder| {
                        root.join(folder.trim_start_matches('/'))
                            .canonicalize()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect(),
            ),
            collections: None,
            tags: None,
        })
    }

    fn collection_scope(collection_ids: &[&str]) -> Option<TokenScope> {
        Some(TokenScope {
            folders: None,
            collections: Some(collection_ids.iter().map(|id| id.to_string()).collect()),
            tags: None,
        })
    }

    #[test]
    fn catalog_creation_rolls_back_when_primary_attachment_fails() {
        let db = open_db();
        let root = tempfile::tempdir().unwrap();
        insert_image_with_path(&db, root.path(), "primary", "/library/primary.png");
        db.conn
            .lock()
            .execute_batch(
                "CREATE TRIGGER reject_primary_attachment BEFORE INSERT ON catalog_work_images
             BEGIN SELECT RAISE(ABORT, 'attachment failure'); END;",
            )
            .unwrap();

        assert!(db.create_catalog_work("primary").is_err());
        assert!(db.list_catalog_works().unwrap().is_empty());
    }

    #[test]
    fn attach_scope_rejects_mixed_batch_before_any_writes() {
        let db = open_db();
        let root = tempfile::tempdir().unwrap();
        insert_image_with_path(&db, root.path(), "in-scope", "/library/in-a.png");
        insert_image_with_path(&db, root.path(), "out-of-scope", "/private/out.png");
        let work_id = db.create_catalog_work("in-scope").unwrap();
        let scope = folder_scope(root.path(), &["/library"]);

        let error = ensure_attach_images_scope_for_db(
            &db,
            &scope,
            &work_id,
            &["in-scope".to_string(), "out-of-scope".to_string()],
        )
        .unwrap_err();

        assert!(
            error.contains("out-of-scope") && error.contains("outside token scope"),
            "unexpected error: {error}"
        );
        // Atomic rejection: only the primary row from create_catalog_work
        // exists — the in-scope member of the mixed batch was not attached.
        assert_eq!(db.list_catalog_work_images(&work_id).unwrap().len(), 1);
    }

    #[test]
    fn attach_scope_allows_fully_in_scope_batch() {
        let db = open_db();
        let root = tempfile::tempdir().unwrap();
        insert_image_with_path(&db, root.path(), "primary", "/library/primary.png");
        insert_image_with_path(&db, root.path(), "detail-a", "/library/detail-a.png");
        insert_image_with_path(&db, root.path(), "detail-b", "/library/sub/detail-b.png");
        let work_id = db.create_catalog_work("primary").unwrap();
        let scope = folder_scope(root.path(), &["/library"]);

        ensure_attach_images_scope_for_db(
            &db,
            &scope,
            &work_id,
            &["detail-a".to_string(), "detail-b".to_string()],
        )
        .unwrap();

        // The production handler proceeds to the real mutation once the seam
        // authorizes the batch; both attachments land.
        db.attach_images_to_catalog_work(
            &work_id,
            &[
                ("detail-a".to_string(), "detail".to_string(), 1, None),
                ("detail-b".to_string(), "detail".to_string(), 2, None),
            ],
        )
        .unwrap();
        assert_eq!(db.list_catalog_work_images(&work_id).unwrap().len(), 3);
    }

    #[test]
    fn attach_scope_requires_work_inside_token_scope() {
        let db = open_db();
        let root = tempfile::tempdir().unwrap();
        insert_image_with_path(&db, root.path(), "private-primary", "/private/primary.png");
        insert_image_with_path(&db, root.path(), "in-scope", "/library/in-a.png");
        // Work exists (e.g. created locally by the desktop user), but its
        // primary image is outside the agent's folder scope.
        let work_id = db.create_catalog_work("private-primary").unwrap();
        let scope = folder_scope(root.path(), &["/library"]);

        let error =
            ensure_attach_images_scope_for_db(&db, &scope, &work_id, &["in-scope".to_string()])
                .unwrap_err();

        assert!(
            error.contains("outside token scope") && error.contains(work_id.as_str()),
            "unexpected error: {error}"
        );
        assert_eq!(db.list_catalog_work_images(&work_id).unwrap().len(), 1);
    }

    #[test]
    fn attach_scope_fail_closed_for_unknown_image_in_batch() {
        let db = open_db();
        let root = tempfile::tempdir().unwrap();
        insert_image_with_path(&db, root.path(), "in-scope", "/library/in-a.png");
        let work_id = db.create_catalog_work("in-scope").unwrap();
        let scope = folder_scope(root.path(), &["/library"]);

        // A scoped token can never attach an ID with no DB row: unknown
        // images are treated as out of scope, not silently skipped.
        let error =
            ensure_attach_images_scope_for_db(&db, &scope, &work_id, &["ghost".to_string()])
                .unwrap_err();

        assert!(error.contains("ghost"), "unexpected error: {error}");
        assert_eq!(db.list_catalog_work_images(&work_id).unwrap().len(), 1);
    }

    #[test]
    fn attach_scope_rejects_unknown_work() {
        let db = open_db();
        let root = tempfile::tempdir().unwrap();
        insert_image_with_path(&db, root.path(), "in-scope", "/library/in-a.png");
        let scope = folder_scope(root.path(), &["/library"]);

        let error =
            ensure_attach_images_scope_for_db(&db, &scope, "cw_missing", &["in-scope".to_string()])
                .unwrap_err();

        assert!(
            error.contains("Work 'cw_missing' not found"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attach_scope_honors_collection_membership() {
        let db = open_db();
        let root = tempfile::tempdir().unwrap();
        insert_image_with_path(
            &db,
            root.path(),
            "by-collection",
            "/anywhere/by-collection.png",
        );
        insert_image_with_path(&db, root.path(), "unlisted", "/anywhere/unlisted.png");
        let collection_id = db
            .create_collection_with_images("Shared", &["by-collection"])
            .unwrap();
        let work_id = db.create_catalog_work("by-collection").unwrap();
        let scope = collection_scope(&[&collection_id]);

        ensure_attach_images_scope_for_db(&db, &scope, &work_id, &["by-collection".to_string()])
            .unwrap();

        let error =
            ensure_attach_images_scope_for_db(&db, &scope, &work_id, &["unlisted".to_string()])
                .unwrap_err();
        assert!(
            error.contains("unlisted") && error.contains("outside token scope"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attach_scope_unscoped_token_keeps_local_behavior() {
        let db = open_db();
        let root = tempfile::tempdir().unwrap();
        insert_image_with_path(&db, root.path(), "anywhere", "/private/anywhere.png");
        let work_id = db.create_catalog_work("anywhere").unwrap();

        // Local (and plugin) contexts have no token scope; authorization is
        // capability-gated upstream, so the seam must not add folder limits.
        ensure_attach_images_scope_for_db(&db, &None, &work_id, &["anywhere".to_string()]).unwrap();
    }
}
