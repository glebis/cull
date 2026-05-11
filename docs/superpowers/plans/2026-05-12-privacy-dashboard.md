# Privacy Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Privacy & Data panel showing data flow status, API call audit log with provider compliance details, and export functionality.

**Architecture:** New `api_audit_log` SQLite table logged at each outbound API call site. New `services/audit.rs` for DB operations, `commands/privacy.rs` for Tauri commands, `PrivacyDashboard.svelte` for UI rendered as a tab in the existing McpSettings modal.

**Tech Stack:** Rust (rusqlite, serde, uuid, chrono), SvelteKit 5 (runes), Tauri 2 IPC

---

### Task 1: Audit log table + service module

**Files:**
- Create: `src-tauri/src/services/audit.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/db_core/db.rs`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/services/audit.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;

    #[test]
    fn test_log_and_retrieve_api_call() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        log_api_call(&db, "gemini", "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-exp-03-07:embedContent",
            "image", 1200000, None, Some("2048x1536"), Some("gemini-embedding-exp-03-07"), 200, "US - Google LLC").unwrap();

        log_api_call(&db, "openai", "https://api.openai.com/v1/images/generations",
            "prompt", 450, Some("a cat in watercolor style"), None, Some("gpt-image-2"), 200, "US - OpenAI Inc").unwrap();

        let entries = get_audit_log(&db, 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].provider, "openai");
        assert_eq!(entries[1].provider, "gemini");
        assert_eq!(entries[0].prompt_preview.as_deref(), Some("a cat in watercolor style"));
        assert_eq!(entries[1].image_dimensions.as_deref(), Some("2048x1536"));
    }

    #[test]
    fn test_audit_log_limit() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        for i in 0..5 {
            log_api_call(&db, "gemini", "https://example.com", "image", i * 100,
                None, None, None, 200, "US").unwrap();
        }

        let entries = get_audit_log(&db, 3).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_export_audit_log_json() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        log_api_call(&db, "openai", "https://api.openai.com/v1/images/generations",
            "prompt", 100, Some("test prompt"), None, Some("gpt-image-2"), 200, "US - OpenAI Inc").unwrap();

        let json = export_audit_log_json(&db).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["provider"], "openai");
    }
}
```

- [ ] **Step 2: Add migration to db.rs**

In `src-tauri/src/db_core/db.rs`, add a new migration method and call it from `run_migrations()`:

```rust
// Add to run_migrations() chain, after migrate_raw_metadata:
self.migrate_audit_log()?;

// New method:
fn migrate_audit_log(&self) -> Result<()> {
    let conn = self.conn.lock();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS api_audit_log (
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
        );"
    )?;
    Ok(())
}
```

- [ ] **Step 3: Implement audit service**

Create `src-tauri/src/services/audit.rs`:

```rust
// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use rusqlite::{params, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
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

pub fn log_api_call(
    db: &Database,
    provider: &str,
    endpoint: &str,
    data_type: &str,
    data_size_bytes: i64,
    prompt_preview: Option<&str>,
    image_dimensions: Option<&str>,
    model: Option<&str>,
    response_status: i32,
    jurisdiction: &str,
) -> Result<()> {
    let conn = db.conn.lock();
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let preview = prompt_preview.map(|p| if p.len() > 200 { &p[..200] } else { p });
    conn.execute(
        "INSERT INTO api_audit_log (id, timestamp, provider, endpoint, data_type, data_size_bytes, prompt_preview, image_dimensions, model, response_status, jurisdiction)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![id, timestamp, provider, endpoint, data_type, data_size_bytes, preview, image_dimensions, model, response_status, jurisdiction],
    )?;
    Ok(())
}

pub fn get_audit_log(db: &Database, limit: u32) -> Result<Vec<AuditLogEntry>> {
    let conn = db.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, provider, endpoint, data_type, data_size_bytes, prompt_preview, image_dimensions, model, response_status, jurisdiction
         FROM api_audit_log ORDER BY timestamp DESC LIMIT ?1"
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(AuditLogEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            provider: row.get(2)?,
            endpoint: row.get(3)?,
            data_type: row.get(4)?,
            data_size_bytes: row.get(5)?,
            prompt_preview: row.get(6)?,
            image_dimensions: row.get(7)?,
            model: row.get(8)?,
            response_status: row.get(9)?,
            jurisdiction: row.get(10)?,
        })
    })?;
    rows.collect()
}

