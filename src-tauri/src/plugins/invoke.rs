// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! The single privileged path for plugins: `plugin_invoke(plugin_id, tool, args)`.
//!
//! A plugin is a locally-installed actor with a capability set — exactly what
//! an MCP token is — so enforcement reuses the same code path MCP calls use:
//! `tokens::tool_capability` + `require_capability` via an
//! `AuthContext::Plugin`, never a parallel webview-side check. Every call
//! (allowed or denied) is written to `mcp_audit_log` with actor
//! `plugin:<id>`, inheriting redaction and retention.
//!
//! Module-gated tools (e.g. `export_static_publish_package`) additionally
//! require the plugin to hold the matching `module:<key>` grant: installing a
//! plugin that declares the module permission substitutes for the raw module
//! setting — plugin presence IS the gate (Track C3).

use crate::mcp::auth::{require_capability, AuthContext};
use crate::services::{tokens, ServiceContext};
use crate::AppState;
use serde::de::DeserializeOwned;

/// Capability-check + audit + dispatch core of the `plugin_invoke` command.
/// Takes the full `AppState` (constructible in tests — see
/// `commands::static_publishing::tests::test_state` for the same pattern) so
/// dispatch can reach heavyweight backends like the static-publishing
/// exporter.
pub fn plugin_invoke_inner(
    state: &AppState,
    plugin_id: &str,
    tool: &str,
    args: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let ctx = ServiceContext::from_app_state(state, None);
    let capabilities = ctx
        .db
        .granted_plugin_capabilities(plugin_id)
        .map_err(|e| e.to_string())?;
    let auth = AuthContext::for_plugin(plugin_id, capabilities.clone());
    let actor = auth
        .token_id()
        .expect("plugin auth context always has an actor")
        .to_string();
    let params_json = args.as_ref().and_then(|a| serde_json::to_string(a).ok());

    // Same enforcement code path as MCP tool calls: tokens::tool_capability
    // resolves the required capability; require_capability produces the same
    // error shape.
    if let Err(msg) = require_capability(&auth, tool) {
        let _ = tokens::log_audit(&ctx, Some(&actor), tool, params_json.as_deref(), "denied");
        return Err(msg);
    }

    // Module-gated tools additionally require the matching module:<key>
    // grant. The grant is recorded at install from the consented manifest,
    // so plugin presence replaces the raw module setting.
    if let Some(required_grant) = required_module_grant(tool) {
        if !capabilities.iter().any(|c| c == &required_grant) {
            let msg = format!("Permission denied: '{tool}' requires '{required_grant}' permission");
            let _ = tokens::log_audit(&ctx, Some(&actor), tool, params_json.as_deref(), "denied");
            return Err(msg);
        }
    }

    let result = dispatch(state, &actor, tool, args.as_ref());
    let status = match &result {
        Ok(_) => "ok",
        Err(DispatchError::Unsupported) => "unsupported",
        Err(DispatchError::Failed(_)) => "error",
    };
    let _ = tokens::log_audit(&ctx, Some(&actor), tool, params_json.as_deref(), status);

    result.map_err(|e| match e {
        DispatchError::Unsupported => {
            format!("Tool '{tool}' is not available through the plugin runtime (v1 whitelist)")
        }
        DispatchError::Failed(msg) => msg,
    })
}

/// `module:<key>` plugin permission required for a module-gated tool, derived
/// from the same `required_module_for_tool` table MCP uses (setting key
/// `module_static_publishing` -> grant `module:static-publishing`).
fn required_module_grant(tool: &str) -> Option<String> {
    let setting_key = crate::mcp::tools::required_module_for_tool(tool)?;
    let key = setting_key.strip_prefix("module_").unwrap_or(setting_key);
    Some(format!("module:{}", key.replace('_', "-")))
}

fn parse_args<T: DeserializeOwned>(
    tool: &str,
    args: Option<&serde_json::Value>,
) -> Result<T, DispatchError> {
    serde_json::from_value(args.cloned().unwrap_or_else(|| serde_json::json!({})))
        .map_err(|e| DispatchError::Failed(format!("invalid {tool} arguments: {e}")))
}

