use super::*;

#[tool_router(router = system_router)]
impl CullMcp {
    #[tool(description = "Get status and progress of a background job by ID")]
    fn get_job(&self, Parameters(params): Parameters<GetJobParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.jobs.get(&params.job_id) {
            Some(snapshot) => serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string()),
            None => format!("Error: Job '{}' not found", params.job_id),
        }
    }

    #[tool(
        description = "List all background jobs (running and recent completed/failed/cancelled)"
    )]
    fn list_jobs(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let jobs = state.jobs.list();
        serde_json::to_string(&jobs).unwrap_or_else(|_| "[]".to_string())
    }

    #[tool(
        description = "Cancel a running background job. Sets status to 'cancelling', job stops after current item."
    )]
    fn cancel_job(&self, Parameters(params): Parameters<CancelJobParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        match state.jobs.cancel(&params.job_id) {
            Ok(()) => {
                serde_json::json!({"status": "cancelling", "job_id": params.job_id}).to_string()
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Create a new MCP access token. Returns token ID and secret (shown only once)."
    )]
    fn create_token(&self, Parameters(params): Parameters<CreateTokenParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::create_token(
            &ctx,
            &params.name,
            &params.role,
            params.scope,
            params.expires_at,
        ) {
            Ok((token, secret)) => serde_json::json!({
                "token_id": token.id,
                "name": token.name,
                "role": token.role,
                "expires_at": token.expires_at,
                "secret": secret,
                "warning": "Store the secret securely — it cannot be retrieved again"
            })
            .to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List all active (non-revoked) MCP tokens")]
    fn list_tokens(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::list_tokens(&ctx) {
            Ok(tokens_list) => {
                let result: Vec<serde_json::Value> = tokens_list
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "id": t.id, "name": t.name, "role": t.role,
                            "created_at": t.created_at, "expires_at": t.expires_at,
                            "last_used_at": t.last_used_at,
                        })
                    })
                    .collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Revoke an MCP token permanently")]
    fn revoke_token(&self, Parameters(params): Parameters<TokenIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::revoke_token(&ctx, &params.token_id) {
            Ok(()) => serde_json::json!({"status": "ok", "revoked": params.token_id}).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Rotate a token's secret. Returns new secret (old becomes invalid).")]
    fn rotate_token(&self, Parameters(params): Parameters<TokenIdParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        match crate::services::tokens::rotate_token(&ctx, &params.token_id) {
            Ok(new_secret) => serde_json::json!({
                "token_id": params.token_id,
                "new_secret": new_secret,
                "warning": "Store the new secret securely"
            })
            .to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get recent MCP audit log entries")]
    fn get_audit_log(&self, Parameters(params): Parameters<AuditLogParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        let limit = params.limit.unwrap_or(50).min(500);
        match crate::services::tokens::get_recent_audit(&ctx, limit) {
            Ok(entries) => {
                let result: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id, "token_id": e.token_id,
                            "tool_name": e.tool_name, "result_status": e.result_status,
                            "timestamp": e.timestamp,
                        })
                    })
                    .collect();
                serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Delete old audit log entries. Returns count deleted.")]
    fn prune_audit_log(&self, Parameters(params): Parameters<PruneAuditParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let ctx =
            crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
        let days = params.retention_days.unwrap_or(30);
        match crate::services::tokens::prune_audit_log(&ctx, days) {
            Ok(deleted) => {
                serde_json::json!({"deleted": deleted, "retention_days": days}).to_string()
            }
            Err(e) => format!("Error: {}", e),
        }
    }
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::system_router()
}
