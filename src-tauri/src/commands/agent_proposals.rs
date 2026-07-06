use crate::db_core::models::{AgentActionProposal, AgentSelectionPreset, NewSessionEvent};
use crate::services::agent_proposals as svc;
use crate::services::claude_agent as claude_svc;
use crate::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn create_action_proposal(
    state: State<'_, AppState>,
    request: svc::CreateActionProposalRequest,
) -> Result<AgentActionProposal, String> {
    let proposal = svc::create_action_proposal_db(&state.db, request).map_err(|e| e.to_string())?;
    log_agent_proposal_event(&state, "agent_proposal_created", "user", &proposal, None);
    Ok(proposal)
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
    let proposal = state
        .db
        .get_action_proposal(&proposal_id)
        .map_err(|e| e.to_string())?;
    svc::dismiss_action_proposal_db(&state.db, &proposal_id).map_err(|e| e.to_string())?;
    if let Some(proposal) = proposal {
        log_agent_proposal_event(&state, "agent_proposal_dismissed", "user", &proposal, None);
    }
    Ok(())
}

#[tauri::command]
pub async fn apply_action_proposal(
    state: State<'_, AppState>,
    proposal_id: String,
    approved_image_ids: Vec<String>,
    result_json: String,
) -> Result<svc::ApplyActionProposalResult, String> {
    let proposal = state
        .db
        .get_action_proposal(&proposal_id)
        .map_err(|e| e.to_string())?;
    let result =
        svc::apply_action_proposal_db(&state.db, &proposal_id, &approved_image_ids, &result_json)
            .map_err(|e| e.to_string())?;
    if let Some(proposal) = proposal {
        log_agent_proposal_event(
            &state,
            "agent_proposal_applied",
            "user",
            &proposal,
            Some(serde_json::json!({
                "approved_image_count": approved_image_ids.len(),
                "approved_image_ids": approved_image_ids,
            })),
        );
    }
    Ok(result)
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
    app: AppHandle,
    state: State<'_, AppState>,
    request: claude_svc::ClaudeAgentChatTurnRequest,
) -> Result<claude_svc::ClaudeAgentChatTurnResult, String> {
    let instruction = request.instruction.clone();
    let result =
        claude_svc::run_claude_agent_chat_turn_db_with_events(&state.db, request, Some(app))
            .await
            .map_err(|e| e.to_string())?;
    if let Some(proposal) = result.proposal.as_ref() {
        log_agent_proposal_event(
            &state,
            "agent_proposal_created",
            "agent",
            proposal,
            Some(serde_json::json!({
                "operation": result.operation,
                "instruction": instruction,
                "message": result.message,
            })),
        );
    }
    Ok(result)
}

#[tauri::command]
pub async fn cancel_claude_agent_chat_turn(request_id: String) -> Result<bool, String> {
    Ok(claude_svc::cancel_claude_agent_chat_turn_request(
        &request_id,
    ))
}

fn log_agent_proposal_event(
    state: &State<'_, AppState>,
    event_type: &str,
    actor_type: &str,
    proposal: &AgentActionProposal,
    extra_payload: Option<serde_json::Value>,
) {
    let item_count = serde_json::from_str::<serde_json::Value>(&proposal.items_json)
        .ok()
        .and_then(|value| value.as_array().map(Vec::len))
        .unwrap_or(0);
    let mut payload = serde_json::json!({
        "proposal_id": proposal.id,
        "kind": proposal.kind,
        "lens": proposal.lens,
        "status": proposal.status,
        "visual_level": proposal.visual_level,
        "selection_preset_id": proposal.selection_preset_id,
        "item_count": item_count,
    });
    if let (Some(payload_object), Some(extra_object)) = (
        payload.as_object_mut(),
        extra_payload.and_then(|value| value.as_object().cloned()),
    ) {
        payload_object.extend(extra_object);
    }

    let _ = state.db.log_session_event(&NewSessionEvent {
        session_id: None,
        event_type: event_type.to_string(),
        actor_type: actor_type.to_string(),
        actor_id: None,
        subject_type: Some("agent_action_proposal".to_string()),
        subject_id: Some(proposal.id.clone()),
        payload_json: payload.to_string(),
    });
}
