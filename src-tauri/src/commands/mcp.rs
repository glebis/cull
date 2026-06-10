use crate::db_core::models::{McpToken, TokenScope};
use crate::services::{tokens, ServiceContext};
use crate::AppState;
use tauri::State;

fn ui_op_status<T, E>(result: &Result<T, E>) -> &'static str {
    if result.is_ok() {
        "ok"
    } else {
        "error"
    }
}

/// UI-side token creation. Writes the same audit-log trail as the MCP
/// `create_token` tool; `token_id = NULL` marks a local (UI) actor.
pub(crate) fn create_token_audited(
    ctx: &ServiceContext,
    name: &str,
    role: &str,
    scope: Option<TokenScope>,
    expires_at: Option<String>,
) -> Result<(McpToken, String), String> {
    let result = tokens::create_token(ctx, name, role, scope, expires_at);
    let params = serde_json::json!({
        "surface": "ui",
        "target": result.as_ref().ok().map(|(t, _)| t.id.as_str()),
        "name": name,
        "role": role,
        "expires_at": result.as_ref().ok().and_then(|(t, _)| t.expires_at.as_deref()),
    })
    .to_string();
    let _ = tokens::log_audit(
        ctx,
        None,
        "create_token",
        Some(&params),
        ui_op_status(&result),
    );
    result.map_err(|e| e.to_string())
}

/// UI-side token revocation, audited like its MCP twin.
pub(crate) fn revoke_token_audited(ctx: &ServiceContext, token_id: &str) -> Result<(), String> {
    let result = tokens::revoke_token(ctx, token_id);
    let params = serde_json::json!({ "surface": "ui", "target": token_id }).to_string();
    let _ = tokens::log_audit(
        ctx,
        None,
        "revoke_token",
        Some(&params),
        ui_op_status(&result),
    );
    result.map_err(|e| e.to_string())
}

/// UI-side token rotation, audited like its MCP twin. The new secret is never
/// written to the audit log.
pub(crate) fn rotate_token_audited(ctx: &ServiceContext, token_id: &str) -> Result<String, String> {
    let result = tokens::rotate_token(ctx, token_id);
    let params = serde_json::json!({ "surface": "ui", "target": token_id }).to_string();
    let _ = tokens::log_audit(
        ctx,
        None,
        "rotate_token",
        Some(&params),
        ui_op_status(&result),
    );
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_mcp_token(
    state: State<'_, AppState>,
    name: String,
    role: String,
    scope: Option<TokenScope>,
    expires_at: Option<String>,
) -> Result<(McpToken, String), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    create_token_audited(&ctx, &name, &role, scope, expires_at)
}

#[tauri::command]
pub async fn list_mcp_tokens(state: State<'_, AppState>) -> Result<Vec<McpToken>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::list_tokens(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn revoke_mcp_token(state: State<'_, AppState>, token_id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    revoke_token_audited(&ctx, &token_id)
}

#[tauri::command]
pub async fn rotate_mcp_token(
    state: State<'_, AppState>,
    token_id: String,
) -> Result<String, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    rotate_token_audited(&ctx, &token_id)
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
    fn token_audit_create_token_command_writes_audit_log_row() {
        let f = Fixture::new();
        let ctx = f.ctx();

        let (token, secret) = create_token_audited(&ctx, "UI Token", "viewer", None, None).unwrap();

        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool_name, "create_token");
        assert_eq!(entries[0].result_status, "ok");
        // UI-side operations have no acting token.
        assert!(entries[0].token_id.is_none());
        let params = entries[0].params_json.as_deref().unwrap();
        assert!(params.contains(&token.id));
        assert!(params.contains("viewer"));
        // The secret must never reach the audit log.
        assert!(!params.contains(&secret));
    }

    #[test]
    fn token_audit_revoke_token_command_writes_audit_log_row() {
        let f = Fixture::new();
        let ctx = f.ctx();

        let (token, _) = create_token_audited(&ctx, "Doomed", "viewer", None, None).unwrap();
        revoke_token_audited(&ctx, &token.id).unwrap();

        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries[0].tool_name, "revoke_token");
        assert_eq!(entries[0].result_status, "ok");
        assert!(entries[0].token_id.is_none());
        assert!(entries[0]
            .params_json
            .as_deref()
            .unwrap()
            .contains(&token.id));
    }

    #[test]
    fn token_audit_rotate_token_command_writes_audit_log_row() {
        let f = Fixture::new();
        let ctx = f.ctx();

        let (token, _) = create_token_audited(&ctx, "Rotated", "curator", None, None).unwrap();
        let new_secret = rotate_token_audited(&ctx, &token.id).unwrap();

        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries[0].tool_name, "rotate_token");
        assert_eq!(entries[0].result_status, "ok");
        assert!(entries[0]
            .params_json
            .as_deref()
            .unwrap()
            .contains(&token.id));
        // The new secret must never reach the audit log.
        assert!(!entries[0]
            .params_json
            .as_deref()
            .unwrap()
            .contains(&new_secret));
    }

    #[test]
    fn token_audit_failed_token_command_writes_error_audit_row() {
        let f = Fixture::new();
        let ctx = f.ctx();

        assert!(revoke_token_audited(&ctx, "tok_missing").is_err());

        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries[0].tool_name, "revoke_token");
        assert_eq!(entries[0].result_status, "error");
    }
}
