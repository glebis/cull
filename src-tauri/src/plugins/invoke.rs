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

use crate::mcp::auth::{require_capability, AuthContext};
use crate::services::{tokens, ServiceContext};

/// Capability-check + audit + dispatch core of the `plugin_invoke` command.
/// Separated from the Tauri command so it is unit-testable with a plain
/// `ServiceContext`.
pub fn plugin_invoke_inner(
    ctx: &ServiceContext,
    plugin_id: &str,
    tool: &str,
    args: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let capabilities = ctx
        .db
        .granted_plugin_capabilities(plugin_id)
        .map_err(|e| e.to_string())?;
    let auth = AuthContext::for_plugin(plugin_id, capabilities);
    let actor = auth
        .token_id()
        .expect("plugin auth context always has an actor")
        .to_string();
    let params_json = args.as_ref().and_then(|a| serde_json::to_string(a).ok());

    // Same enforcement code path as MCP tool calls: tokens::tool_capability
    // resolves the required capability; require_capability produces the same
    // error shape.
    if let Err(msg) = require_capability(&auth, tool) {
        let _ = tokens::log_audit(ctx, Some(&actor), tool, params_json.as_deref(), "denied");
        return Err(msg);
    }

    let result = dispatch(ctx, tool, args.as_ref());
    let status = match &result {
        Ok(_) => "ok",
        Err(DispatchError::Unsupported) => "unsupported",
        Err(DispatchError::Failed(_)) => "error",
    };
    let _ = tokens::log_audit(ctx, Some(&actor), tool, params_json.as_deref(), status);

    result.map_err(|e| match e {
        DispatchError::Unsupported => {
            format!("Tool '{tool}' is not available through the plugin runtime (v1 whitelist)")
        }
        DispatchError::Failed(msg) => msg,
    })
}

enum DispatchError {
    /// Tool passed the capability check but is not in the v1 dispatch whitelist.
    Unsupported,
    Failed(String),
}

/// v1 dispatch whitelist. Deliberately small: the runtime ships before the
/// proof plugin (Track C2), which will extend this alongside its view.
fn dispatch(
    ctx: &ServiceContext,
    tool: &str,
    _args: Option<&serde_json::Value>,
) -> Result<serde_json::Value, DispatchError> {
    match tool {
        "list_collections" => {
            let collections = ctx
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
        _ => Err(DispatchError::Unsupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::secrets::MemoryStore;
    use parking_lot::Mutex;
    use std::path::PathBuf;

    struct Fixture {
        db: Database,
        secrets: MemoryStore,
        app_data_dir: PathBuf,
        embedding_engine: Mutex<EmbeddingEngine>,
        detection_engine: Mutex<DetectionEngine>,
        safety_engine: Mutex<DetectionEngine>,
        _tmp: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let model_dir = tmp.path().join("models");
            Self {
                db: Database::open(std::path::Path::new(":memory:")).unwrap(),
                secrets: MemoryStore::new(),
                app_data_dir: tmp.path().to_path_buf(),
                embedding_engine: Mutex::new(EmbeddingEngine::new(&model_dir)),
                detection_engine: Mutex::new(DetectionEngine::new_yolo(&model_dir)),
                safety_engine: Mutex::new(DetectionEngine::new_nudenet(&model_dir)),
                _tmp: tmp,
            }
        }

        fn ctx(&self) -> ServiceContext<'_> {
            ServiceContext {
                db: &self.db,
                app_data_dir: &self.app_data_dir,
                embedding_engine: &self.embedding_engine,
                detection_engine: &self.detection_engine,
                safety_engine: &self.safety_engine,
                secrets: &self.secrets,
                app_handle: None,
            }
        }
    }

    #[test]
    fn plugin_invoke_rejects_missing_capability() {
        let f = Fixture::new();
        let ctx = f.ctx();
        // Plugin granted ONLY library:read calls an export-capability tool.
        f.db.set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();

        let err = plugin_invoke_inner(&ctx, "cull-publish", "export_images", None).unwrap_err();

        // Same error shape as require_capability (mcp/auth.rs).
        assert_eq!(
            err,
            "Permission denied: 'export_images' requires 'export:read' capability"
        );

        // The rejection is audit-logged with the plugin actor marker.
        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool_name, "export_images");
        assert_eq!(entries[0].result_status, "denied");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_with_no_grants_rejects_everything() {
        let f = Fixture::new();
        let ctx = f.ctx();

        let err =
            plugin_invoke_inner(&ctx, "unknown-plugin", "get_library_stats", None).unwrap_err();
        assert!(
            err.contains("Permission denied"),
            "ungranted plugin must be denied: {err}"
        );

        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries[0].result_status, "denied");
        assert_eq!(
            entries[0].token_id.as_deref(),
            Some("plugin:unknown-plugin")
        );
    }

    #[test]
    fn plugin_invoke_allows_granted_capability_and_audits_ok() {
        let f = Fixture::new();
        let ctx = f.ctx();
        f.db.set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();

        // list_collections maps to library:read in tokens::tool_capability.
        let value = plugin_invoke_inner(&ctx, "cull-publish", "list_collections", None)
            .expect("granted capability must pass the bridge");
        assert!(value.get("collections").is_some());

        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool_name, "list_collections");
        assert_eq!(entries[0].result_status, "ok");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_granted_but_undisptachable_tool_is_audited_unsupported() {
        let f = Fixture::new();
        let ctx = f.ctx();
        f.db.set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();

        // Passes the capability check (library:read) but is not in the v1
        // dispatch whitelist.
        let err = plugin_invoke_inner(&ctx, "cull-publish", "get_image", None).unwrap_err();
        assert!(
            err.contains("plugin runtime"),
            "unsupported tool error must say so: {err}"
        );

        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries[0].result_status, "unsupported");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn plugin_invoke_args_are_redacted_in_audit_log() {
        let f = Fixture::new();
        let ctx = f.ctx();
        f.db.set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();

        let args = serde_json::json!({ "folder_path": "/Users/alice/secret", "limit": 5 });
        let _ = plugin_invoke_inner(&ctx, "cull-publish", "list_collections", Some(args));

        let entries = tokens::get_recent_audit(&ctx, 1).unwrap();
        let params = entries[0].params_json.as_deref().unwrap();
        assert!(!params.contains("/Users/alice/secret"));
        assert!(params.contains("[redacted:path]"));
        assert!(params.contains("\"limit\":5"));
    }
}
