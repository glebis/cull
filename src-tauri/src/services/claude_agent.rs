use crate::db_core::db::Database;
use crate::db_core::models::{AgentActionProposal, AgentSelectionPreset};
use crate::services::agent_proposals as proposals;
use crate::services::ServiceError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

const DEFAULT_MODEL: &str = "haiku";
const DEFAULT_MAX_BUDGET_USD: f64 = 0.05;
const USD_TO_EUR_DISPLAY_ESTIMATE: f64 = 0.93;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeAgentChatTurnRequest {
    pub instruction: String,
    pub visual_level: String,
    pub preset: Option<AgentSelectionPreset>,
    pub candidate_images: Vec<AgentChatImageContext>,
    pub selected_count: u32,
    pub visible_count: u32,
    pub model: Option<String>,
    pub max_budget_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChatImageContext {
    pub image_id: String,
    pub filename: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<String>,
    pub star_rating: Option<i64>,
    pub color_label: Option<String>,
    pub decision: Option<String>,
    pub source_label: Option<String>,
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeAgentChatTurnResult {
    pub operation: String,
    pub message: String,
    pub proposal: Option<AgentActionProposal>,
    pub updated_preset: Option<AgentSelectionPreset>,
    pub usage_json: String,
    pub raw_result_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeCliEnvelope {
    #[serde(default)]
    structured_output: Value,
    #[serde(default)]
    usage: Value,
    #[serde(default)]
    #[serde(rename = "modelUsage")]
    model_usage: Value,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeAgentDecision {
    operation: String,
    message: String,
    #[serde(default)]
    proposal: Option<ClaudeProposalDraft>,
    #[serde(default)]
    preset_update: Option<ClaudePresetUpdateDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeProposalDraft {
    kind: String,
    #[serde(default)]
    lens: Option<String>,
    criteria: String,
    items: Vec<ClaudeProposalItem>,
    #[serde(default)]
    guard_results: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeProposalItem {
    image_id: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    confidence: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudePresetUpdateDraft {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    purpose: Option<String>,
    prompt: String,
    #[serde(default)]
    criteria_json: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ClaudeInvocation {
    pub args: Vec<String>,
    #[cfg(test)]
    pub allowed_dirs: Vec<String>,
    #[cfg(test)]
    pub prompt: String,
    #[cfg(test)]
    pub removes_anthropic_api_key: bool,
}

pub async fn run_claude_agent_chat_turn_db(
    db: &Database,
    request: ClaudeAgentChatTurnRequest,
) -> Result<ClaudeAgentChatTurnResult, ServiceError> {
    validate_request(&request)?;
    let invocation = build_claude_invocation(&request)?;
    let output = run_claude_cli(invocation).await?;
    let envelope: ClaudeCliEnvelope = serde_json::from_str(&output)
        .map_err(|e| ServiceError::Engine(format!("Claude returned invalid JSON: {}", e)))?;
    if envelope.is_error == Some(true) {
        return Err(ServiceError::Engine(
            envelope
                .result
                .unwrap_or_else(|| "Claude returned an error".to_string()),
        ));
    }
    let decision = parse_structured_decision(&envelope.structured_output)?;
    persist_decision(db, request, decision, envelope, output)
}

pub fn build_claude_invocation(
    request: &ClaudeAgentChatTurnRequest,
) -> Result<ClaudeInvocation, ServiceError> {
    let model = request
        .model
        .clone()
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    validate_model_alias(&model)?;
    let budget = request
        .max_budget_usd
        .unwrap_or(DEFAULT_MAX_BUDGET_USD)
        .clamp(0.02, 2.0);
    let allowed_dirs = allowed_thumbnail_dirs(request);
    let mut args = vec![
        "--safe-mode".to_string(),
        "--print".to_string(),
        "--no-session-persistence".to_string(),
        "--model".to_string(),
        model,
        "--max-budget-usd".to_string(),
        format!("{budget:.2}"),
        "--output-format".to_string(),
        "json".to_string(),
        "--json-schema".to_string(),
        claude_decision_schema().to_string(),
    ];
    if allowed_dirs.is_empty() || request.visual_level == "text" {
        args.push("--tools".to_string());
        args.push("".to_string());
    } else {
        args.push("--tools".to_string());
        args.push("Read".to_string());
        args.push("--allowedTools".to_string());
        args.push("Read".to_string());
        for dir in &allowed_dirs {
            args.push("--add-dir".to_string());
            args.push(dir.clone());
        }
    }
    args.push(build_claude_prompt(request)?);

    Ok(ClaudeInvocation {
        args,
        #[cfg(test)]
        allowed_dirs,
        #[cfg(test)]
        prompt: build_claude_prompt(request)?,
        #[cfg(test)]
        removes_anthropic_api_key: true,
    })
}

fn validate_request(request: &ClaudeAgentChatTurnRequest) -> Result<(), ServiceError> {
    if request.instruction.trim().is_empty() {
        return Err(ServiceError::InvalidInput(
            "Agent instruction must not be empty".to_string(),
        ));
    }
    if !matches!(
        request.visual_level.as_str(),
        "text" | "tiny" | "preview" | "full"
    ) {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported visual level '{}'",
            request.visual_level
        )));
    }
    if request.candidate_images.is_empty() {
        return Err(ServiceError::InvalidInput(
            "Agent chat requires candidate image context".to_string(),
        ));
    }
    Ok(())
}

fn validate_model_alias(model: &str) -> Result<(), ServiceError> {
    if model.is_empty()
        || !model
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '[' | ']'))
    {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported Claude model alias '{}'",
            model
        )));
    }
    Ok(())
}

