use crate::db_core::db::Database;
use crate::db_core::models::{AgentActionProposal, AgentSelectionPreset};
use crate::services::agent_proposals as proposals;
use crate::services::ServiceError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

const DEFAULT_MODEL: &str = "haiku";
const DEFAULT_MAX_BUDGET_USD: f64 = 0.50;
const USD_TO_EUR_DISPLAY_ESTIMATE: f64 = 0.93;
const SDK_RUNNER_RESOURCE_NAME: &str = "claude-agent-sdk-runner.mjs";
pub const CLAUDE_AGENT_STREAM_EVENT: &str = "claude-agent:stream-event";
const SDK_EVENT_PREFIX: &str = "CULL_AGENT_EVENT ";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeAgentChatTurnRequest {
    #[serde(default)]
    pub request_id: Option<String>,
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
pub struct ClaudeAgentStreamEvent {
    pub request_id: String,
    pub sequence: u32,
    pub phase: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
    pub is_final: bool,
    pub is_error: bool,
}

#[derive(Clone)]
struct ClaudeAgentStreamEmitter {
    app: AppHandle,
    request_id: String,
    sequence: Arc<AtomicU32>,
}

impl ClaudeAgentStreamEmitter {
    fn new(app: AppHandle, request_id: String) -> Self {
        Self {
            app,
            request_id,
            sequence: Arc::new(AtomicU32::new(0)),
        }
    }

    fn emit(&self, phase: &str, message: impl Into<String>, details: Value) {
        self.emit_with_flags(phase, message, details, false, false);
    }

    fn emit_final(&self, phase: &str, message: impl Into<String>, details: Value) {
        self.emit_with_flags(phase, message, details, true, false);
    }

    fn emit_error(&self, message: impl Into<String>) {
        self.emit_with_flags("error", message, Value::Null, true, true);
    }

    fn emit_sdk_event(&self, value: Value) {
        let phase = value
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or("sdk_event")
            .to_string();
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("SDK event")
            .to_string();
        let details = value.get("details").cloned().unwrap_or(Value::Null);
        let is_error = phase.contains("error");
        self.emit_with_flags(&phase, message, details, false, is_error);
    }

    fn emit_with_flags(
        &self,
        phase: &str,
        message: impl Into<String>,
        details: Value,
        is_final: bool,
        is_error: bool,
    ) {
        let payload = ClaudeAgentStreamEvent {
            request_id: self.request_id.clone(),
            sequence: self.sequence.fetch_add(1, Ordering::SeqCst) + 1,
            phase: phase.to_string(),
            message: message.into(),
            details,
            is_final,
            is_error,
        };
        let _ = self.app.emit(CLAUDE_AGENT_STREAM_EVENT, payload);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeCliEnvelope {
    #[serde(default)]
    runtime: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudeRuntimeSource {
    AgentSdk,
    CliFallback,
}

impl ClaudeRuntimeSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::AgentSdk => "claude_agent_sdk",
            Self::CliFallback => "claude_code_cli_fallback",
        }
    }
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
    pub sdk_request_json: String,
    pub runner_path: Option<PathBuf>,
    #[cfg(test)]
    pub allowed_dirs: Vec<String>,
    #[cfg(test)]
    pub prompt: String,
    #[cfg(test)]
    pub removes_anthropic_api_key: bool,
}

pub async fn run_claude_agent_chat_turn_db_with_events(
    db: &Database,
    request: ClaudeAgentChatTurnRequest,
    app: Option<AppHandle>,
) -> Result<ClaudeAgentChatTurnResult, ServiceError> {
    let request_id = request
        .request_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let emitter = app.map(|app| ClaudeAgentStreamEmitter::new(app, request_id));
    if let Some(emitter) = emitter.as_ref() {
        emitter.emit(
            "queued",
            "Preparing Claude request",
            json!({
                "selected_count": request.selected_count,
                "visible_count": request.visible_count,
                "candidate_count": request.candidate_images.len(),
                "visual_level": request.visual_level,
            }),
        );
    }
    let result = run_claude_agent_chat_turn_inner(db, request, emitter.clone()).await;
    if let (Some(emitter), Err(error)) = (emitter.as_ref(), result.as_ref()) {
        emitter.emit_error(error.to_string());
    }
    result
}

async fn run_claude_agent_chat_turn_inner(
    db: &Database,
    request: ClaudeAgentChatTurnRequest,
    emitter: Option<ClaudeAgentStreamEmitter>,
) -> Result<ClaudeAgentChatTurnResult, ServiceError> {
    validate_request(&request)?;
    let invocation = build_claude_invocation(&request)?;
    if let Some(emitter) = emitter.as_ref() {
        emitter.emit(
            "context",
            format!(
                "Sending {} candidates as {} context",
                request.candidate_images.len(),
                request.visual_level
            ),
            json!({
                "candidate_count": request.candidate_images.len(),
                "selected_count": request.selected_count,
                "visual_level": request.visual_level,
            }),
        );
    }
    let (output, runtime_source) = run_claude_agent_runtime(invocation, emitter.clone()).await?;
    if let Some(emitter) = emitter.as_ref() {
        emitter.emit(
            "parse",
            "Parsing Claude result",
            json!({ "runtime": runtime_source.as_str() }),
        );
    }
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
    let result = persist_decision(db, request, decision, envelope, output, runtime_source)?;
    if let Some(emitter) = emitter.as_ref() {
        emitter.emit_final(
            "complete",
            result.message.clone(),
            json!({
                "operation": result.operation,
                "has_proposal": result.proposal.is_some(),
                "has_updated_preset": result.updated_preset.is_some(),
            }),
        );
    }
    Ok(result)
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
    let prompt = build_claude_prompt(request)?;
    let sdk_request_json = serde_json::to_string(&json!({
        "prompt": prompt,
        "model": model.clone(),
        "max_budget_usd": budget,
        "visual_level": request.visual_level,
        "allowed_dirs": allowed_dirs,
        "schema": claude_decision_schema_value()?,
    }))
    .map_err(|e| ServiceError::Engine(format!("Failed to build Claude SDK request: {}", e)))?;
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
    args.push(prompt.clone());

    Ok(ClaudeInvocation {
        args,
        sdk_request_json,
        runner_path: resolve_agent_sdk_runner(),
        #[cfg(test)]
        allowed_dirs,
        #[cfg(test)]
        prompt,
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

    let schema_json = serde_json::to_string_pretty(&claude_decision_schema_value()?)
        .map_err(|e| ServiceError::Engine(format!("Failed to build Claude schema: {}", e)))?;

    Ok(format!(
        r#"You are Claude inside Cull, a desktop image curation app.

Return only one JSON object matching this JSON schema. Do not wrap it in Markdown.

JSON schema:
{schema_json}

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

async fn run_claude_agent_runtime(
    invocation: ClaudeInvocation,
    emitter: Option<ClaudeAgentStreamEmitter>,
) -> Result<(String, ClaudeRuntimeSource), ServiceError> {
    if std::env::var("CULL_AGENT_RUNTIME").as_deref() == Ok("cli") {
        if let Some(emitter) = emitter.as_ref() {
            emitter.emit("runtime", "Starting Claude CLI fallback", Value::Null);
        }
        return run_claude_cli(invocation)
            .await
            .map(|output| (output, ClaudeRuntimeSource::CliFallback));
    }
    match invocation.runner_path.as_ref() {
        Some(path) => {
            if let Some(emitter) = emitter.as_ref() {
                emitter.emit("runtime", "Starting Claude Agent SDK", Value::Null);
            }
            run_claude_agent_sdk(path, &invocation.sdk_request_json, emitter)
                .await
                .map(|output| (output, ClaudeRuntimeSource::AgentSdk))
        }
        None => {
            if let Some(emitter) = emitter.as_ref() {
                emitter.emit("runtime", "Starting Claude CLI fallback", Value::Null);
            }
            run_claude_cli(invocation)
                .await
                .map(|output| (output, ClaudeRuntimeSource::CliFallback))
        }
    }
}

async fn run_claude_agent_sdk(
    runner_path: &Path,
    request_json: &str,
    emitter: Option<ClaudeAgentStreamEmitter>,
) -> Result<String, ServiceError> {
    let mut command = Command::new("node");
    command
        .arg(runner_path)
        .env_remove("ANTHROPIC_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|e| {
        ServiceError::Engine(format!(
            "Failed to start Claude Agent SDK runner at '{}': {}",
            runner_path.display(),
            e
        ))
    })?;
    {
        let mut stdin = child.stdin.take().ok_or_else(|| {
            ServiceError::Engine("Failed to open Claude Agent SDK runner stdin".to_string())
        })?;
        stdin
            .write_all(request_json.as_bytes())
            .await
            .map_err(ServiceError::Io)?;
        stdin.write_all(b"\n").await.map_err(ServiceError::Io)?;
    }

    let stdout = child.stdout.take().ok_or_else(|| {
        ServiceError::Engine("Failed to open Claude Agent SDK runner stdout".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        ServiceError::Engine("Failed to open Claude Agent SDK runner stderr".to_string())
    })?;
    let stdout_task = tokio::spawn(async move {
        let mut output = String::new();
        let mut reader = BufReader::new(stdout);
        reader.read_to_string(&mut output).await?;
        Ok::<String, std::io::Error>(output)
    });
    let stderr_task = tokio::spawn(read_sdk_stderr_events(stderr, emitter));
    let status = timeout(Duration::from_secs(120), child.wait())
        .await
        .map_err(|_| {
            ServiceError::Engine("Claude Agent SDK timed out after 120 seconds".to_string())
        })?
        .map_err(ServiceError::Io)?;
    let stdout = stdout_task
        .await
        .map_err(|e| ServiceError::Engine(format!("Claude Agent SDK stdout task failed: {}", e)))?
        .map_err(ServiceError::Io)?;
    let stderr = stderr_task
        .await
        .map_err(|e| ServiceError::Engine(format!("Claude Agent SDK stderr task failed: {}", e)))?
        .map_err(ServiceError::Io)?;

    if !status.success() {
        let stderr = stderr.trim().to_string();
        return Err(ServiceError::Engine(format!(
            "Claude Agent SDK failed with status {}{}",
            status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }
    Ok(stdout)
}

async fn read_sdk_stderr_events(
    stderr: tokio::process::ChildStderr,
    emitter: Option<ClaudeAgentStreamEmitter>,
) -> Result<String, std::io::Error> {
    let mut lines = BufReader::new(stderr).lines();
    let mut stderr_lines = Vec::new();
    while let Some(line) = lines.next_line().await? {
        if let Some(payload) = line.strip_prefix(SDK_EVENT_PREFIX) {
            if let (Some(emitter), Ok(value)) =
                (emitter.as_ref(), serde_json::from_str::<Value>(payload))
            {
                emitter.emit_sdk_event(value);
            }
        } else {
            stderr_lines.push(line);
        }
    }
    Ok(stderr_lines.join("\n"))
}

fn resolve_agent_sdk_runner() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CULL_AGENT_SDK_RUNNER") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(app_contents) = exe_path
            .parent()
            .and_then(|path| path.parent())
            .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("Contents"))
        {
            let resource_path = app_contents
                .join("Resources")
                .join(SDK_RUNNER_RESOURCE_NAME);
            if resource_path.exists() {
                return Some(resource_path);
            }
        }
    }

    let repo_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts")
        .join(SDK_RUNNER_RESOURCE_NAME);
    if repo_path.exists() {
        return Some(repo_path);
    }
    None
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
    runtime_source: ClaudeRuntimeSource,
) -> Result<ClaudeAgentChatTurnResult, ServiceError> {
    let usage_json = json!({
        "source": runtime_source.as_str(),
        "sdk_reported_runtime": envelope.runtime,
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
                runtime_source,
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
    runtime_source: ClaudeRuntimeSource,
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
        "source": runtime_source.as_str(),
        "selected_count": request.selected_count,
        "visible_count": request.visible_count,
        "candidate_count": request.candidate_images.len(),
        "active_preset_id": request.preset.as_ref().map(|preset| preset.id.as_str()),
        "runtime": usage_value,
    })
    .to_string();
    let estimated_input_tokens = estimated_input_tokens_from_usage(&usage_value);
    let estimated_output_tokens = estimated_output_tokens_from_usage(&usage_value);
    let estimated_cost_eur = estimated_cost_usd_from_usage(&usage_value, total_cost_usd)
        .map(|usd| (usd * USD_TO_EUR_DISPLAY_ESTIMATE * 1000.0).round() / 1000.0);

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

fn estimated_input_tokens_from_usage(usage_value: &Value) -> Option<i64> {
    usage_value
        .get("usage")
        .and_then(usage_input_tokens)
        .or_else(|| {
            usage_value
                .get("model_usage")
                .or_else(|| usage_value.get("modelUsage"))
                .and_then(sum_model_input_tokens)
        })
}

fn estimated_output_tokens_from_usage(usage_value: &Value) -> Option<i64> {
    usage_value
        .get("usage")
        .and_then(|usage| token_field(usage, "output_tokens", "outputTokens"))
        .or_else(|| {
            usage_value
                .get("model_usage")
                .or_else(|| usage_value.get("modelUsage"))
                .and_then(sum_model_output_tokens)
        })
}

fn estimated_cost_usd_from_usage(usage_value: &Value, total_cost_usd: Option<f64>) -> Option<f64> {
    total_cost_usd
        .or_else(|| usage_value.get("total_cost_usd").and_then(Value::as_f64))
        .or_else(|| {
            usage_value
                .get("model_usage")
                .or_else(|| usage_value.get("modelUsage"))
                .and_then(sum_model_cost_usd)
        })
}

fn usage_input_tokens(usage: &Value) -> Option<i64> {
    let base = token_field(usage, "input_tokens", "inputTokens")?;
    Some(
        base + token_field(
            usage,
            "cache_creation_input_tokens",
            "cacheCreationInputTokens",
        )
        .unwrap_or(0)
            + token_field(usage, "cache_read_input_tokens", "cacheReadInputTokens").unwrap_or(0),
    )
}

fn token_field(value: &Value, snake: &str, camel: &str) -> Option<i64> {
    value
        .get(snake)
        .or_else(|| value.get(camel))
        .and_then(Value::as_i64)
}

fn cost_field(value: &Value, snake: &str, camel: &str) -> Option<f64> {
    value
        .get(snake)
        .or_else(|| value.get(camel))
        .and_then(Value::as_f64)
}

fn sum_model_input_tokens(model_usage: &Value) -> Option<i64> {
    let models = model_usage.as_object()?;
    if models.is_empty() {
        return None;
    }
    let mut total = 0;
    let mut seen = false;
    for usage in models.values() {
        if let Some(tokens) = usage_input_tokens(usage) {
            total += tokens;
            seen = true;
        }
    }
    seen.then_some(total)
}

fn sum_model_output_tokens(model_usage: &Value) -> Option<i64> {
    let models = model_usage.as_object()?;
    if models.is_empty() {
        return None;
    }
    let mut total = 0;
    let mut seen = false;
    for usage in models.values() {
        if let Some(tokens) = token_field(usage, "output_tokens", "outputTokens") {
            total += tokens;
            seen = true;
        }
    }
    seen.then_some(total)
}

fn sum_model_cost_usd(model_usage: &Value) -> Option<f64> {
    let models = model_usage.as_object()?;
    if models.is_empty() {
        return None;
    }
    let mut total = 0.0;
    let mut seen = false;
    for usage in models.values() {
        if let Some(cost) = cost_field(usage, "cost_usd", "costUSD") {
            total += cost;
            seen = true;
        }
    }
    seen.then_some(total)
}

fn claude_decision_schema() -> &'static str {
    r#"{"type":"object","properties":{"operation":{"type":"string","enum":["answer","create_proposal","update_preset"]},"message":{"type":"string"},"proposal":{"type":"object","properties":{"kind":{"type":"string","enum":["select_images","trash_images"]},"lens":{"type":["string","null"]},"criteria":{"type":"string"},"items":{"type":"array","items":{"type":"object","properties":{"image_id":{"type":"string"},"reason":{"type":"string"},"confidence":{}},"required":["image_id","reason"],"additionalProperties":false},"minItems":1},"guard_results":{"type":"object"}},"required":["kind","criteria","items"],"additionalProperties":false},"preset_update":{"type":"object","properties":{"name":{"type":"string"},"purpose":{"type":"string"},"prompt":{"type":"string"},"criteria_json":{"type":"object"}},"required":["prompt"],"additionalProperties":false}},"required":["operation","message"],"additionalProperties":false}"#
}

fn claude_decision_schema_value() -> Result<Value, ServiceError> {
    serde_json::from_str(claude_decision_schema())
        .map_err(|e| ServiceError::Engine(format!("Invalid Claude decision schema: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::sync::Mutex as AsyncMutex;

    static AGENT_ENV_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

    fn sample_request() -> ClaudeAgentChatTurnRequest {
        ClaudeAgentChatTurnRequest {
            request_id: Some("agent-request-test".to_string()),
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

    fn test_db() -> Database {
        let dir = tempfile::tempdir().unwrap();
        Database::open(&dir.path().join("test.db")).unwrap()
    }

    fn fake_sdk_runner(dir: &Path) -> PathBuf {
        let runner_path = dir.join("fake-claude-agent-sdk-runner.mjs");
        fs::write(
            &runner_path,
            r#"#!/usr/bin/env node
import { readFileSync } from 'node:fs';
const request = JSON.parse(readFileSync(0, 'utf8'));
if (process.env.ANTHROPIC_API_KEY) {
  console.error('ANTHROPIC_API_KEY leaked into runner');
  process.exit(1);
}
const structured = JSON.parse(process.env.CULL_FAKE_AGENT_RESPONSE);
const inputTokens = request.visual_level === 'tiny' ? 2100 : 4200;
process.stdout.write(JSON.stringify({
  runtime: 'claude_agent_sdk',
  structured_output: structured,
  usage: { input_tokens: inputTokens, output_tokens: 64 },
  modelUsage: { [request.model]: { input_tokens: inputTokens, output_tokens: 64 } },
  total_cost_usd: 0.014,
  result: JSON.stringify(structured),
  is_error: false,
  seen_tools: request.visual_level === 'text' ? [] : ['Read'],
  seen_allowed_dirs: request.allowed_dirs
}) + '\n');
"#,
        )
        .unwrap();
        runner_path
    }

    async fn run_with_fake_sdk(
        db: &Database,
        mut request: ClaudeAgentChatTurnRequest,
        structured_output: Value,
    ) -> Result<ClaudeAgentChatTurnResult, ServiceError> {
        let _guard = AGENT_ENV_LOCK.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let runner_path = fake_sdk_runner(dir.path());
        request.model = Some("haiku".to_string());
        std::env::set_var("CULL_AGENT_SDK_RUNNER", runner_path);
        std::env::set_var("CULL_FAKE_AGENT_RESPONSE", structured_output.to_string());
        std::env::set_var("ANTHROPIC_API_KEY", "must-not-leak");
        let result = run_claude_agent_chat_turn_db_with_events(db, request, None).await;
        std::env::remove_var("CULL_AGENT_SDK_RUNNER");
        std::env::remove_var("CULL_FAKE_AGENT_RESPONSE");
        std::env::remove_var("ANTHROPIC_API_KEY");
        result
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
    fn invocation_uses_practical_default_budget_when_ui_leaves_budget_unset() {
        let mut request = sample_request();
        request.max_budget_usd = None;
        let invocation = build_claude_invocation(&request).unwrap();
        let sdk_request: Value = serde_json::from_str(&invocation.sdk_request_json).unwrap();

        assert_eq!(sdk_request["max_budget_usd"], json!(0.50));
        assert!(invocation
            .args
            .windows(2)
            .any(|pair| pair == ["--max-budget-usd", "0.50"]));
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
        let sdk_request: Value = serde_json::from_str(&invocation.sdk_request_json).unwrap();
        assert_eq!(sdk_request["visual_level"], "text");
        assert_eq!(sdk_request["allowed_dirs"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn sdk_answer_task_returns_message_without_persisting_proposal() {
        let db = test_db();
        let result = run_with_fake_sdk(
            &db,
            sample_request(),
            json!({
                "operation": "answer",
                "message": "I need Preview before making a selection."
            }),
        )
        .await
        .unwrap();

        assert_eq!(result.operation, "answer");
        assert!(result.proposal.is_none());
        assert_eq!(db.list_action_proposals(None, 20).unwrap().len(), 0);
        let usage: Value = serde_json::from_str(&result.usage_json).unwrap();
        assert_eq!(usage["source"], "claude_agent_sdk");
        assert_eq!(usage["anthropic_api_key_removed"], true);
    }

    #[tokio::test]
    async fn sdk_select_images_task_persists_pending_proposal() {
        let db = test_db();
        let result = run_with_fake_sdk(
            &db,
            sample_request(),
            json!({
                "operation": "create_proposal",
                "message": "I found one strong candidate.",
                "proposal": {
                    "kind": "select_images",
                    "lens": "portfolio",
                    "criteria": "strong composition",
                    "items": [{"image_id": "img_a", "reason": "best composition", "confidence": 0.86}],
                    "guard_results": {"blocked": []}
                }
            }),
        )
        .await
        .unwrap();

        let proposal = result.proposal.unwrap();
        assert_eq!(proposal.kind, "select_images");
        assert_eq!(proposal.status, "pending");
        assert_eq!(proposal.estimated_input_tokens, Some(2100));
        assert_eq!(
            db.list_action_proposals(Some("pending"), 20).unwrap().len(),
            1
        );
        let source_context: Value = serde_json::from_str(&proposal.source_context_json).unwrap();
        assert_eq!(source_context["source"], "claude_agent_sdk");
    }

    #[tokio::test]
    async fn sdk_trash_images_task_persists_pending_proposal_without_applying() {
        let db = test_db();
        let result = run_with_fake_sdk(
            &db,
            sample_request(),
            json!({
                "operation": "create_proposal",
                "message": "I can queue this as a trash proposal.",
                "proposal": {
                    "kind": "trash_images",
                    "lens": "near_duplicates",
                    "criteria": "visually weaker near duplicate",
                    "items": [{"image_id": "img_a", "reason": "weaker duplicate candidate", "confidence": "medium"}],
                    "guard_results": {"requires_user_accept": true}
                }
            }),
        )
        .await
        .unwrap();

        let proposal = result.proposal.unwrap();
        assert_eq!(proposal.kind, "trash_images");
        assert_eq!(proposal.status, "pending");
        assert!(proposal.applied_at.is_none());
        assert!(proposal.guard_results_json.contains("requires_user_accept"));
    }

    #[tokio::test]
    async fn sdk_update_preset_task_persists_active_preset_edit() {
        let db = test_db();
        let active_preset = db
            .list_agent_selection_presets()
            .unwrap()
            .into_iter()
            .find(|preset| preset.id == "selpreset_portfolio")
            .unwrap();
        let mut request = sample_request();
        request.preset = Some(active_preset.clone());
        let result = run_with_fake_sdk(
            &db,
            request,
            json!({
                "operation": "update_preset",
                "message": "Updated the active preset.",
                "preset_update": {
                    "name": "Portfolio agent edit",
                    "purpose": "portfolio",
                    "prompt": "Select only cohesive, publication-ready images.",
                    "criteria_json": {"agent_edited": true}
                }
            }),
        )
        .await
        .unwrap();

        let updated = result.updated_preset.unwrap();
        assert_eq!(updated.id, active_preset.id);
        assert_eq!(updated.name, "Portfolio agent edit");
        assert!(updated.prompt.contains("publication-ready"));
        assert_eq!(
            db.get_agent_selection_preset(&active_preset.id)
                .unwrap()
                .unwrap()
                .criteria_json,
            r#"{"agent_edited":true}"#
        );
    }

    #[tokio::test]
    async fn sdk_rejects_create_proposal_with_image_id_outside_context() {
        let db = test_db();
        let err = run_with_fake_sdk(
            &db,
            sample_request(),
            json!({
                "operation": "create_proposal",
                "message": "Bad ID",
                "proposal": {
                    "kind": "select_images",
                    "lens": "portfolio",
                    "criteria": "strong",
                    "items": [{"image_id": "img_missing", "reason": "not in context", "confidence": 0.1}],
                    "guard_results": {}
                }
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("outside the candidate context"));
        assert_eq!(db.list_action_proposals(None, 20).unwrap().len(), 0);
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
            ClaudeRuntimeSource::AgentSdk,
        )
        .unwrap_err();
        assert!(err.to_string().contains("outside the candidate context"));
    }

    #[test]
    fn proposal_estimates_fall_back_to_sdk_model_usage() {
        let db = test_db();
        let draft = ClaudeProposalDraft {
            kind: "select_images".to_string(),
            lens: Some("portfolio".to_string()),
            criteria: "strong".to_string(),
            items: vec![ClaudeProposalItem {
                image_id: "img_a".to_string(),
                reason: Some("best candidate".to_string()),
                confidence: None,
            }],
            guard_results: json!({}),
        };

        let proposal = create_proposal_from_draft(
            &db,
            &sample_request(),
            &draft,
            r#"{
                "usage": {},
                "model_usage": {
                    "claude-haiku": {
                        "inputTokens": 2000,
                        "cacheCreationInputTokens": 75,
                        "cacheReadInputTokens": 25,
                        "outputTokens": 64,
                        "costUSD": 0.014
                    }
                }
            }"#,
            None,
            ClaudeRuntimeSource::AgentSdk,
        )
        .unwrap();

        assert_eq!(proposal.estimated_input_tokens, Some(2100));
        assert_eq!(proposal.estimated_output_tokens, Some(64));
        assert_eq!(proposal.estimated_cost_eur, Some(0.013));
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
                runtime: Some("claude_agent_sdk".to_string()),
                structured_output: json!({}),
                usage: json!({"input_tokens": 10, "output_tokens": 4}),
                model_usage: json!({}),
                total_cost_usd: Some(0.01),
                result: None,
                is_error: Some(false),
            },
            "{}".to_string(),
            ClaudeRuntimeSource::AgentSdk,
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
