# Privacy Dashboard — Design Spec

*2026-05-12*

## Overview

A Privacy & Data panel in Settings that shows users exactly what data leaves their machine, where it goes, and a history of actual API calls made. Builds trust and satisfies GDPR transparency requirements.

## Backend

### New SQLite table: `api_audit_log`

```sql
CREATE TABLE IF NOT EXISTS api_audit_log (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    provider TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    data_type TEXT NOT NULL,
    data_size_bytes INTEGER,
    prompt_preview TEXT,
    image_dimensions TEXT,
    model TEXT,
    response_status INTEGER,
    jurisdiction TEXT
);
```

Migration added to `Database::run_migrations()` chain.

### Logging function

New function in `services/audit.rs`:

```rust
pub fn log_api_call(db: &Database, provider: &str, endpoint: &str, data_type: &str,
    data_size_bytes: i64, prompt_preview: Option<&str>, image_dimensions: Option<&str>,
    model: Option<&str>, response_status: i32, jurisdiction: &str) -> Result<()>
```

Generates UUID id, captures UTC timestamp, inserts row.

### Logging points

Insert `log_api_call` at three sites:

1. **`db_core/gemini.rs`** — in `generate_embedding()`, after the HTTP POST to `generativelanguage.googleapis.com`. Provider: `"gemini"`, data_type: `"image"`, jurisdiction: `"US - Google LLC"`.

2. **`services/generation.rs`** — in `generate_images()`, after each generation API call. Provider from request, data_type: `"prompt"` or `"prompt+image"`, jurisdiction looked up from provider config.

3. **`db_core/vision.rs`** — in `analyze_with_ollama()`, after POST. Provider: `"ollama"`, jurisdiction: `"Local"` or derived from URL.

### New Tauri commands

In `commands/privacy.rs`:

```rust
#[tauri::command]
pub async fn get_api_audit_log(state: State<'_, AppState>, limit: u32) -> Result<Vec<AuditLogEntry>, String>

#[tauri::command]
pub async fn get_data_flow_status(state: State<'_, AppState>) -> Result<Vec<DataFlowEntry>, String>

#[tauri::command]
pub async fn export_audit_log(state: State<'_, AppState>) -> Result<String, String>
```

`get_data_flow_status` checks:
- `is_model_available()` for CLIP
- `has_api_key("google")` for Gemini
- `has_api_key("openai")` for OpenAI
- `has_api_key("openrouter")` for OpenRouter
- `get_setting("ollama_url")` for Ollama
- MCP socket/HTTP status from settings

Returns a vec of `DataFlowEntry { feature, status, server, data_sent }`.

`export_audit_log` returns full log as JSON string.

### Response types

```rust
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: String,
    pub provider: String,
    pub endpoint: String,
    pub data_type: String,
    pub data_size_bytes: Option<i64>,
    pub prompt_preview: Option<String>,
    pub image_dimensions: Option<String>,
    pub model: Option<String>,
    pub response_status: Option<i32>,
    pub jurisdiction: String,
}

pub struct DataFlowEntry {
    pub feature: String,
    pub status: String,       // "local", "active", "configured", "off"
    pub server: String,
    pub data_sent: String,
}
```

## Frontend

### New component: `PrivacyDashboard.svelte`

Located at `src/lib/components/PrivacyDashboard.svelte`.

#### Section 1: Data Flow Status

Table with columns: Feature, Status (colored dot), Server, Data Sent.

Status dot colors:
- Green `#9ece6a` — "local" (running locally, no data leaves)
- Red `#f7768e` — "active" (cloud API active, data is being sent)
- Yellow `#e0af68` — "configured" (key set but no recent calls)
- Gray `#565a7a` — "off" (no key, not configured)

Static rows (8 features). Data fetched from `get_data_flow_status` on mount.

Inline warning below Gemini row if Gemini key is configured: "Free tier: Google may use your images for model training. Use paid tier for privacy."

#### Section 2: API Call History

Collapsible section, default expanded. Fetches `get_api_audit_log(20)` on mount.

Each entry shows: `timestamp | provider | data_size | status_code`

Click to expand, showing:
- Endpoint URL
- Data sent description (e.g., "1 image (1.2 MB, 2048x1536 JPEG)")
- Model name
- Prompt preview (if applicable, first 200 chars)
- Jurisdiction line (e.g., "US - Google LLC, Mountain View CA")
- ToS link (hardcoded per provider)
- Provider compliance badge: "SOC 2 Type II" / "ISO 27001" / etc.

Provider compliance data is a hardcoded map in the component:

```typescript
const PROVIDER_COMPLIANCE = {
    gemini: {
        company: 'Google LLC',
        jurisdiction: 'US - Mountain View, CA',
        certifications: ['SOC 1/2/3', 'ISO 27001', 'ISO 27017', 'ISO 27018'],
        gdpr: 'DPA available via Google Cloud terms',
        training: 'Paid tier: No. Free tier: Yes',
        tos: 'https://ai.google.dev/gemini-api/terms',
        retention: '≤30 days for debugging',
    },
    openai: {
        company: 'OpenAI, Inc.',
        jurisdiction: 'US - San Francisco, CA',
        certifications: ['SOC 2 Type II'],
        gdpr: 'DPA available. EU data residency option.',
        training: 'No (since March 2023)',
        tos: 'https://openai.com/policies/terms-of-use/',
        retention: '30 days. ZDR available.',
    },
    openrouter: {
        company: 'OpenRouter',
        jurisdiction: 'US',
        certifications: ['SOC 2 (Enterprise only)'],
        gdpr: 'Claims compliance. No public DPA.',
        training: 'Proxy — depends on downstream provider',
        tos: 'https://openrouter.ai/terms',
        retention: 'ZDR routing available',
    },
    ollama: {
        company: 'Local inference',
        jurisdiction: 'Your machine',
        certifications: [],
        gdpr: 'N/A — fully local',
        training: 'No',
        tos: 'https://ollama.com/privacy',
        retention: 'Local only',
    },
};
```

#### Section 3: Footer

Two collapsible sections:
- **"What could leave this machine"** — generated from data flow status. Lists each configured provider with what data types could be sent.
- **"Export audit log"** — button that calls `export_audit_log` and triggers a JSON file download.

### Integration

Add PrivacyDashboard as a new tab/section in the existing settings area. Accessible via sidebar or tab labeled "Privacy & Data".

### API layer additions

In `src/lib/api.ts`:

```typescript
export async function getApiAuditLog(limit: number): Promise<AuditLogEntry[]>
export async function getDataFlowStatus(): Promise<DataFlowEntry[]>
export async function exportAuditLog(): Promise<string>
```

## Files to create

- `src-tauri/src/services/audit.rs` — logging function + DB queries
- `src-tauri/src/commands/privacy.rs` — Tauri commands
- `src/lib/components/PrivacyDashboard.svelte` — UI component

## Files to modify

- `src-tauri/src/db_core/db.rs` — add migration
- `src-tauri/src/db_core/gemini.rs` — add audit log call
- `src-tauri/src/services/generation.rs` — add audit log call
- `src-tauri/src/db_core/vision.rs` — add audit log call
- `src-tauri/src/services/mod.rs` — add `pub mod audit;`
- `src-tauri/src/commands/mod.rs` — add `pub mod privacy;`
- `src-tauri/src/lib.rs` — register new commands
- `src/lib/api.ts` — add new API functions
- Settings UI file — add PrivacyDashboard section

## Out of scope

- PII detection / face detection before upload
- Pre-upload consent dialog
- Audit log deletion / retention limits
- Real-time provider compliance fetching
- Gemini free vs paid tier detection