fn allowed_thumbnail_dirs(request: &ClaudeAgentChatTurnRequest) -> Vec<String> {
    let dirs: BTreeSet<String> = request
        .candidate_images
        .iter()
        .filter_map(|image| image.thumbnail_path.as_deref())
        .filter_map(|path| Path::new(path).parent())
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    dirs.into_iter().take(8).collect()
}

fn build_claude_prompt(request: &ClaudeAgentChatTurnRequest) -> Result<String, ServiceError> {
    let context = json!({
        "instruction": request.instruction,
        "visual_level": request.visual_level,
        "selected_count": request.selected_count,
        "visible_count": request.visible_count,
        "active_preset": request.preset,
        "candidate_images": request.candidate_images,
    });
    let context_json = serde_json::to_string_pretty(&context)
        .map_err(|e| ServiceError::Engine(format!("Failed to build Claude prompt: {}", e)))?;

    Ok(format!(
        r#"You are Claude inside Cull, a desktop image curation app.

Return only structured output matching the provided JSON schema.

Hard rules:
- Do not execute, apply, delete, trash, move, or mutate anything.
- You may only return intent: answer, update_preset, or create_proposal.
- Proposal kinds allowed in this bridge: select_images or trash_images.
- Proposal item image_id values must come from candidate_images.
- For trash_images, be conservative and explain uncertainty in guard_results.
- If the user asks to change the active preset, return operation update_preset.
- If the task is curation, return operation create_proposal.
- If there is not enough evidence, return answer and ask for a Preview escalation.
- The default visual level is tiny. Use thumbnail_path only as small visual context. Never ask for original files unless the user explicitly chooses Full in Cull.

Cull context:
{context_json}
"#
    ))
}