pub fn export_audit_log_json(db: &Database) -> Result<String> {
    let entries = get_audit_log(db, 10000)?;
    Ok(serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string()))
}
```

- [ ] **Step 4: Register module**

In `src-tauri/src/services/mod.rs`, add after the `pub mod sessions;` line:

```rust
pub mod audit;
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test audit -- --nocapture`
Expected: 3 tests pass.

- [ ] **Step 6: Commit**

```
git add src-tauri/src/services/audit.rs src-tauri/src/services/mod.rs src-tauri/src/db_core/db.rs
git commit -m "feat(privacy): audit log table and service module"
```

---

### Task 2: Tauri commands for privacy dashboard

**Files:**
- Create: `src-tauri/src/commands/privacy.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create privacy commands**

Create `src-tauri/src/commands/privacy.rs`:

```rust
// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use tauri::State;
use serde::Serialize;
use crate::AppState;
use crate::services::audit;

#[derive(Debug, Serialize)]
pub struct DataFlowEntry {
    pub feature: String,
    pub status: String,
    pub server: String,
    pub data_sent: String,
}

#[tauri::command]
pub async fn get_data_flow_status(state: State<'_, AppState>) -> Result<Vec<DataFlowEntry>, String> {
    let mut entries = Vec::new();

    let clip_available = {
        let engine = state.embedding_engine.lock();
        engine.is_model_available()
    };
    entries.push(DataFlowEntry {
        feature: "CLIP embeddings".into(),
        status: if clip_available { "local" } else { "off" }.into(),
        server: "—".into(),
        data_sent: "Nothing".into(),
    });

    entries.push(DataFlowEntry {
        feature: "Object detection".into(),
        status: "local".into(),
        server: "—".into(),
        data_sent: "Nothing".into(),
    });

    let ollama_url = state.db.get_setting("ollama_url")
        .ok().flatten()
        .unwrap_or_else(|| "http://localhost:11434/api/generate".to_string());
    let ollama_is_local = ollama_url.contains("localhost") || ollama_url.contains("127.0.0.1");
    entries.push(DataFlowEntry {
        feature: "Ollama vision".into(),
        status: if ollama_is_local { "local" } else { "active" }.into(),
        server: if ollama_is_local { "localhost".into() } else { "remote".into() },
        data_sent: "Images".into(),
    });

    let gemini_key = state.db.get_setting("api_key_exists_google")
        .ok().flatten().map(|v| v == "true").unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "Gemini embeddings".into(),
        status: if gemini_key { "active" } else { "off" }.into(),
        server: if gemini_key { "US 🇺🇸".into() } else { "—".into() },
        data_sent: if gemini_key { "Images + API key".into() } else { "Nothing".into() },
    });

    let openai_key = state.db.get_setting("api_key_exists_openai")
        .ok().flatten().map(|v| v == "true").unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "OpenAI generation".into(),
        status: if openai_key { "configured" } else { "off" }.into(),
        server: if openai_key { "US 🇺🇸".into() } else { "—".into() },
        data_sent: if openai_key { "Prompts + images".into() } else { "Nothing".into() },
    });

    let openrouter_key = state.db.get_setting("api_key_exists_openrouter")
        .ok().flatten().map(|v| v == "true").unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "OpenRouter generation".into(),
        status: if openrouter_key { "configured" } else { "off" }.into(),
        server: if openrouter_key { "US 🇺🇸".into() } else { "—".into() },
        data_sent: if openrouter_key { "Prompts + images".into() } else { "Nothing".into() },
    });

    let mcp_http = state.db.get_setting("mcp_http_enabled")
        .ok().flatten().map(|v| v == "true").unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "MCP (local)".into(),
        status: "local".into(),
        server: "localhost".into(),
        data_sent: "Metadata".into(),
    });
    entries.push(DataFlowEntry {
        feature: "MCP (HTTP)".into(),
        status: if mcp_http { "active" } else { "off" }.into(),
        server: if mcp_http { "network".into() } else { "—".into() },
        data_sent: "Metadata".into(),
    });

    Ok(entries)
}

#[tauri::command]
pub async fn get_api_audit_log(state: State<'_, AppState>, limit: u32) -> Result<Vec<audit::AuditLogEntry>, String> {
    let clamped = limit.min(100).max(1);
    audit::get_audit_log(&state.db, clamped).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_audit_log(state: State<'_, AppState>) -> Result<String, String> {
    audit::export_audit_log_json(&state.db).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register module in commands/mod.rs**

Add after `pub mod sessions;`:

```rust
pub mod privacy;
```

- [ ] **Step 3: Register commands in lib.rs**

Find the `.invoke_handler(tauri::generate_handler![...])` block in `src-tauri/src/lib.rs` and add these three commands:

```rust
commands::privacy::get_data_flow_status,
commands::privacy::get_api_audit_log,
commands::privacy::export_audit_log,
```

- [ ] **Step 4: Build check**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors.

- [ ] **Step 5: Commit**

```
git add src-tauri/src/commands/privacy.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(privacy): Tauri commands for data flow status and audit log"
```

---

### Task 3: Instrument API call sites with audit logging

**Files:**
- Modify: `src-tauri/src/db_core/gemini.rs`
- Modify: `src-tauri/src/services/generation.rs`
- Modify: `src-tauri/src/db_core/vision.rs`

- [ ] **Step 1: Instrument Gemini embeddings**

The `GeminiEmbeddingProvider::generate_embedding` method doesn't have access to `Database`. We need to change the approach — return the response status alongside the embedding, and let the caller log.

Instead, add a standalone logging function that callers invoke. In `src-tauri/src/commands/embeddings.rs`, find the `generate_gemini_embeddings` command where `provider.generate_embedding()` is called. After the call, add audit logging.

Find the loop in `generate_gemini_embeddings` where embeddings are generated. After the successful embedding generation, add:

```rust
let _ = crate::services::audit::log_api_call(
    &state.db,
    "gemini",
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-exp-03-07:embedContent",
    "image",
    std::fs::metadata(&resolve_path).map(|m| m.len() as i64).unwrap_or(0),
    None,
    Some(&format!("{}x{}", img.image.width, img.image.height)),
    Some("gemini-embedding-exp-03-07"),
    200,
    "US - Google LLC",
);
```

For error cases, log with the appropriate status code (e.g., 500).

- [ ] **Step 2: Instrument generation (OpenAI/OpenRouter/Google)**

In `src-tauri/src/services/generation.rs`, in the `generate_images` function:

After the OpenAI-style response is received (around line 222, after `resp.status().is_success()` check), add:

```rust
let jurisdiction = match request.provider.as_str() {
    "openai" => "US - OpenAI Inc",
    "openrouter" => "US - OpenRouter (proxy)",
    "google" => "US - Google LLC",
    _ => "Unknown",
};
let prompt_preview = Some(request.prompt.as_str());
let data_type = if request.source_image_id.is_some() { "prompt+image" } else { "prompt" };
let endpoint = format!("{}/images/generations", base_url);
let _ = crate::services::audit::log_api_call(
    db,
    &request.provider,
    &endpoint,
    data_type,
    request.prompt.len() as i64,
    prompt_preview,
    None,
    Some(&request.model),
    resp.status().as_u16() as i32,
    jurisdiction,
);
```

Note: You need to capture `resp.status()` before consuming the response body. Save it: `let status = resp.status();` before `resp.text().await`.

Do the same for the Gemini-style branch (around line 290+).

- [ ] **Step 3: Instrument Ollama vision**

In `src-tauri/src/db_core/vision.rs`, in `analyze_image`, after the response is received:

```rust
let jurisdiction = if ollama_url.contains("localhost") || ollama_url.contains("127.0.0.1") {
    "Local"
} else {
    "Remote - user configured"
};
// Note: This function doesn't have Database access. Log from the caller instead.
```

Since `analyze_image` doesn't have Database access, instrument the caller in `src-tauri/src/commands/vision.rs` where `analyze_image` is called. After the call returns:

```rust
let _ = crate::services::audit::log_api_call(
    &state.db,
    "ollama",
    &url,
    "image",
    std::fs::metadata(&image_path).map(|m| m.len() as i64).unwrap_or(0),
    None,
    None,
    Some(&model),
    200,
    if url.contains("localhost") || url.contains("127.0.0.1") { "Local" } else { "Remote" },
);
```

- [ ] **Step 4: Build and test**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors.

Run: `cd src-tauri && cargo test audit`
Expected: existing tests still pass.

- [ ] **Step 5: Commit**

```
git add src-tauri/src/commands/embeddings.rs src-tauri/src/services/generation.rs src-tauri/src/commands/vision.rs
git commit -m "feat(privacy): instrument API call sites with audit logging"
```

---

### Task 4: Frontend API functions

**Files:**
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add TypeScript types and API functions**

At the end of `src/lib/api.ts`, add:

```typescript
// Privacy & audit log
export interface AuditLogEntry {
    id: string;
    timestamp: string;
    provider: string;
    endpoint: string;
    data_type: string;
    data_size_bytes: number | null;
    prompt_preview: string | null;
    image_dimensions: string | null;
    model: string | null;
    response_status: number | null;
    jurisdiction: string;
}

