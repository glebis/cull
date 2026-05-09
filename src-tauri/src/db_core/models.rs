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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFile {
    pub id: String,
    pub image_id: String,
    pub path: String,
    pub last_seen_at: String,
    pub missing_at: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
