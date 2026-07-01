use crate::db_core::models::{AgentActionProposal, AgentSelectionPreset, NewSessionEvent};
use crate::services::agent_proposals as svc;
use crate::services::claude_agent as claude_svc;
use crate::services::generation;
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
    let mut result = claude_svc::run_claude_agent_chat_turn_db_with_events(
        &state.db,
        request,
        Some(app.clone()),
    )
    .await
    .map_err(|e| e.to_string())?;
    if let Some(generation_draft) = result.generation.clone() {
        let job_id = start_agent_generation_job(&app, &state, generation_draft)?;
        result.generation_job_id = Some(job_id.clone());
        log_agent_generation_event(
            &state,
            "agent_generation_started",
            &job_id,
            &instruction,
            &result,
        );
    }
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

fn start_agent_generation_job(
    app: &AppHandle,
    state: &State<'_, AppState>,
    draft: claude_svc::ClaudeGenerationDraft,
) -> Result<String, String> {
    let provider = non_empty_or(draft.provider, "openai");
    let provider_cfg = generation::provider_config(&provider)?;
    let api_key = state
        .secrets
        .get(provider_cfg.key_name)?
        .ok_or_else(|| format!("{} API key not set. Go to Settings to add it.", provider))?;
    let gen_request = generation::GenerationRequest {
        provider: provider.clone(),
        source_image_id: draft
            .include_source
            .unwrap_or(true)
            .then_some(draft.image_id),
        prompt: draft.prompt.trim().to_string(),
        n: draft.n.unwrap_or(1).clamp(1, 4),
        model: non_empty_or(draft.model, default_generation_model(&provider)),
        size: non_empty_or(draft.size, default_generation_size(&provider)),
        quality: non_empty_or(draft.quality, "auto"),
    };
    let db = state.db.clone();
    let jobs = state.jobs.clone();
    let app_data_dir = state.app_data_dir.clone();
    let app_clone = app.clone();
    let base_url = provider_cfg.base_url.to_string();
    let api_style = provider_cfg.api_style;
    let (job_id, cancel) = state.jobs.create_job("generation", gen_request.n as u32);
    let job_id_for_task = job_id.clone();

    tokio::spawn(async move {
        let _ = generation::generate_images(
            &gen_request,
            &api_key,
            &base_url,
            api_style,
            &app_data_dir,
            &db,
            &jobs,
            &job_id_for_task,
            &cancel,
            &app_clone,
        )
        .await;
    });

    Ok(job_id)
}

fn non_empty_or(value: Option<String>, fallback: &str) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn default_generation_model(provider: &str) -> &'static str {
    match provider {
        "google" => "gemini-2.5-flash-image",
        "openrouter" => "openai/gpt-image-2",
        _ => "gpt-image-2",
    }
}

fn default_generation_size(provider: &str) -> &'static str {
    match provider {
        "google" | "openrouter" => "auto",
        _ => "1024x1024",
    }
}

fn log_agent_generation_event(
    state: &State<'_, AppState>,
    event_type: &str,
    job_id: &str,
    instruction: &str,
    result: &claude_svc::ClaudeAgentChatTurnResult,
) {
    let generation = result.generation.as_ref();
    let _ = state.db.log_session_event(&NewSessionEvent {
        session_id: None,
        event_type: event_type.to_string(),
        actor_type: "agent".to_string(),
        actor_id: None,
        subject_type: Some("generation_job".to_string()),
        subject_id: Some(job_id.to_string()),
        payload_json: serde_json::json!({
            "operation": result.operation,
            "instruction": instruction,
            "message": result.message,
            "image_id": generation.map(|draft| draft.image_id.as_str()),
            "provider": generation.and_then(|draft| draft.provider.as_deref()),
            "model": generation.and_then(|draft| draft.model.as_deref()),
        })
        .to_string(),
    });
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