#[derive(serde::Deserialize)]
struct CatalogPresetIdArgs {
    preset_id: String,
}

#[derive(Default, serde::Deserialize)]
struct ListCatalogPresetsArgs {
    preset_kind: Option<String>,
}

#[derive(Default, serde::Deserialize)]
struct ListCatalogFieldsArgs {
    subject_scope: Option<String>,
    include_deprecated: Option<bool>,
}

#[derive(serde::Deserialize)]
struct CreateCatalogFieldDefArgs {
    stable_key: String,
    label: String,
    description: Option<String>,
    subject_scope: String,
    value_type: String,
    cardinality: String,
    unit_kind: Option<String>,
    validation_json: Option<String>,
    sensitivity: String,
    derived_source: Option<String>,
    crosswalk_json: Option<String>,
}

#[derive(serde::Deserialize)]
struct CatalogFieldDefIdArgs {
    field_def_id: String,
}

#[derive(serde::Deserialize)]
struct CreateCatalogPresetArgs {
    name: String,
    description: Option<String>,
    preset_kind: String,
    field_def_ids: Vec<String>,
    layout_json: Option<String>,
}

#[derive(serde::Deserialize)]
struct UpdateCatalogPresetArgs {
    preset_id: String,
    name: Option<String>,
    description: Option<String>,
    field_def_ids: Option<Vec<String>>,
    layout_json: Option<String>,
}

#[derive(serde::Deserialize)]
struct CreateCatalogWorkArgs {
    primary_image_id: String,
}

#[derive(serde::Deserialize)]
struct AttachImagesToCatalogWorkArgs {
    work_id: String,
    images: Vec<crate::commands::catalog::CatalogWorkImageInput>,
}

#[derive(Default, serde::Deserialize)]
struct ListCatalogValuesArgs {
    subject_type: Option<String>,
    subject_id: Option<String>,
    status: Option<String>,
    source_type: Option<String>,
    field_def_id: Option<String>,
}

#[derive(Default, serde::Deserialize)]
struct ListCatalogDraftsArgs {
    subject_type: Option<String>,
    subject_id: Option<String>,
    source_type: Option<String>,
}

#[derive(serde::Deserialize)]
struct CatalogRecordArgs {
    subject_type: String,
    subject_id: String,
}

#[derive(serde::Deserialize)]
struct SetCatalogDraftValueArgs {
    subject_type: String,
    subject_id: String,
    field_def_id: String,
    value_json: String,
    display_value: String,
    source_type: Option<String>,
    source_id: Option<String>,
    confidence: Option<f64>,
}

#[derive(serde::Deserialize)]
struct CatalogDraftValuesArgs {
    values: Vec<crate::commands::catalog::CatalogDraftValueInput>,
}

#[derive(serde::Deserialize)]
struct CatalogValueIdsArgs {
    value_ids: Vec<String>,
}

enum DispatchError {
    /// Tool passed the capability check but is not in the v1 dispatch whitelist.
    Unsupported,
    Failed(String),
}