export interface DataFlowEntry {
    feature: string;
    status: string;
    server: string;
    data_sent: string;
}

export async function getDataFlowStatus(): Promise<DataFlowEntry[]> {
    return invoke('get_data_flow_status');
}

export async function getApiAuditLog(limit: number): Promise<AuditLogEntry[]> {
    return invoke('get_api_audit_log', { limit });
}

export async function exportAuditLog(): Promise<string> {
    return invoke('export_audit_log');
}
```

- [ ] **Step 2: Commit**

```
git add src/lib/api.ts
git commit -m "feat(privacy): frontend API types and functions for audit log"
```

---

### Task 5: PrivacyDashboard Svelte component

**Files:**
- Create: `src/lib/components/PrivacyDashboard.svelte`

- [ ] **Step 1: Create the component**

Create `src/lib/components/PrivacyDashboard.svelte`:

```svelte
<!-- Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author. -->
<!-- Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md. -->
<script lang="ts">
    import { onMount } from 'svelte';
    import { getDataFlowStatus, getApiAuditLog, exportAuditLog } from '$lib/api';
    import type { DataFlowEntry, AuditLogEntry } from '$lib/api';

    let flowStatus = $state<DataFlowEntry[]>([]);
    let auditLog = $state<AuditLogEntry[]>([]);
    let expandedEntry = $state<string | null>(null);
    let historyOpen = $state(true);
    let potentialOpen = $state(false);
    let loading = $state(true);

    const STATUS_COLORS: Record<string, string> = {
        local: 'var(--green)',
        active: 'var(--red)',
        configured: 'var(--orange)',
        off: 'var(--text-secondary)',
    };

    const PROVIDER_COMPLIANCE: Record<string, {
        company: string;
        certifications: string[];
        gdpr: string;
        training: string;
        tos: string;
        retention: string;
    }> = {
        gemini: {
            company: 'Google LLC, Mountain View CA',
            certifications: ['SOC 1/2/3', 'ISO 27001', 'ISO 27017', 'ISO 27018'],
            gdpr: 'DPA available via Google Cloud terms',
            training: 'Paid tier: No. Free tier: Yes — images used for training',
            tos: 'https://ai.google.dev/gemini-api/terms',
            retention: '≤30 days for debugging',
        },
        openai: {
            company: 'OpenAI Inc, San Francisco CA',
            certifications: ['SOC 2 Type II'],
            gdpr: 'DPA available. EU data residency option.',
            training: 'No (since March 2023)',
            tos: 'https://openai.com/policies/terms-of-use/',
            retention: '30 days. Zero Data Retention available.',
        },
        openrouter: {
            company: 'OpenRouter (US)',
            certifications: ['SOC 2 (Enterprise only)'],
            gdpr: 'Claims compliance. No public DPA.',
            training: 'Proxy — depends on downstream provider',
            tos: 'https://openrouter.ai/terms',
            retention: 'ZDR routing available',
        },
        ollama: {
            company: 'Local inference (your machine)',
            certifications: [],
            gdpr: 'N/A — fully local',
            training: 'No data collection',
            tos: 'https://ollama.com/privacy',
            retention: 'Local only',
        },
    };

    onMount(async () => {
        try {
            const [status, log] = await Promise.all([
                getDataFlowStatus(),
                getApiAuditLog(20),
            ]);
            flowStatus = status;
            auditLog = log;
        } catch (e) {
            console.error('Privacy dashboard load error:', e);
        }
        loading = false;
    });

    function formatBytes(bytes: number | null): string {
        if (bytes == null) return '—';
        if (bytes < 1024) return `${bytes} B`;
        if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
        return `${(bytes / 1048576).toFixed(1)} MB`;
    }

    function formatTime(ts: string): string {
        const d = new Date(ts);
        return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }

    function formatDate(ts: string): string {
        const d = new Date(ts);
        return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
    }

    async function handleExport() {
        try {
            const json = await exportAuditLog();
            const blob = new Blob([json], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `imageview-audit-log-${new Date().toISOString().slice(0, 10)}.json`;
            a.click();
            URL.revokeObjectURL(url);
        } catch (e) {
            console.error('Export error:', e);
        }
    }
</script>

<div class="privacy-dashboard">
    <h3 class="section-header">Privacy & Data</h3>

    {#if loading}
        <p class="loading">Loading...</p>
    {:else}
        <div class="flow-section">
            <h4 class="subsection-header">Data Flow Status</h4>
            <table class="flow-table">
                <thead>
                    <tr>
                        <th>Feature</th>
                        <th>Status</th>
                        <th>Server</th>
                        <th>Data Sent</th>
                    </tr>
                </thead>
                <tbody>
                    {#each flowStatus as entry}
                        <tr>
                            <td>{entry.feature}</td>
                            <td>
                                <span class="status-dot" style="background: {STATUS_COLORS[entry.status] || 'var(--text-secondary)'}"></span>
                                <span class="status-label">{entry.status}</span>
                            </td>
                            <td class="server-cell">{entry.server}</td>
                            <td class="data-cell">{entry.data_sent}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>

            {#if flowStatus.some(e => e.feature === 'Gemini embeddings' && e.status === 'active')}
                <div class="gemini-warning">
                    ⚠ Gemini free tier: Google may use your images for model training. Use paid tier for privacy.
                </div>
            {/if}
        </div>

        <div class="history-section">
            <button class="collapse-toggle" onclick={() => historyOpen = !historyOpen}>
                <span class="arrow">{historyOpen ? '▼' : '▶'}</span>
                API Call History ({auditLog.length})
            </button>

            {#if historyOpen}
                <div class="audit-list">
                    {#if auditLog.length === 0}
                        <p class="empty-state">No API calls recorded yet.</p>
                    {:else}
                        {#each auditLog as entry}
                            <div class="audit-entry" class:expanded={expandedEntry === entry.id}>
                                <button class="audit-row" onclick={() => expandedEntry = expandedEntry === entry.id ? null : entry.id}>
                                    <span class="audit-time">{formatDate(entry.timestamp)} {formatTime(entry.timestamp)}</span>
                                    <span class="audit-provider">{entry.provider}</span>
                                    <span class="audit-size">{formatBytes(entry.data_size_bytes)}</span>
                                    <span class="audit-status" class:success={entry.response_status === 200}>
                                        {entry.response_status === 200 ? '✓' : entry.response_status ?? '?'}
                                    </span>
                                    <span class="arrow">{expandedEntry === entry.id ? '▼' : '▶'}</span>
                                </button>

                                {#if expandedEntry === entry.id}
                                    <div class="audit-details">
                                        <div class="detail-row">
                                            <span class="detail-label">Endpoint</span>
                                            <span class="detail-value endpoint">{entry.endpoint}</span>
                                        </div>
                                        <div class="detail-row">
                                            <span class="detail-label">Data sent</span>
                                            <span class="detail-value">
                                                {entry.data_type === 'image' ? `1 image (${formatBytes(entry.data_size_bytes)}${entry.image_dimensions ? `, ${entry.image_dimensions}` : ''})` : ''}
                                                {entry.data_type === 'prompt' ? `Prompt (${formatBytes(entry.data_size_bytes)})` : ''}
                                                {entry.data_type === 'prompt+image' ? `Prompt + source image` : ''}
                                            </span>
                                        </div>
                                        {#if entry.prompt_preview}
                                            <div class="detail-row">
                                                <span class="detail-label">Prompt</span>
                                                <span class="detail-value prompt-preview">"{entry.prompt_preview}"</span>
                                            </div>
                                        {/if}
                                        {#if entry.model}
                                            <div class="detail-row">
                                                <span class="detail-label">Model</span>
                                                <span class="detail-value">{entry.model}</span>
                                            </div>
                                        {/if}
                                        <div class="detail-row">
                                            <span class="detail-label">Jurisdiction</span>
                                            <span class="detail-value">{entry.jurisdiction}</span>
                                        </div>
                                        {#if PROVIDER_COMPLIANCE[entry.provider]}
                                            {@const comp = PROVIDER_COMPLIANCE[entry.provider]}
                                            <div class="detail-row">
                                                <span class="detail-label">Company</span>
                                                <span class="detail-value">{comp.company}</span>
                                            </div>
                                            {#if comp.certifications.length > 0}
                                                <div class="detail-row">
                                                    <span class="detail-label">Certifications</span>
                                                    <span class="detail-value certs">{comp.certifications.join(', ')}</span>
                                                </div>
                                            {/if}
                                            <div class="detail-row">
                                                <span class="detail-label">GDPR</span>
                                                <span class="detail-value">{comp.gdpr}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span class="detail-label">Training</span>
                                                <span class="detail-value" class:training-warning={comp.training.includes('Yes')}>{comp.training}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span class="detail-label">ToS</span>
                                                <a class="detail-value tos-link" href={comp.tos} target="_blank">{comp.tos}</a>
                                            </div>
                                        {/if}
                                    </div>
                                {/if}
                            </div>
                        {/each}
                    {/if}
                </div>
            {/if}
        </div>

        <div class="footer-sections">
            <button class="collapse-toggle" onclick={() => potentialOpen = !potentialOpen}>
                <span class="arrow">{potentialOpen ? '▼' : '▶'}</span>
                What could leave this machine
            </button>
            {#if potentialOpen}
                <div class="potential-list">
                    {#each flowStatus.filter(e => e.status !== 'local' && e.status !== 'off') as entry}
                        <div class="potential-item">
                            <strong>{entry.feature}</strong>: {entry.data_sent} → {entry.server}
                        </div>
                    {/each}
                    {#if flowStatus.filter(e => e.status !== 'local' && e.status !== 'off').length === 0}
                        <p class="safe-state">No cloud APIs configured. All data stays on your machine.</p>
                    {/if}
                </div>
            {/if}

            <button class="collapse-toggle" onclick={handleExport}>
                <span class="arrow">↓</span>
                Export audit log (JSON)
            </button>
        </div>
    {/if}
</div>

<style>
    .privacy-dashboard {
        padding: 0;
    }

    .section-header {
        font-size: 14px;
        font-weight: 600;
        margin-bottom: 16px;
        text-transform: uppercase;
        letter-spacing: 0.5px;
    }

    .subsection-header {
        font-size: 12px;
        color: var(--text-secondary);
        margin-bottom: 8px;
        text-transform: uppercase;
        letter-spacing: 0.5px;
    }

    .flow-table {
        width: 100%;
        border-collapse: collapse;
        font-size: 12px;
        margin-bottom: 12px;
    }

    .flow-table th {
        text-align: left;
        color: var(--text-secondary);
        font-weight: 500;
        padding: 6px 8px;
        border-bottom: 1px solid var(--border);
    }

    .flow-table td {
        padding: 6px 8px;
        border-bottom: 1px solid var(--border);
    }

    .status-dot {
        display: inline-block;
        width: 8px;
        height: 8px;
        border-radius: 50%;
        margin-right: 6px;
        vertical-align: middle;
    }

    .status-label {
        text-transform: capitalize;
        font-size: 11px;
    }

    .server-cell, .data-cell {
        color: var(--text-secondary);
    }

    .gemini-warning {
        background: rgba(247, 118, 142, 0.1);
        border: 1px solid rgba(247, 118, 142, 0.3);
        border-radius: var(--radius);
        padding: 8px 12px;
        font-size: 11px;
        color: var(--red);
        margin-bottom: 16px;
    }

    .collapse-toggle {
        display: flex;
        align-items: center;
        gap: 8px;
        width: 100%;
        background: none;
        border: none;
        border-bottom: 1px solid var(--border);
        color: var(--text);
        font-family: var(--font);
        font-size: 12px;
        padding: 10px 0;
        cursor: pointer;
        text-align: left;
    }

    .collapse-toggle:hover {
        color: var(--blue);
    }

    .arrow {
        font-size: 10px;
        color: var(--text-secondary);
        width: 12px;
    }

    .audit-list {
        margin-bottom: 8px;
    }

    .empty-state, .safe-state {
        color: var(--text-secondary);
        font-size: 12px;
        padding: 12px 0;
    }

    .safe-state {
        color: var(--green);
    }

    .audit-entry {
        border-bottom: 1px solid var(--border);
    }

    .audit-row {
        display: flex;
        align-items: center;
        gap: 12px;
        width: 100%;
        background: none;
        border: none;
        color: var(--text);
        font-family: var(--font);
        font-size: 12px;
        padding: 8px 0;
        cursor: pointer;
        text-align: left;
    }

    .audit-row:hover {
        background: var(--surface);
    }

    .audit-time {
        color: var(--text-secondary);
        min-width: 100px;
        font-size: 11px;
    }

    .audit-provider {
        min-width: 80px;
        font-weight: 500;
    }

    .audit-size {
        color: var(--text-secondary);
        min-width: 70px;
        font-size: 11px;
    }

    .audit-status {
        font-size: 11px;
        color: var(--text-secondary);
    }

    .audit-status.success {
        color: var(--green);
    }

    .audit-details {
        padding: 8px 0 12px 24px;
        background: var(--surface);
        border-radius: var(--radius);
        margin-bottom: 4px;
    }

    .detail-row {
        display: flex;
        gap: 12px;
        padding: 3px 8px;
        font-size: 11px;
    }

    .detail-label {
        color: var(--text-secondary);
        min-width: 90px;
        flex-shrink: 0;
    }

    .detail-value {
        color: var(--text);
        word-break: break-all;
    }

    .endpoint {
        font-size: 10px;
        color: var(--text-secondary);
    }

    .prompt-preview {
        color: var(--orange);
        font-style: italic;
    }

    .certs {
        color: var(--green);
    }

    .training-warning {
        color: var(--red);
    }

    .tos-link {
        color: var(--blue);
        text-decoration: none;
        font-size: 10px;
    }

    .tos-link:hover {
        text-decoration: underline;
    }

    .potential-list {
        padding: 8px 0;
    }

    .potential-item {
        font-size: 12px;
        padding: 4px 0;
        color: var(--text-secondary);
    }

    .potential-item strong {
        color: var(--text);
    }

    .loading {
        color: var(--text-secondary);
        font-size: 12px;
    }

    .footer-sections {
        margin-top: 8px;
    }
</style>
```

- [ ] **Step 2: Commit**

```
git add src/lib/components/PrivacyDashboard.svelte
git commit -m "feat(privacy): PrivacyDashboard component with data flow and audit log"
```

---

### Task 6: Integrate into McpSettings

**Files:**
- Modify: `src/lib/components/McpSettings.svelte`

- [ ] **Step 1: Add Privacy tab to McpSettings**

In `McpSettings.svelte`, add the import at the top of the `<script>`:

```typescript
import PrivacyDashboard from './PrivacyDashboard.svelte';
```

Add a tab state variable near the existing state declarations:

```typescript
let activeSettingsTab = $state<'general' | 'privacy'>('general');
```

Wrap the existing settings content in a tab structure. Before the existing settings content (after the header/title area), add tab buttons:

```svelte
<div class="settings-tabs">
    <button class="settings-tab" class:active={activeSettingsTab === 'general'} onclick={() => activeSettingsTab = 'general'}>General</button>
    <button class="settings-tab" class:active={activeSettingsTab === 'privacy'} onclick={() => activeSettingsTab = 'privacy'}>Privacy & Data</button>
</div>
```

Wrap existing content in `{#if activeSettingsTab === 'general'}...{/if}` and add the privacy tab:

```svelte
{#if activeSettingsTab === 'privacy'}
    <PrivacyDashboard />
{/if}
```

Add tab styles:

```css
.settings-tabs {
    display: flex;
    gap: 4px;
    margin-bottom: 16px;
    border-bottom: 1px solid var(--border);
    padding-bottom: 8px;
}

.settings-tab {
    padding: 6px 12px;
    background: none;
    border: none;
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-family: var(--font);
    font-size: 12px;
    cursor: pointer;
}

.settings-tab:hover {
    color: var(--text);
}

.settings-tab.active {
    background: var(--surface);
    color: var(--text);
}
```

- [ ] **Step 2: Build and test in browser**

Run: `npx vite dev --port 1420`
Open settings, verify both tabs work. Check Privacy tab shows data flow table.

- [ ] **Step 3: Commit**

```
git add src/lib/components/McpSettings.svelte
git commit -m "feat(privacy): integrate PrivacyDashboard into Settings tabs"
```
