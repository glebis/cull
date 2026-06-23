use crate::db_core::models::{AgentActionProposal, AgentSelectionPreset};
use crate::services::agent_proposals as svc;
use crate::services::claude_agent as claude_svc;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn create_action_proposal(
    state: State<'_, AppState>,
    request: svc::CreateActionProposalRequest,
) -> Result<AgentActionProposal, String> {
    svc::create_action_proposal_db(&state.db, request).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_action_proposals(
    state: State<'_, AppState>,
    status: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<AgentActionProposal>, String> {
    svc::list_action_proposals_db(&state.db, status.as_deref(), limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dismiss_action_proposal(
    state: State<'_, AppState>,
    proposal_id: String,
) -> Result<(), String> {
    svc::dismiss_action_proposal_db(&state.db, &proposal_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn apply_action_proposal(
    state: State<'_, AppState>,
    proposal_id: String,
    approved_image_ids: Vec<String>,
    result_json: String,
) -> Result<svc::ApplyActionProposalResult, String> {
    svc::apply_action_proposal_db(&state.db, &proposal_id, &approved_image_ids, &result_json)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_agent_selection_presets(
    state: State<'_, AppState>,
) -> Result<Vec<AgentSelectionPreset>, String> {
    svc::list_agent_selection_presets_db(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_agent_selection_preset(
    state: State<'_, AppState>,
    request: svc::UpsertAgentSelectionPresetRequest,
) -> Result<AgentSelectionPreset, String> {
    svc::upsert_agent_selection_preset_db(&state.db, request).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_claude_agent_chat_turn(
    state: State<'_, AppState>,
    request: claude_svc::ClaudeAgentChatTurnRequest,
) -> Result<claude_svc::ClaudeAgentChatTurnResult, String> {
    claude_svc::run_claude_agent_chat_turn_db(&state.db, request)
        .await
        .map_err(|e| e.to_string())
}