/// v1 dispatch whitelist. Deliberately small: exactly the ops the
/// `cull-publish` proof plugin needs (Track C3) plus the C1 bootstrap op.
fn dispatch(
    state: &AppState,
    actor: &str,
    tool: &str,
    args: Option<&serde_json::Value>,
) -> Result<serde_json::Value, DispatchError> {
    match tool {
        "list_collections" => {
            let collections = state
                .db
                .list_collections()
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            let items: Vec<serde_json::Value> = collections
                .into_iter()
                .map(|(id, name, count)| {
                    serde_json::json!({ "id": id, "name": name, "image_count": count })
                })
                .collect();
            Ok(serde_json::json!({ "collections": items }))
        }
        "get_library_stats" => {
            let image_count = state
                .db
                .image_count()
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            let folder_count = state
                .db
                .list_folders()
                .map_err(|e| DispatchError::Failed(e.to_string()))?
                .len();
            let collection_count = state
                .db
                .list_collections()
                .map_err(|e| DispatchError::Failed(e.to_string()))?
                .len();
            Ok(serde_json::json!({
                "image_count": image_count,
                "folder_count": folder_count,
                "collection_count": collection_count,
            }))
        }
        "list_collection_images" => {
            let collection_id = args
                .and_then(|a| a.get("collection_id"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    DispatchError::Failed("'collection_id' argument is required".to_string())
                })?;
            let images = state
                .db
                .list_collection_images(collection_id)
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            let items: Vec<serde_json::Value> = images
                .into_iter()
                .map(|img| serde_json::json!({ "id": img.image.id }))
                .collect();
            Ok(serde_json::json!({ "count": items.len(), "images": items }))
        }
        "export_static_publish_package" => {
            let request: crate::commands::static_publishing::StaticPublishRequest =
                serde_json::from_value(args.cloned().unwrap_or(serde_json::json!({})))
                    .map_err(|e| DispatchError::Failed(format!("invalid publish request: {e}")))?;
            // module:static-publishing grant already verified above; the
            // module-granted entry point skips the raw setting gate.
            let result =
                crate::commands::static_publishing::export_static_publish_package_module_granted(
                    state, request,
                )
                .map_err(DispatchError::Failed)?;
            serde_json::to_value(result).map_err(|e| DispatchError::Failed(e.to_string()))
        }
        "list_catalog_presets" => {
            let params: ListCatalogPresetsArgs = parse_args(tool, args)?;
            let mut presets = state
                .db
                .list_catalog_presets()
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            if let Some(preset_kind) = params.preset_kind {
                presets.retain(|preset| preset.preset_kind == preset_kind);
            }
            serde_json::to_value(presets).map_err(|e| DispatchError::Failed(e.to_string()))
        }
        "get_catalog_preset" => {
            let params: CatalogPresetIdArgs = parse_args(tool, args)?;
            let preset = state
                .db
                .get_catalog_preset(&params.preset_id)
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            serde_json::to_value(preset).map_err(|e| DispatchError::Failed(e.to_string()))
        }
        "list_catalog_fields" => {
            let params: ListCatalogFieldsArgs = parse_args(tool, args)?;
            let fields = state
                .db
                .list_catalog_fields(
                    params.subject_scope.as_deref(),
                    params.include_deprecated.unwrap_or(false),
                )
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            serde_json::to_value(fields).map_err(|e| DispatchError::Failed(e.to_string()))
        }
        "create_catalog_field_def" => {
            let params: CreateCatalogFieldDefArgs = parse_args(tool, args)?;
            let id = state
                .db
                .create_catalog_field_def(
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
                )
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "id": id }))
        }
        "deprecate_catalog_field_def" => {
            let params: CatalogFieldDefIdArgs = parse_args(tool, args)?;
            state
                .db
                .deprecate_catalog_field_def(&params.field_def_id)
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "create_catalog_preset" => {
            let params: CreateCatalogPresetArgs = parse_args(tool, args)?;
            let id = state
                .db
                .create_catalog_preset(
                    &params.name,
                    params.description.as_deref(),
                    &params.preset_kind,
                    &params.field_def_ids,
                    params.layout_json.as_deref(),
                )
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "id": id }))
        }
        "update_catalog_preset" => {
            let params: UpdateCatalogPresetArgs = parse_args(tool, args)?;
            state
                .db
                .update_catalog_preset(
                    &params.preset_id,
                    params.name.as_deref(),
                    params.description.as_deref(),
                    params.field_def_ids.as_deref(),
                    params.layout_json.as_deref(),
                )
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "status": "ok", "id": params.preset_id }))
        }
        "create_catalog_work" => {
            let params: CreateCatalogWorkArgs = parse_args(tool, args)?;
            let id = state
                .db
                .create_catalog_work(&params.primary_image_id)
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "id": id }))
        }
        "attach_images_to_catalog_work" => {
            let params: AttachImagesToCatalogWorkArgs = parse_args(tool, args)?;
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
            let attached = state
                .db
                .attach_images_to_catalog_work(&params.work_id, &prepared)
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "attached": attached }))
        }
        "list_catalog_values" => {
            let params: ListCatalogValuesArgs = parse_args(tool, args)?;
            let values = state
                .db
                .list_catalog_values(
                    params.subject_type.as_deref(),
                    params.subject_id.as_deref(),
                    params.status.as_deref(),
                    params.source_type.as_deref(),
                    params.field_def_id.as_deref(),
                )
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            serde_json::to_value(values).map_err(|e| DispatchError::Failed(e.to_string()))
        }
        "list_catalog_drafts" => {
            let params: ListCatalogDraftsArgs = parse_args(tool, args)?;
            let values = state
                .db
                .list_catalog_drafts(
                    params.subject_type.as_deref(),
                    params.subject_id.as_deref(),
                    params.source_type.as_deref(),
                )
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            serde_json::to_value(values).map_err(|e| DispatchError::Failed(e.to_string()))
        }
        "get_catalog_record" => {
            let params: CatalogRecordArgs = parse_args(tool, args)?;
            let record = state
                .db
                .get_catalog_record(&params.subject_type, &params.subject_id)
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            serde_json::to_value(record).map_err(|e| DispatchError::Failed(e.to_string()))
        }
        "set_catalog_draft_value" => {
            let params: SetCatalogDraftValueArgs = parse_args(tool, args)?;
            let source_type = params.source_type.unwrap_or_else(|| "plugin".to_string());
            let id = state
                .db
                .upsert_catalog_draft_value(
                    &params.subject_type,
                    &params.subject_id,
                    &params.field_def_id,
                    &params.value_json,
                    &params.display_value,
                    &source_type,
                    params.source_id.as_deref(),
                    params.confidence,
                    "plugin",
                    Some(actor),
                    "draft",
                )
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "value_id": id }))
        }
        "set_catalog_draft_values" => {
            let params: CatalogDraftValuesArgs = parse_args(tool, args)?;
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
                        value.source_type.or_else(|| Some("plugin".to_string())),
                        value.confidence,
                        value.source_id,
                    )
                })
                .collect();
            let ids = state
                .db
                .set_catalog_draft_values(&payload, "plugin", Some(actor))
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            serde_json::to_value(ids).map_err(|e| DispatchError::Failed(e.to_string()))
        }
        "suggest_catalog_values" => {
            let params: CatalogDraftValuesArgs = parse_args(tool, args)?;
            let mut ids = Vec::new();
            for value in params.values {
                let id = state
                    .db
                    .upsert_catalog_draft_value(
                        &value.subject_type,
                        &value.subject_id,
                        &value.field_def_id,
                        &value.value_json,
                        &value.display_value,
                        "agent",
                        value.source_id.as_deref(),
                        value.confidence,
                        "plugin",
                        Some(actor),
                        "draft",
                    )
                    .map_err(|e| DispatchError::Failed(e.to_string()))?;
                ids.push(id);
            }
            Ok(serde_json::json!({
                "status": "completed",
                "drafted_count": ids.len(),
                "written_count": ids.len(),
                "ids": ids,
            }))
        }
        "approve_catalog_values" => {
            let params: CatalogValueIdsArgs = parse_args(tool, args)?;
            let updated = state
                .db
                .approve_catalog_values(&params.value_ids, Some(actor))
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "updated": updated }))
        }
        "reject_catalog_values" => {
            let params: CatalogValueIdsArgs = parse_args(tool, args)?;
            let updated = state
                .db
                .reject_catalog_values(&params.value_ids, Some(actor))
                .map_err(|e| DispatchError::Failed(e.to_string()))?;
            Ok(serde_json::json!({ "updated": updated }))
        }
        _ => Err(DispatchError::Unsupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::static_publishing::StaticPublishServerState;
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::secrets::MemoryStore;
    use crate::{services, watcher};
    use std::path::Path;

    fn test_state() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();
        let app_data_dir = tmp.path().join("app-data");
        let model_dir = tmp.path().join("models");
        std::fs::create_dir_all(&app_data_dir).unwrap();

        let state = AppState {
            db,
            app_data_dir,
            embedding_engine: parking_lot::Mutex::new(EmbeddingEngine::new(&model_dir)),
            detection_engine: parking_lot::Mutex::new(DetectionEngine::new_yolo(&model_dir)),
            safety_engine: parking_lot::Mutex::new(DetectionEngine::new_nudenet(&model_dir)),
            secrets: Box::new(MemoryStore::new()),
            jobs: services::jobs::JobRegistry::default(),
            action_manager: services::undo::ActionManager::new(),
            file_watcher: parking_lot::Mutex::new(watcher::FileWatcher::new()),
            clipboard_monitor: parking_lot::Mutex::new(
                services::clipboard_monitor::ClipboardMonitorState::default(),
            ),
            static_publish_server: parking_lot::Mutex::new(StaticPublishServerState::default()),
            preview_state: crate::preview::state::PreviewStateStore::default(),
            preview_web_stream: crate::preview::web_stream::PreviewWebStreamController::default(),
            agent_snapshots: parking_lot::Mutex::new(
                services::agent_snapshots::AgentSnapshotRegistry::default(),
            ),
            agent_snapshot_requests: parking_lot::Mutex::new(std::collections::HashMap::new()),
        };
        (state, tmp)
    }

    fn recent_audit(state: &AppState, limit: u32) -> Vec<crate::db_core::models::AuditEntry> {
        let ctx = ServiceContext::from_app_state(state, None);
        tokens::get_recent_audit(&ctx, limit).unwrap()
    }

    fn write_test_image(path: &Path) {
        let image: image::RgbImage =
            image::ImageBuffer::from_pixel(48, 32, image::Rgb([32, 96, 160]));
        image.save(path).unwrap();
    }

    fn import_test_image(state: &AppState, tmp: &Path, name: &str) -> String {
        let source_path = tmp.join(name);
        write_test_image(&source_path);
        crate::db_core::import::import_file(&state.db, &source_path, &state.app_data_dir)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn plugin_invoke_rejects_missing_capability() {
        let (state, _tmp) = test_state();
        // Plugin granted ONLY library:read calls an export-capability tool.
        state
            .db
            .set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();

        let err = plugin_invoke_inner(&state, "cull-publish", "export_images", None).unwrap_err();

        // Same error shape as require_capability (mcp/auth.rs).
        assert_eq!(
            err,
            "Permission denied: 'export_images' requires 'export:read' capability"
        );

        // The rejection is audit-logged with the plugin actor marker.
        let entries = recent_audit(&state, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool_name, "export_images");
        assert_eq!(entries[0].result_status, "denied");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_with_no_grants_rejects_everything() {
        let (state, _tmp) = test_state();

        let err =
            plugin_invoke_inner(&state, "unknown-plugin", "get_library_stats", None).unwrap_err();
        assert!(
            err.contains("Permission denied"),
            "ungranted plugin must be denied: {err}"
        );

        let entries = recent_audit(&state, 10);
        assert_eq!(entries[0].result_status, "denied");
        assert_eq!(
            entries[0].token_id.as_deref(),
            Some("plugin:unknown-plugin")
        );
    }

    #[test]
    fn plugin_invoke_allows_granted_capability_and_audits_ok() {
        let (state, _tmp) = test_state();
        state
            .db
            .set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();

        // list_collections maps to library:read in tokens::tool_capability.
        let value = plugin_invoke_inner(&state, "cull-publish", "list_collections", None)
            .expect("granted capability must pass the bridge");
        assert!(value.get("collections").is_some());

        let entries = recent_audit(&state, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool_name, "list_collections");
        assert_eq!(entries[0].result_status, "ok");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_granted_but_undisptachable_tool_is_audited_unsupported() {
        let (state, _tmp) = test_state();
        state
            .db
            .set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();

        // Passes the capability check (library:read) but is not in the v1
        // dispatch whitelist.
        let err = plugin_invoke_inner(&state, "cull-publish", "get_image", None).unwrap_err();
        assert!(
            err.contains("plugin runtime"),
            "unsupported tool error must say so: {err}"
        );

        let entries = recent_audit(&state, 10);
        assert_eq!(entries[0].result_status, "unsupported");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_args_are_redacted_in_audit_log() {
        let (state, _tmp) = test_state();
        state
            .db
            .set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();

        let args = serde_json::json!({ "folder_path": "/Users/alice/secret", "limit": 5 });
        let _ = plugin_invoke_inner(&state, "cull-publish", "list_collections", Some(args));

        let entries = recent_audit(&state, 1);
        let params = entries[0].params_json.as_deref().unwrap();
        assert!(!params.contains("/Users/alice/secret"));
        assert!(params.contains("[redacted:path]"));
        assert!(params.contains("\"limit\":5"));
    }

    // --- Track C3: publish-relevant dispatch ops ---------------------------

    #[test]
    fn plugin_invoke_dispatches_get_library_stats() {
        let (state, tmp) = test_state();
        state
            .db
            .set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();
        let image_id = import_test_image(&state, tmp.path(), "stats.png");
        let collection_id = state.db.create_collection("Publish Set").unwrap();
        state
            .db
            .add_to_collection(&collection_id, &[image_id.as_str()])
            .unwrap();

        let value = plugin_invoke_inner(&state, "cull-publish", "get_library_stats", None)
            .expect("library:read grants get_library_stats");
        assert_eq!(value["image_count"], 1);
        assert_eq!(value["collection_count"], 1);
        assert!(value.get("folder_count").is_some());

        let entries = recent_audit(&state, 1);
        assert_eq!(entries[0].tool_name, "get_library_stats");
        assert_eq!(entries[0].result_status, "ok");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_dispatches_list_collection_images() {
        let (state, tmp) = test_state();
        state
            .db
            .set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();
        let image_id = import_test_image(&state, tmp.path(), "in-collection.png");
        let collection_id = state.db.create_collection("Publish Set").unwrap();
        state
            .db
            .add_to_collection(&collection_id, &[image_id.as_str()])
            .unwrap();

        let value = plugin_invoke_inner(
            &state,
            "cull-publish",
            "list_collection_images",
            Some(serde_json::json!({ "collection_id": collection_id })),
        )
        .expect("library:read grants list_collection_images");
        assert_eq!(value["count"], 1);
        assert_eq!(value["images"][0]["id"], serde_json::json!(image_id));

        // Missing collection_id is a dispatch failure, not a panic.
        let err = plugin_invoke_inner(&state, "cull-publish", "list_collection_images", None)
            .unwrap_err();
        assert!(err.contains("collection_id"), "useful error, got: {err}");
    }

    #[test]
    fn plugin_invoke_export_requires_module_grant() {
        let (state, _tmp) = test_state();
        // export:read alone is NOT enough: the tool is module-gated and the
        // plugin does not hold module:static-publishing.
        state
            .db
            .set_plugin_grants("cull-publish", &["export:read".to_string()])
            .unwrap();

        let err = plugin_invoke_inner(
            &state,
            "cull-publish",
            "export_static_publish_package",
            None,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "Permission denied: 'export_static_publish_package' requires 'module:static-publishing' permission"
        );

        let entries = recent_audit(&state, 1);
        assert_eq!(entries[0].result_status, "denied");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_export_static_publish_package_builds_package() {
        let (state, tmp) = test_state();
        // NOTE: module_static_publishing app setting stays OFF — the plugin's
        // module:static-publishing grant substitutes for the raw setting
        // (plugin presence is the gate).
        state
            .db
            .set_plugin_grants(
                "cull-publish",
                &[
                    "library:read".to_string(),
                    "export:read".to_string(),
                    "module:static-publishing".to_string(),
                ],
            )
            .unwrap();
        let image_id = import_test_image(&state, tmp.path(), "publish-me.png");
        let output_dir = tmp.path().join("exports");

        let args = serde_json::json!({
            "canvas_name": "Plugin Publish",
            "items": [{ "image_id": image_id }],
            "layout_json": "{\"type\":\"plugin_publish\"}",
            "output_dir": output_dir.to_string_lossy(),
            "include_thumbnails": true,
            "include_web": true,
            "include_full": false,
            "indexable": false,
            "links": [],
        });
        let value = plugin_invoke_inner(
            &state,
            "cull-publish",
            "export_static_publish_package",
            Some(args),
        )
        .expect("granted publish op must build a package");

        assert_eq!(value["image_count"], 1);
        let site_dir = value["site_dir"].as_str().unwrap();
        assert!(Path::new(site_dir).join("index.html").exists());

        let entries = recent_audit(&state, 1);
        assert_eq!(entries[0].tool_name, "export_static_publish_package");
        assert_eq!(entries[0].result_status, "ok");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_catalog_capabilities_are_enforced() {
        let (state, _tmp) = test_state();
        state
            .db
            .set_plugin_grants("catalog-plugin", &[tokens::CAP_CATALOG_READ.to_string()])
            .unwrap();

        let fields = plugin_invoke_inner(&state, "catalog-plugin", "list_catalog_fields", None)
            .expect("catalog:read should allow catalog field reads");
        assert!(fields.as_array().is_some());

        let write_err = plugin_invoke_inner(
            &state,
            "catalog-plugin",
            "create_catalog_work",
            Some(serde_json::json!({ "primary_image_id": "img_missing" })),
        )
        .unwrap_err();
        assert_eq!(
            write_err,
            "Permission denied: 'create_catalog_work' requires 'catalog:write' capability"
        );

        let admin_err = plugin_invoke_inner(
            &state,
            "catalog-plugin",
            "create_catalog_field_def",
            Some(serde_json::json!({
                "stable_key": "plugin.test",
                "label": "Plugin Test",
                "subject_scope": "image",
                "value_type": "text",
                "cardinality": "single",
                "sensitivity": "normal"
            })),
        )
        .unwrap_err();
        assert_eq!(
            admin_err,
            "Permission denied: 'create_catalog_field_def' requires 'catalog:admin' capability"
        );
    }

    #[test]
    fn plugin_invoke_dispatches_catalog_draft_and_approval_flow() {
        let (state, tmp) = test_state();
        state
            .db
            .set_plugin_grants(
                "catalog-plugin",
                &[
                    tokens::CAP_CATALOG_READ.to_string(),
                    tokens::CAP_CATALOG_WRITE.to_string(),
                    tokens::CAP_CATALOG_APPROVE.to_string(),
                    tokens::CAP_CATALOG_ADMIN.to_string(),
                ],
            )
            .unwrap();

        let field = plugin_invoke_inner(
            &state,
            "catalog-plugin",
            "create_catalog_field_def",
            Some(serde_json::json!({
                "stable_key": "plugin.caption",
                "label": "Plugin Caption",
                "subject_scope": "image",
                "value_type": "text",
                "cardinality": "single",
                "sensitivity": "normal"
            })),
        )
        .expect("catalog:admin grants field definition creation");
        let field_def_id = field["id"].as_str().unwrap().to_string();

        let image_id = import_test_image(&state, tmp.path(), "catalog-plugin.png");
        let draft = plugin_invoke_inner(
            &state,
            "catalog-plugin",
            "set_catalog_draft_value",
            Some(serde_json::json!({
                "subject_type": "image",
                "subject_id": image_id,
                "field_def_id": field_def_id,
                "value_json": "{\"value\":\"Gallery wall\"}",
                "display_value": "Gallery wall"
            })),
        )
        .expect("catalog:write grants draft writes");
        let value_id = draft["value_id"].as_str().unwrap().to_string();

        let drafts = plugin_invoke_inner(
            &state,
            "catalog-plugin",
            "list_catalog_drafts",
            Some(serde_json::json!({ "subject_id": image_id })),
        )
        .expect("catalog:read grants draft reads");
        assert_eq!(drafts.as_array().unwrap().len(), 1);

        let approved = plugin_invoke_inner(
            &state,
            "catalog-plugin",
            "approve_catalog_values",
            Some(serde_json::json!({ "value_ids": [value_id] })),
        )
        .expect("catalog:approve grants approvals");
        assert_eq!(approved["updated"], 1);

        let values = plugin_invoke_inner(
            &state,
            "catalog-plugin",
            "list_catalog_values",
            Some(serde_json::json!({ "status": "approved" })),
        )
        .expect("catalog:read grants approved value reads");
        assert_eq!(values.as_array().unwrap().len(), 1);
        assert_eq!(values[0]["status"], "approved");
    }
}