async fn run_claude_cli(invocation: ClaudeInvocation) -> Result<String, ServiceError> {
    let mut command = Command::new("claude");
    command
        .args(&invocation.args)
        .env_remove("ANTHROPIC_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn().map_err(|e| {
        ServiceError::Engine(format!(
            "Failed to start Claude Code. Is the claude CLI installed and authenticated? {}",
            e
        ))
    })?;
    let output = timeout(Duration::from_secs(120), child.wait_with_output())
        .await
        .map_err(|_| ServiceError::Engine("Claude timed out after 120 seconds".to_string()))?
        .map_err(ServiceError::Io)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ServiceError::Engine(format!(
            "Claude failed with status {}{}",
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_structured_decision(value: &Value) -> Result<ClaudeAgentDecision, ServiceError> {
    if value.is_object() {
        return serde_json::from_value(value.clone()).map_err(|e| {
            ServiceError::Engine(format!("Claude structured output was invalid: {}", e))
        });
    }
    if let Some(text) = value.as_str() {
        return serde_json::from_str(text).map_err(|e| {
            ServiceError::Engine(format!(
                "Claude structured output string was invalid: {}",
                e
            ))
        });
    }
    Err(ServiceError::Engine(
        "Claude did not return structured output".to_string(),
    ))
}

fn persist_decision(
    db: &Database,
    request: ClaudeAgentChatTurnRequest,
    decision: ClaudeAgentDecision,
    envelope: ClaudeCliEnvelope,
    raw_result_json: String,
) -> Result<ClaudeAgentChatTurnResult, ServiceError> {
    let usage_json = json!({
        "source": "claude_code_cli",
        "subscription_auth": true,
        "anthropic_api_key_removed": true,
        "usage": envelope.usage,
        "model_usage": envelope.model_usage,
        "total_cost_usd": envelope.total_cost_usd,
    })
    .to_string();

    let mut proposal = None;
    let mut updated_preset = None;
    match decision.operation.as_str() {
        "answer" => {}
        "update_preset" => {
            let active = request.preset.ok_or_else(|| {
                ServiceError::InvalidInput(
                    "Claude requested a preset update without an active preset".to_string(),
                )
            })?;
            let update = decision.preset_update.ok_or_else(|| {
                ServiceError::InvalidInput(
                    "Claude update_preset result omitted preset_update".to_string(),
                )
            })?;
            let criteria_json = update
                .criteria_json
                .map(|value| value.to_string())
                .unwrap_or(active.criteria_json);
            updated_preset = Some(proposals::upsert_agent_selection_preset_db(
                db,
                proposals::UpsertAgentSelectionPresetRequest {
                    id: Some(active.id),
                    name: update.name.unwrap_or(active.name),
                    purpose: update.purpose.unwrap_or(active.purpose),
                    prompt: update.prompt,
                    criteria_json,
                    sort_order: Some(active.sort_order),
                },
            )?);
        }
        "create_proposal" => {
            let draft = decision.proposal.ok_or_else(|| {
                ServiceError::InvalidInput(
                    "Claude create_proposal result omitted proposal".to_string(),
                )
            })?;
            proposal = Some(create_proposal_from_draft(
                db,
                &request,
                &draft,
                &usage_json,
                envelope.total_cost_usd,
            )?);
        }
        other => {
            return Err(ServiceError::InvalidInput(format!(
                "Unsupported Claude operation '{}'",
                other
            )));
        }
    }

    Ok(ClaudeAgentChatTurnResult {
        operation: decision.operation,
        message: decision.message,
        proposal,
        updated_preset,
        usage_json,
        raw_result_json,
    })
}

fn create_proposal_from_draft(
    db: &Database,
    request: &ClaudeAgentChatTurnRequest,
    draft: &ClaudeProposalDraft,
    usage_json: &str,
    total_cost_usd: Option<f64>,
) -> Result<AgentActionProposal, ServiceError> {
    if !matches!(draft.kind.as_str(), "select_images" | "trash_images") {
        return Err(ServiceError::InvalidInput(format!(
            "Claude returned unsupported proposal kind '{}'",
            draft.kind
        )));
    }
    let allowed: BTreeSet<&str> = request
        .candidate_images
        .iter()
        .map(|image| image.image_id.as_str())
        .collect();
    if draft.items.is_empty() {
        return Err(ServiceError::InvalidInput(
            "Claude proposal requires at least one item".to_string(),
        ));
    }
    for item in &draft.items {
        if !allowed.contains(item.image_id.as_str()) {
            return Err(ServiceError::InvalidInput(format!(
                "Claude returned image_id '{}' outside the candidate context",
                item.image_id
            )));
        }
    }
    let usage_value: Value = serde_json::from_str(usage_json)
        .map_err(|e| ServiceError::Engine(format!("Invalid usage JSON: {}", e)))?;
    let source_context_json = json!({
        "source": "claude_code_cli",
        "selected_count": request.selected_count,
        "visible_count": request.visible_count,
        "candidate_count": request.candidate_images.len(),
        "active_preset_id": request.preset.as_ref().map(|preset| preset.id.as_str()),
        "runtime": usage_value,
    })
    .to_string();
    let estimated_input_tokens = usage_value
        .pointer("/usage/input_tokens")
        .and_then(Value::as_i64)
        .map(|base| {
            base + usage_value
                .pointer("/usage/cache_creation_input_tokens")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                + usage_value
                    .pointer("/usage/cache_read_input_tokens")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
        });
    let estimated_output_tokens = usage_value
        .pointer("/usage/output_tokens")
        .and_then(Value::as_i64);
    let estimated_cost_eur =
        total_cost_usd.map(|usd| (usd * USD_TO_EUR_DISPLAY_ESTIMATE * 1000.0).round() / 1000.0);

    proposals::create_action_proposal_db(
        db,
        proposals::CreateActionProposalRequest {
            kind: draft.kind.clone(),
            persona: "copilot".to_string(),
            lens: draft.lens.clone(),
            criteria: draft.criteria.clone(),
            visual_level: request.visual_level.clone(),
            selection_preset_id: request.preset.as_ref().map(|preset| preset.id.clone()),
            estimated_input_tokens,
            estimated_output_tokens,
            estimated_cost_eur,
            source_context_json,
            items_json: serde_json::to_string(&draft.items)
                .map_err(|e| ServiceError::Engine(format!("Invalid proposal items: {}", e)))?,
            guard_results_json: draft.guard_results.to_string(),
        },
    )
}

fn claude_decision_schema() -> &'static str {
    r#"{"type":"object","properties":{"operation":{"type":"string","enum":["answer","create_proposal","update_preset"]},"message":{"type":"string"},"proposal":{"type":"object","properties":{"kind":{"type":"string","enum":["select_images","trash_images"]},"lens":{"type":["string","null"]},"criteria":{"type":"string"},"items":{"type":"array","items":{"type":"object","properties":{"image_id":{"type":"string"},"reason":{"type":"string"},"confidence":{}},"required":["image_id","reason"],"additionalProperties":false},"minItems":1},"guard_results":{"type":"object"}},"required":["kind","criteria","items"],"additionalProperties":false},"preset_update":{"type":"object","properties":{"name":{"type":"string"},"purpose":{"type":"string"},"prompt":{"type":"string"},"criteria_json":{"type":"object"}},"required":["prompt"],"additionalProperties":false}},"required":["operation","message"],"additionalProperties":false}"#
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> ClaudeAgentChatTurnRequest {
        ClaudeAgentChatTurnRequest {
            instruction: "select the strongest portfolio image".to_string(),
            visual_level: "tiny".to_string(),
            preset: Some(AgentSelectionPreset {
                id: "selpreset_portfolio".to_string(),
                name: "Portfolio".to_string(),
                purpose: "portfolio".to_string(),
                prompt: "Select portfolio candidates".to_string(),
                criteria_json: "{}".to_string(),
                sort_order: 10,
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            }),
            candidate_images: vec![AgentChatImageContext {
                image_id: "img_a".to_string(),
                filename: Some("a.png".to_string()),
                width: Some(800),
                height: Some(600),
                format: Some("png".to_string()),
                star_rating: Some(4),
                color_label: None,
                decision: Some("undecided".to_string()),
                source_label: Some("midjourney".to_string()),
                thumbnail_path: Some("/tmp/cull/thumbs/a.png".to_string()),
            }],
            selected_count: 1,
            visible_count: 12,
            model: Some("haiku".to_string()),
            max_budget_usd: Some(0.05),
        }
    }

    #[test]
    fn invocation_uses_subscription_auth_and_thumbnail_read_only_context() {
        let invocation = build_claude_invocation(&sample_request()).unwrap();
        assert!(invocation.removes_anthropic_api_key);
        assert!(invocation
            .args
            .windows(2)
            .any(|pair| pair == ["--tools", "Read"]));
        assert!(invocation
            .args
            .windows(2)
            .any(|pair| pair == ["--allowedTools", "Read"]));
        assert!(invocation.args.contains(&"--json-schema".to_string()));
        assert!(invocation
            .allowed_dirs
            .contains(&"/tmp/cull/thumbs".to_string()));
        assert!(invocation.prompt.contains("thumbnail_path"));
        assert!(!invocation.prompt.contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn text_only_invocation_disables_tools() {
        let mut request = sample_request();
        request.visual_level = "text".to_string();
        let invocation = build_claude_invocation(&request).unwrap();
        assert!(invocation
            .args
            .windows(2)
            .any(|pair| pair == ["--tools", ""]));
    }

    #[test]
    fn parses_structured_output_from_cli_envelope() {
        let envelope = json!({
            "structured_output": {
                "operation": "create_proposal",
                "message": "Review one candidate",
                "proposal": {
                    "kind": "select_images",
                    "lens": "portfolio",
                    "criteria": "strong composition",
                    "items": [{"image_id":"img_a","reason":"best composition","confidence":0.82}],
                    "guard_results": {"blocked":[]}
                }
            }
        });
        let parsed: ClaudeCliEnvelope = serde_json::from_value(envelope).unwrap();
        let decision = parse_structured_decision(&parsed.structured_output).unwrap();
        assert_eq!(decision.operation, "create_proposal");
        assert_eq!(decision.proposal.unwrap().items[0].image_id, "img_a");
    }

    #[test]
    fn rejects_proposal_ids_outside_candidate_context() {
        let db = {
            let dir = tempfile::tempdir().unwrap();
            Database::open(&dir.path().join("test.db")).unwrap()
        };
        let draft = ClaudeProposalDraft {
            kind: "select_images".to_string(),
            lens: Some("portfolio".to_string()),
            criteria: "strong".to_string(),
            items: vec![ClaudeProposalItem {
                image_id: "other".to_string(),
                reason: Some("not in context".to_string()),
                confidence: None,
            }],
            guard_results: json!({}),
        };
        let err = create_proposal_from_draft(
            &db,
            &sample_request(),
            &draft,
            r#"{"usage":{"input_tokens":1,"output_tokens":1}}"#,
            Some(0.01),
        )
        .unwrap_err();
        assert!(err.to_string().contains("outside the candidate context"));
    }

    #[test]
    fn update_preset_decision_persists_agent_chat_edit() {
        let db = {
            let dir = tempfile::tempdir().unwrap();
            Database::open(&dir.path().join("test.db")).unwrap()
        };
        let active_preset = db
            .list_agent_selection_presets()
            .unwrap()
            .into_iter()
            .find(|preset| preset.id == "selpreset_portfolio")
            .unwrap();
        let mut request = sample_request();
        request.preset = Some(active_preset.clone());

        let result = persist_decision(
            &db,
            request,
            ClaudeAgentDecision {
                operation: "update_preset".to_string(),
                message: "Updated the portfolio selection preset.".to_string(),
                proposal: None,
                preset_update: Some(ClaudePresetUpdateDraft {
                    name: Some("Portfolio tight edit".to_string()),
                    purpose: Some("portfolio".to_string()),
                    prompt: "Select only cohesive, publication-ready portfolio images.".to_string(),
                    criteria_json: Some(json!({"agent_edited": true})),
                }),
            },
            ClaudeCliEnvelope {
                structured_output: json!({}),
                usage: json!({"input_tokens": 10, "output_tokens": 4}),
                model_usage: json!({}),
                total_cost_usd: Some(0.01),
                result: None,
                is_error: Some(false),
            },
            "{}".to_string(),
        )
        .unwrap();

        let updated = result.updated_preset.unwrap();
        assert_eq!(updated.id, active_preset.id);
        assert_eq!(updated.name, "Portfolio tight edit");
        assert!(updated.prompt.contains("publication-ready"));

        let stored = db
            .get_agent_selection_preset("selpreset_portfolio")
            .unwrap()
            .unwrap();
        assert_eq!(stored.prompt, updated.prompt);
        assert_eq!(stored.criteria_json, r#"{"agent_edited":true}"#);
    }
}
