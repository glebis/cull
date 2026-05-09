use tauri::State;
use crate::AppState;
use crate::db_core::models::{McpToken, TokenScope};
use crate::services::{ServiceContext, tokens};

#[tauri::command]
pub async fn create_mcp_token(
    state: State<'_, AppState>,
    name: String,
    role: String,
    scope: Option<TokenScope>,
) -> Result<(McpToken, String), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::create_token(&ctx, &name, &role, scope).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_mcp_tokens(state: State<'_, AppState>) -> Result<Vec<McpToken>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::list_tokens(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn revoke_mcp_token(state: State<'_, AppState>, token_id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::revoke_token(&ctx, &token_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rotate_mcp_token(state: State<'_, AppState>, token_id: String) -> Result<String, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::rotate_token(&ctx, &token_id).map_err(|e| e.to_string())
}
