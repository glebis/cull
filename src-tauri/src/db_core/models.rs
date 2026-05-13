use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    pub sha256_hash: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub file_size: u64,
    pub created_at: String,
    pub imported_at: String,
    pub ai_prompt: Option<String>,
    pub raw_metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFile {
    pub id: String,
    pub image_id: String,
    pub path: String,
    pub last_seen_at: String,
    pub missing_at: Option<String>,
    pub last_seen_size: Option<u64>,
    pub last_seen_mtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub image_id: String,
    pub project_id: Option<String>,
    pub star_rating: Option<u8>,
    pub color_label: Option<String>,
    pub decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageWithFile {
    pub image: Image,
    pub path: String,
    pub thumbnail_path: Option<String>,
    pub selection: Option<Selection>,
    pub source_label: Option<String>,
    pub missing_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToken {
    pub id: String,
    pub name: String,
    pub role: String,
    pub scope_json: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TokenScope {
    pub collections: Option<Vec<String>>,
    pub folders: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub token_id: Option<String>,
    pub tool_name: String,
    pub params_json: Option<String>,
    pub result_status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRun {
    pub id: String,
    pub prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub settings_json: String,
    pub seed: Option<String>,
    pub parent_run_id: Option<String>,
    pub source_type: String,
    pub source_path: Option<String>,
    pub raw_metadata_json: Option<String>,
    pub created_at: Option<String>,
    pub imported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewModelRun {
    pub id: String,
    pub job_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub profile_id: Option<String>,
    pub task: String,
    pub provider: String,
    pub model_id: String,
    pub model_revision: Option<String>,
    pub status: String,
    pub input_scope_json: String,
    pub params_json: String,
    pub output_summary_json: String,
    pub cost_estimate_usd: Option<f64>,
    pub cost_actual_usd: Option<f64>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRun {
    pub id: String,
    pub job_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub profile_id: Option<String>,
    pub task: String,
    pub provider: String,
    pub model_id: String,
    pub model_revision: Option<String>,
    pub status: String,
    pub input_scope_json: String,
    pub params_json: String,
    pub output_summary_json: String,
    pub cost_estimate_usd: Option<f64>,
    pub cost_actual_usd: Option<f64>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewModelRunItem {
    pub id: String,
    pub run_id: String,
    pub image_id: Option<String>,
    pub input_asset_uri: String,
    pub input_hash: Option<String>,
    pub status: String,
    pub output_ref_kind: Option<String>,
    pub output_ref_id: Option<String>,
    pub audit_payload_json: Option<String>,
    pub cost_usd: Option<f64>,
    pub attempt_count: u32,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSessionEvent {
    pub session_id: Option<String>,
    pub event_type: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub id: String,
    pub session_id: Option<String>,
    pub event_type: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLibrarySummary {
    pub total_images: u32,
    pub scoped_images: u32,
    pub rated_images: u32,
    pub accepted_images: u32,
    pub rejected_images: u32,
    pub import_batches: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityContext {
    pub session: Option<Session>,
    pub library: ActivityLibrarySummary,
    pub recent_events: Vec<SessionEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRecord {
    pub seq: i64,
    pub id: String,
    pub action_type: String,
    pub label: String,
    pub before_json: String,
    pub after_json: String,
    pub affected_image_ids: Option<String>,
    pub group_id: Option<String>,
    pub has_file_backup: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub folder_path: String,
    pub settings_json: Option<String>,
    pub created_at: String,
    pub image_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub canvas_type: String,
    pub layout_json: String,
    pub filter_json: Option<String>,
    pub grid_config_json: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}
