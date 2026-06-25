# Claude Agent Proposal Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first safe, testable slice of the Claude Agent workflow: persisted action proposals, per-file Trash results, a right-side proposal dock, and a native review gate.

**Architecture:** This slice intentionally stops before the real Claude Agent SDK runtime. It creates the substrate that Claude will later drive: proposal persistence in SQLite/Rust, explicit apply commands, usage/context metadata, and Svelte UI that can display and approve proposals. The first proposal source is deterministic/manual so the safety boundary can be tested before connecting a model.

**Tech Stack:** Tauri 2 commands, Rust/rusqlite database layer, Svelte 5 runes/stores, Vitest, Rust unit tests.

## Global Constraints

- Permanent delete is out of scope for v1.
- Claude must not directly perform destructive mutations.
- Applying a proposal is a Cull UI/native command, not a free-form agent tool call.
- `Tiny` is the default visual context level.
- `Preview` shows the estimated incremental cost before loading.
- `Full` opens a confirmation gate unless the user has explicitly allowed full images for the session.
- Token/cost and visual level remain visible but secondary.
- Trash proposal apply uses per-file results and native review; permanent delete remains excluded.
- Existing `trash_images` behavior only returns a count and hides per-file errors. The implementation must add reliable per-file results before exposing Trash to agent proposals.
- Follow Svelte 5 runes patterns: `$state()`, `$props()`, `$derived()`, `onclick`, `onkeydown`.
- All UI styling uses existing Tokyo Night tokens from `src/app.css`; do not hardcode new product colors in component CSS.
- `src/lib/api.ts` must import `invoke` directly from `@tauri-apps/api/core`; do not add or import a mock layer.
- Do not delete, trash, reset, or recreate `~/Library/Application Support/com.glebkalinin.cull/cull.db`.

---

## Scope Split

The approved design covers multiple subsystems. This plan implements only the first vertical slice:

- SQLite proposal persistence.
- Tauri commands and frontend API wrappers for creating/listing/applying/dismissing proposals.
- Detailed per-file Trash results.
- Right-side agent proposal dock and review gate shell.
- Tests for the safety contract.
- Codex-generated UI design artifacts as needed, with Claude Opus review and self-review before committing the final UI implementation.

Follow-up plans should cover:

- Claude Agent SDK streaming runtime.
- Cull-owned Claude plugin/skills bundle.
- Cost ledger reconciliation from real SDK usage events.
- MCP/in-process tool exposure for model-driven proposal creation.

---

## Design Review Gate

Before implementing Task 5 or Task 6, generate or update the interaction prototype with Codex when the UI question is non-trivial. Evaluate the prototype with Claude Opus and perform a written self-review against:

- Thumbnail-first context: `Tiny` is default and the visual level chip is secondary but visible.
- Safety: proposal review is a native Cull gate, not an agent-controlled apply path.
- Layout: pinned mode must not cover the grid; floating mode must not become the only discoverable affordance.
- Cost visibility: token and EUR estimates are present without dominating the curation controls.
- States: empty, pending proposal, escalation required, apply success, apply partial failure, and undo availability.

Record any accepted review findings in the task notes before coding. If Opus and self-review disagree, prefer the stricter safety or clarity requirement.

---

## File Structure

- `src-tauri/src/db_core/agent_proposals.rs`: database methods for proposal CRUD and apply metadata.
- `src-tauri/src/db_core/models.rs`: serializable proposal and Trash result structs shared by commands.
- `src-tauri/src/db_core/db.rs`: schema version bump and migration step for proposal tables.
- `src-tauri/src/db_core/schema.sql`: consolidated schema includes proposal tables for fresh databases.
- `src-tauri/src/services/agent_proposals.rs`: business rules for proposal validation and apply.
- `src-tauri/src/services/mod.rs`: exports the new service module.
- `src-tauri/src/commands/agent_proposals.rs`: Tauri command wrappers.
- `src-tauri/src/commands/library.rs`: detailed Trash command implementation.
- `src-tauri/src/commands/mod.rs`: exports command module.
- `src-tauri/src/lib.rs`: registers new Tauri commands.
- `src/lib/api.ts`: typed API wrappers for proposals and detailed Trash results.
- `src/lib/stores.ts`: panel visibility, visual level, selected proposal state.
- `src/lib/components/AgentProposalDock.svelte`: pinned/floating right-side proposal dock.
- `src/lib/components/ActionProposalReviewDialog.svelte`: native review gate for proposals.
- `src/routes/+page.svelte`: mounts dock/review gate and wires apply refresh behavior.
- `src/lib/components/agent-proposal-dock.test.ts`: frontend behavior tests.
- `src/lib/components/action-proposal-review-dialog.test.ts`: review gate tests.

---

### Task 1: Persist Action Proposals

**Files:**
- Create: `src-tauri/src/db_core/agent_proposals.rs`
- Modify: `src-tauri/src/db_core/models.rs`
- Modify: `src-tauri/src/db_core/mod.rs`
- Modify: `src-tauri/src/db_core/db.rs`
- Modify: `src-tauri/src/db_core/schema.sql`

**Interfaces:**
- Produces: `AgentActionProposal`, `CreateActionProposalDb`, `TrashImageResult`
- Produces: `Database::create_action_proposal(request: CreateActionProposalDb) -> Result<AgentActionProposal>`
- Produces: `Database::get_action_proposal(id: &str) -> Result<Option<AgentActionProposal>>`
- Produces: `Database::list_action_proposals(status: Option<&str>, limit: u32) -> Result<Vec<AgentActionProposal>>`
- Produces: `Database::mark_action_proposal_applied(id: &str, apply_result_json: &str, undo_journal_json: &str) -> Result<()>`
- Produces: `Database::dismiss_action_proposal(id: &str) -> Result<()>`

- [ ] **Step 1: Add failing database tests**

Append tests to a new `#[cfg(test)]` module in `src-tauri/src/db_core/agent_proposals.rs` before implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use tempfile::tempdir;

    fn test_db() -> Database {
        let dir = tempdir().unwrap();
        Database::open(&dir.path().join("test.db")).unwrap()
    }

    fn sample_request() -> CreateActionProposalDb {
        CreateActionProposalDb {
            kind: "trash_images".to_string(),
            persona: "copilot".to_string(),
            lens: Some("near_duplicates".to_string()),
            criteria: "near duplicate, lower focus, unrated".to_string(),
            visual_level: "tiny".to_string(),
            estimated_input_tokens: Some(2100),
            estimated_output_tokens: Some(420),
            estimated_cost_eur: Some(0.014),
            source_context_json: serde_json::json!({"scope":"grid","image_count":24}).to_string(),
            items_json: serde_json::json!([
                {"image_id":"img_a","reason":"lower focus","confidence":0.91}
            ]).to_string(),
            guard_results_json: serde_json::json!({"blocked":[]}).to_string(),
        }
    }

    #[test]
    fn create_get_and_list_action_proposal_round_trips() {
        let db = test_db();
        let created = db.create_action_proposal(sample_request()).unwrap();
        assert_eq!(created.kind, "trash_images");
        assert_eq!(created.status, "pending");
        assert_eq!(created.visual_level, "tiny");
        assert_eq!(created.estimated_input_tokens, Some(2100));

        let loaded = db.get_action_proposal(&created.id).unwrap().unwrap();
        assert_eq!(loaded.id, created.id);
        assert_eq!(loaded.items_json, created.items_json);

        let pending = db.list_action_proposals(Some("pending"), 10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, created.id);
    }

    #[test]
    fn dismiss_and_mark_applied_update_statuses() {
        let db = test_db();
        let first = db.create_action_proposal(sample_request()).unwrap();
        db.dismiss_action_proposal(&first.id).unwrap();
        assert_eq!(
            db.get_action_proposal(&first.id).unwrap().unwrap().status,
            "dismissed"
        );

        let second = db.create_action_proposal(sample_request()).unwrap();
        db.mark_action_proposal_applied(
            &second.id,
            r#"{"applied":1,"failed":0}"#,
            r#"{"steps":[{"kind":"trash","image_id":"img_a"}]}"#,
        )
        .unwrap();
        let applied = db.get_action_proposal(&second.id).unwrap().unwrap();
        assert_eq!(applied.status, "applied");
        assert_eq!(applied.apply_result_json.as_deref(), Some(r#"{"applied":1,"failed":0}"#));
        assert_eq!(applied.undo_journal_json.as_deref(), Some(r#"{"steps":[{"kind":"trash","image_id":"img_a"}]}"#));
        assert!(applied.applied_at.is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test --lib db_core::agent_proposals::tests
```

Expected: compile failure because `agent_proposals` module and proposal structs do not exist.

- [ ] **Step 3: Add model structs**

Append to `src-tauri/src/db_core/models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionProposal {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub persona: String,
    pub lens: Option<String>,
    pub criteria: String,
    pub visual_level: String,
    pub estimated_input_tokens: Option<i64>,
    pub estimated_output_tokens: Option<i64>,
    pub estimated_cost_eur: Option<f64>,
    pub source_context_json: String,
    pub items_json: String,
    pub guard_results_json: String,
    pub apply_result_json: Option<String>,
    pub undo_journal_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub applied_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActionProposalDb {
    pub kind: String,
    pub persona: String,
    pub lens: Option<String>,
    pub criteria: String,
    pub visual_level: String,
    pub estimated_input_tokens: Option<i64>,
    pub estimated_output_tokens: Option<i64>,
    pub estimated_cost_eur: Option<f64>,
    pub source_context_json: String,
    pub items_json: String,
    pub guard_results_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashImageResult {
    pub image_id: String,
    pub path: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashImagesDetailedResult {
    pub requested: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub results: Vec<TrashImageResult>,
}
```

- [ ] **Step 4: Add schema migration**

Modify the constants near the top of `src-tauri/src/db_core/db.rs`:

```rust
const CURRENT_SCHEMA_VERSION: i64 = 2;

const MIGRATIONS: &[(i64, &str)] = &[(1, "initial_schema"), (2, "agent_action_proposals")];
```

Modify `run_migrations()` after the existing migration step:

```rust
self.run_migration_step(2, "agent_action_proposals", || {
    let conn = self.conn.lock();
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS agent_action_proposals (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'applied', 'dismissed')),
            persona TEXT NOT NULL
                CHECK (persona IN ('curator', 'copilot', 'operator')),
            lens TEXT,
            criteria TEXT NOT NULL,
            visual_level TEXT NOT NULL
                CHECK (visual_level IN ('text', 'tiny', 'preview', 'full')),
            estimated_input_tokens INTEGER,
            estimated_output_tokens INTEGER,
            estimated_cost_eur REAL,
            source_context_json TEXT NOT NULL DEFAULT '{}',
            items_json TEXT NOT NULL DEFAULT '[]',
            guard_results_json TEXT NOT NULL DEFAULT '{}',
            apply_result_json TEXT,
            undo_journal_json TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            applied_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_agent_action_proposals_status_created
            ON agent_action_proposals(status, created_at DESC);
        "#
    )?;
    Ok(())
})?;
```

Add the same `CREATE TABLE` and `CREATE INDEX` statements to `src-tauri/src/db_core/schema.sql` so fresh databases include the table during migration 1.

- [ ] **Step 5: Create database module implementation**

Create `src-tauri/src/db_core/agent_proposals.rs`:

```rust
use super::db::Database;
use super::models::{AgentActionProposal, CreateActionProposalDb};
use rusqlite::{params, OptionalExtension, Result};

fn map_agent_action_proposal(row: &rusqlite::Row<'_>) -> Result<AgentActionProposal> {
    Ok(AgentActionProposal {
        id: row.get(0)?,
        kind: row.get(1)?,
        status: row.get(2)?,
        persona: row.get(3)?,
        lens: row.get(4)?,
        criteria: row.get(5)?,
        visual_level: row.get(6)?,
        estimated_input_tokens: row.get(7)?,
        estimated_output_tokens: row.get(8)?,
        estimated_cost_eur: row.get(9)?,
        source_context_json: row.get(10)?,
        items_json: row.get(11)?,
        guard_results_json: row.get(12)?,
        apply_result_json: row.get(13)?,
        undo_journal_json: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
        applied_at: row.get(17)?,
    })
}

impl Database {
    pub fn create_action_proposal(
        &self,
        request: CreateActionProposalDb,
    ) -> Result<AgentActionProposal> {
        let id = format!("proposal_{}", uuid::Uuid::new_v4().simple());
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO agent_action_proposals (
                id, kind, status, persona, lens, criteria, visual_level,
                estimated_input_tokens, estimated_output_tokens, estimated_cost_eur,
                source_context_json, items_json, guard_results_json,
                created_at, updated_at
             ) VALUES (?1, ?2, 'pending', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'), datetime('now'))",
            params![
                id,
                request.kind,
                request.persona,
                request.lens,
                request.criteria,
                request.visual_level,
                request.estimated_input_tokens,
                request.estimated_output_tokens,
                request.estimated_cost_eur,
                request.source_context_json,
                request.items_json,
                request.guard_results_json,
            ],
        )?;
        drop(conn);
        self.get_action_proposal(&id).map(|proposal| {
            proposal.expect("created action proposal should be readable")
        })
    }

    pub fn get_action_proposal(&self, id: &str) -> Result<Option<AgentActionProposal>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, kind, status, persona, lens, criteria, visual_level,
                    estimated_input_tokens, estimated_output_tokens, estimated_cost_eur,
                    source_context_json, items_json, guard_results_json,
                    apply_result_json, undo_journal_json, created_at, updated_at, applied_at
             FROM agent_action_proposals WHERE id = ?1",
            params![id],
            map_agent_action_proposal,
        )
        .optional()
    }

    pub fn list_action_proposals(
        &self,
        status: Option<&str>,
        limit: u32,
    ) -> Result<Vec<AgentActionProposal>> {
        let limit = limit.clamp(1, 100);
        let conn = self.conn.lock();
        if let Some(status) = status {
            let mut stmt = conn.prepare(
                "SELECT id, kind, status, persona, lens, criteria, visual_level,
                        estimated_input_tokens, estimated_output_tokens, estimated_cost_eur,
                        source_context_json, items_json, guard_results_json,
                        apply_result_json, undo_journal_json, created_at, updated_at, applied_at
                 FROM agent_action_proposals
                 WHERE status = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![status, limit], map_agent_action_proposal)?;
            rows.collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, kind, status, persona, lens, criteria, visual_level,
                        estimated_input_tokens, estimated_output_tokens, estimated_cost_eur,
                        source_context_json, items_json, guard_results_json,
                        apply_result_json, undo_journal_json, created_at, updated_at, applied_at
                 FROM agent_action_proposals
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], map_agent_action_proposal)?;
            rows.collect()
        }
    }

    pub fn mark_action_proposal_applied(
        &self,
        id: &str,
        apply_result_json: &str,
        undo_journal_json: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE agent_action_proposals
             SET status = 'applied',
                 apply_result_json = ?2,
                 undo_journal_json = ?3,
                 applied_at = datetime('now'),
                 updated_at = datetime('now')
             WHERE id = ?1 AND status = 'pending'",
            params![id, apply_result_json, undo_journal_json],
        )?;
        Ok(())
    }

    pub fn dismiss_action_proposal(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE agent_action_proposals
             SET status = 'dismissed', updated_at = datetime('now')
             WHERE id = ?1 AND status = 'pending'",
            params![id],
        )?;
        Ok(())
    }
}
```

Add to `src-tauri/src/db_core/mod.rs`:

```rust
pub mod agent_proposals;
```

- [ ] **Step 6: Run tests**

Run:

```bash
cd src-tauri && cargo test --lib db_core::agent_proposals::tests
```

Expected: PASS.

- [ ] **Step 7: Run migration safety tests**

Run:

```bash
cd src-tauri && cargo test --lib db_core::db::migration_safety_tests
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/db_core/agent_proposals.rs src-tauri/src/db_core/models.rs src-tauri/src/db_core/mod.rs src-tauri/src/db_core/db.rs src-tauri/src/db_core/schema.sql
git commit -m "feat: persist agent action proposals"
```

---

### Task 2: Return Detailed Trash Results

**Files:**
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Create test: `src/lib/agent-proposals-api.test.ts`

**Interfaces:**
- Consumes: `TrashImageResult`, `TrashImagesDetailedResult`
- Produces Tauri command: `trash_images_detailed(image_ids: Vec<String>) -> Result<TrashImagesDetailedResult, String>`
- Produces frontend API: `trashImagesDetailed(imageIds: string[]): Promise<TrashImagesDetailedResult>`
- Keeps existing `trashImages(imageIds): Promise<number>` API stable for current call sites.

- [ ] **Step 1: Add frontend API wrapper test**

Create `src/lib/agent-proposals-api.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { trashImagesDetailed } from './api';

describe('trashImagesDetailed', () => {
    it('invokes trash_images_detailed with imageIds', async () => {
        vi.mocked(invoke).mockResolvedValueOnce({
            requested: 1,
            succeeded: 1,
            failed: 0,
            results: [{ image_id: 'img_1', path: '/tmp/a.png', status: 'trashed', error: null }],
        });

        await expect(trashImagesDetailed(['img_1'])).resolves.toMatchObject({
            requested: 1,
            succeeded: 1,
            failed: 0,
        });
        expect(invoke).toHaveBeenCalledWith('trash_images_detailed', { imageIds: ['img_1'] });
    });
});
```

- [ ] **Step 2: Run frontend test to verify it fails**

Run:

```bash
npm test -- src/lib/agent-proposals-api.test.ts
```

Expected: FAIL because `trashImagesDetailed` is not exported.

- [ ] **Step 3: Add frontend types and wrapper**

Add to `src/lib/api.ts` near existing delete commands:

```ts
export interface TrashImageResult {
    image_id: string;
    path: string | null;
    status: 'trashed' | 'missing' | 'not_found' | 'failed';
    error: string | null;
}

export interface TrashImagesDetailedResult {
    requested: number;
    succeeded: number;
    failed: number;
    results: TrashImageResult[];
}

export async function trashImagesDetailed(imageIds: string[]): Promise<TrashImagesDetailedResult> {
    return invoke<TrashImagesDetailedResult>('trash_images_detailed', { imageIds });
}
```

- [ ] **Step 4: Add Rust command**

Add to `src-tauri/src/commands/library.rs` after `trash_images`:

```rust
#[tauri::command]
pub async fn trash_images_detailed(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<crate::db_core::models::TrashImagesDetailedResult, String> {
    let mut results = Vec::new();

    for image_id in &image_ids {
        let id_refs = vec![image_id.as_str()];
        let found = state
            .db
            .get_images_by_ids(&id_refs)
            .map_err(|e| e.to_string())?;

        let Some(img) = found.first() else {
            results.push(crate::db_core::models::TrashImageResult {
                image_id: image_id.clone(),
                path: None,
                status: "not_found".to_string(),
                error: Some("Image was not found in the library".to_string()),
            });
            continue;
        };

        let path = std::path::Path::new(&img.path);
        if !path.exists() {
            results.push(crate::db_core::models::TrashImageResult {
                image_id: image_id.clone(),
                path: Some(img.path.clone()),
                status: "missing".to_string(),
                error: Some("File is already missing on disk".to_string()),
            });
            continue;
        }

        match trash::delete(path) {
            Ok(()) => {
                let _ = state.db.mark_file_missing(&img.path);
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                let _ = state.action_manager.record_action(
                    &state.db,
                    "trash_image",
                    format!("Trash {}", filename),
                    serde_json::json!({"image_id": image_id, "path": &img.path}).to_string(),
                    serde_json::json!({"image_id": image_id, "path": &img.path, "trashed": true}).to_string(),
                    image_id.clone(),
                    true,
                );
                results.push(crate::db_core::models::TrashImageResult {
                    image_id: image_id.clone(),
                    path: Some(img.path.clone()),
                    status: "trashed".to_string(),
                    error: None,
                });
            }
            Err(e) => {
                results.push(crate::db_core::models::TrashImageResult {
                    image_id: image_id.clone(),
                    path: Some(img.path.clone()),
                    status: "failed".to_string(),
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let succeeded = results.iter().filter(|r| r.status == "trashed").count() as u32;
    let failed = results.len() as u32 - succeeded;
    Ok(crate::db_core::models::TrashImagesDetailedResult {
        requested: image_ids.len() as u32,
        succeeded,
        failed,
        results,
    })
}
```

Register in `src-tauri/src/lib.rs` inside `tauri::generate_handler!` next to `trash_images`:

```rust
commands::library::trash_images_detailed,
```

- [ ] **Step 5: Run frontend test**

Run:

```bash
npm test -- src/lib/agent-proposals-api.test.ts
```

Expected: PASS.

- [ ] **Step 6: Run Rust compile check for command registration**

Run:

```bash
cd src-tauri && cargo test --lib commands::library
```

Expected: PASS or "0 tests" with successful compile.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/library.rs src-tauri/src/lib.rs src/lib/api.ts src/lib/agent-proposals-api.test.ts
git commit -m "feat: return detailed trash results"
```

---

### Task 3: Add Proposal Commands And Apply Service

**Files:**
- Create: `src-tauri/src/services/agent_proposals.rs`
- Create: `src-tauri/src/commands/agent_proposals.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 database methods.
- Consumes: Task 2 detailed Trash behavior internally.
- Produces Tauri commands:
  - `create_action_proposal(request: CreateActionProposalRequest) -> Result<AgentActionProposal, String>`
  - `list_action_proposals(status: Option<String>, limit: Option<u32>) -> Result<Vec<AgentActionProposal>, String>`
  - `dismiss_action_proposal(proposal_id: String) -> Result<(), String>`
  - `apply_action_proposal(proposal_id: String, approved_image_ids: Vec<String>) -> Result<ApplyActionProposalResult, String>`

- [ ] **Step 1: Add failing service tests**

Create `src-tauri/src/services/agent_proposals.rs` with tests first:

```rust
use crate::db_core::models::*;
use crate::services::{ServiceContext, ServiceError};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use tempfile::tempdir;

    fn db() -> Database {
        let dir = tempdir().unwrap();
        Database::open(&dir.path().join("test.db")).unwrap()
    }

    #[test]
    fn validate_proposal_rejects_destructive_direct_apply_without_candidates() {
        let err = validate_create_request(&CreateActionProposalRequest {
            kind: "trash_images".to_string(),
            persona: "copilot".to_string(),
            lens: Some("near_duplicates".to_string()),
            criteria: "cleanup".to_string(),
            visual_level: "tiny".to_string(),
            estimated_input_tokens: Some(1),
            estimated_output_tokens: Some(1),
            estimated_cost_eur: Some(0.001),
            source_context_json: "{}".to_string(),
            items_json: "[]".to_string(),
            guard_results_json: "{}".to_string(),
        })
        .unwrap_err();
        assert!(err.to_string().contains("at least one candidate"));
    }

    #[test]
    fn create_proposal_persists_pending_request() {
        let db = db();
        let request = CreateActionProposalRequest {
            kind: "remove_from_collection".to_string(),
            persona: "copilot".to_string(),
            lens: Some("collection".to_string()),
            criteria: "remove weak alternates".to_string(),
            visual_level: "text".to_string(),
            estimated_input_tokens: Some(300),
            estimated_output_tokens: Some(100),
            estimated_cost_eur: Some(0.002),
            source_context_json: "{}".to_string(),
            items_json: r#"[{"image_id":"img_a","reason":"not selected"}]"#.to_string(),
            guard_results_json: "{}".to_string(),
        };

        let proposal = create_action_proposal_db(&db, request).unwrap();
        assert_eq!(proposal.status, "pending");
        assert_eq!(proposal.kind, "remove_from_collection");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test --lib services::agent_proposals::tests
```

Expected: compile failure because request/result structs and service functions are missing.

- [ ] **Step 3: Add service structs and validation**

At top of `src-tauri/src/services/agent_proposals.rs`, above tests:

```rust
use crate::db_core::db::Database;
use crate::db_core::models::{AgentActionProposal, CreateActionProposalDb};
use crate::services::ServiceError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActionProposalRequest {
    pub kind: String,
    pub persona: String,
    pub lens: Option<String>,
    pub criteria: String,
    pub visual_level: String,
    pub estimated_input_tokens: Option<i64>,
    pub estimated_output_tokens: Option<i64>,
    pub estimated_cost_eur: Option<f64>,
    pub source_context_json: String,
    pub items_json: String,
    pub guard_results_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyActionProposalResult {
    pub proposal_id: String,
    pub status: String,
    pub applied_count: u32,
    pub failed_count: u32,
    pub result_json: String,
}

pub fn validate_create_request(request: &CreateActionProposalRequest) -> Result<(), ServiceError> {
    let valid_kind = matches!(
        request.kind.as_str(),
        "select_images"
            | "set_decisions"
            | "create_collection"
            | "add_to_collection"
            | "remove_from_collection"
            | "reorder_canvas"
            | "remove_from_canvas"
            | "trash_images"
    );
    if !valid_kind {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported proposal kind '{}'",
            request.kind
        )));
    }
    if !matches!(request.persona.as_str(), "curator" | "copilot" | "operator") {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported persona '{}'",
            request.persona
        )));
    }
    if !matches!(request.visual_level.as_str(), "text" | "tiny" | "preview" | "full") {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported visual level '{}'",
            request.visual_level
        )));
    }
    let items: serde_json::Value = serde_json::from_str(&request.items_json)
        .map_err(|e| ServiceError::InvalidInput(format!("Invalid items_json: {}", e)))?;
    if request.kind != "create_collection" && items.as_array().map(|a| a.is_empty()).unwrap_or(true) {
        return Err(ServiceError::InvalidInput(
            "Proposal requires at least one candidate".to_string(),
        ));
    }
    Ok(())
}

pub fn create_action_proposal_db(
    db: &Database,
    request: CreateActionProposalRequest,
) -> Result<AgentActionProposal, ServiceError> {
    validate_create_request(&request)?;
    db.create_action_proposal(CreateActionProposalDb {
        kind: request.kind,
        persona: request.persona,
        lens: request.lens,
        criteria: request.criteria,
        visual_level: request.visual_level,
        estimated_input_tokens: request.estimated_input_tokens,
        estimated_output_tokens: request.estimated_output_tokens,
        estimated_cost_eur: request.estimated_cost_eur,
        source_context_json: request.source_context_json,
        items_json: request.items_json,
        guard_results_json: request.guard_results_json,
    })
    .map_err(ServiceError::Database)
}
```

- [ ] **Step 4: Add minimal apply support for this slice**

Append to `src-tauri/src/services/agent_proposals.rs`:

```rust
pub fn list_action_proposals_db(
    db: &Database,
    status: Option<&str>,
    limit: u32,
) -> Result<Vec<AgentActionProposal>, ServiceError> {
    db.list_action_proposals(status, limit)
        .map_err(ServiceError::Database)
}

pub fn dismiss_action_proposal_db(db: &Database, proposal_id: &str) -> Result<(), ServiceError> {
    db.dismiss_action_proposal(proposal_id)
        .map_err(ServiceError::Database)
}

pub fn apply_action_proposal_db(
    db: &Database,
    proposal_id: &str,
    approved_image_ids: &[String],
    result_json: &str,
) -> Result<ApplyActionProposalResult, ServiceError> {
    let proposal = db
        .get_action_proposal(proposal_id)
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Proposal '{}' was not found", proposal_id)))?;
    if proposal.status != "pending" {
        return Err(ServiceError::InvalidInput(format!(
            "Proposal '{}' is not pending",
            proposal_id
        )));
    }
    let undo_journal_json = serde_json::json!({
        "proposal_id": proposal_id,
        "kind": proposal.kind,
        "approved_image_ids": approved_image_ids,
    })
    .to_string();
    db.mark_action_proposal_applied(proposal_id, result_json, &undo_journal_json)
        .map_err(ServiceError::Database)?;
    Ok(ApplyActionProposalResult {
        proposal_id: proposal_id.to_string(),
        status: "applied".to_string(),
        applied_count: approved_image_ids.len() as u32,
        failed_count: 0,
        result_json: result_json.to_string(),
    })
}
```

Add to `src-tauri/src/services/mod.rs`:

```rust
pub mod agent_proposals;
```

- [ ] **Step 5: Add Tauri command wrappers**

Create `src-tauri/src/commands/agent_proposals.rs`:

```rust
use crate::db_core::models::AgentActionProposal;
use crate::services::agent_proposals as svc;
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
```

Add to `src-tauri/src/commands/mod.rs`:

```rust
pub mod agent_proposals;
```

Register in `src-tauri/src/lib.rs`:

```rust
commands::agent_proposals::create_action_proposal,
commands::agent_proposals::list_action_proposals,
commands::agent_proposals::dismiss_action_proposal,
commands::agent_proposals::apply_action_proposal,
```

- [ ] **Step 6: Run service tests**

Run:

```bash
cd src-tauri && cargo test --lib services::agent_proposals::tests
```

Expected: PASS.

- [ ] **Step 7: Run full Rust lib tests**

Run:

```bash
cd src-tauri && cargo test --lib
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/services/agent_proposals.rs src-tauri/src/services/mod.rs src-tauri/src/commands/agent_proposals.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add action proposal commands"
```

---

### Task 4: Add Frontend Proposal API And Stores

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores.ts`
- Test: `src/lib/stores.test.ts`

**Interfaces:**
- Consumes Tauri commands from Task 3.
- Produces TypeScript types: `AgentActionProposal`, `CreateActionProposalRequest`, `ApplyActionProposalResult`, `AgentVisualLevel`, `AgentPersona`
- Produces stores: `agentPanelPinned`, `agentPanelVisible`, `agentVisualLevel`, `activeAgentProposalId`

- [ ] **Step 1: Add store tests**

Append to `src/lib/stores.test.ts`:

```ts
import { get } from 'svelte/store';
import { agentVisualLevel, cycleAgentVisualLevel } from './stores';

describe('agent visual level store', () => {
    it('defaults to tiny and cycles through guarded visual levels', () => {
        agentVisualLevel.set('tiny');
        expect(get(agentVisualLevel)).toBe('tiny');
        cycleAgentVisualLevel();
        expect(get(agentVisualLevel)).toBe('preview');
        cycleAgentVisualLevel();
        expect(get(agentVisualLevel)).toBe('full');
        cycleAgentVisualLevel();
        expect(get(agentVisualLevel)).toBe('text');
        cycleAgentVisualLevel();
        expect(get(agentVisualLevel)).toBe('tiny');
    });
});
```

- [ ] **Step 2: Run store test to verify it fails**

Run:

```bash
npm test -- src/lib/stores.test.ts
```

Expected: FAIL because `agentVisualLevel` and `cycleAgentVisualLevel` are missing.

- [ ] **Step 3: Add API types and wrappers**

Add to `src/lib/api.ts`:

```ts
export type AgentPersona = 'curator' | 'copilot' | 'operator';
export type AgentVisualLevel = 'text' | 'tiny' | 'preview' | 'full';
export type AgentProposalStatus = 'pending' | 'applied' | 'dismissed';

export interface AgentActionProposal {
    id: string;
    kind: string;
    status: AgentProposalStatus;
    persona: AgentPersona;
    lens: string | null;
    criteria: string;
    visual_level: AgentVisualLevel;
    estimated_input_tokens: number | null;
    estimated_output_tokens: number | null;
    estimated_cost_eur: number | null;
    source_context_json: string;
    items_json: string;
    guard_results_json: string;
    apply_result_json: string | null;
    undo_journal_json: string | null;
    created_at: string;
    updated_at: string;
    applied_at: string | null;
}

export interface CreateActionProposalRequest {
    kind: string;
    persona: AgentPersona;
    lens: string | null;
    criteria: string;
    visual_level: AgentVisualLevel;
    estimated_input_tokens: number | null;
    estimated_output_tokens: number | null;
    estimated_cost_eur: number | null;
    source_context_json: string;
    items_json: string;
    guard_results_json: string;
}

export interface ApplyActionProposalResult {
    proposal_id: string;
    status: string;
    applied_count: number;
    failed_count: number;
    result_json: string;
}

export async function createActionProposal(request: CreateActionProposalRequest): Promise<AgentActionProposal> {
    return invoke<AgentActionProposal>('create_action_proposal', { request });
}

export async function listActionProposals(status: AgentProposalStatus | null = 'pending', limit = 20): Promise<AgentActionProposal[]> {
    return invoke<AgentActionProposal[]>('list_action_proposals', { status, limit });
}

export async function dismissActionProposal(proposalId: string): Promise<void> {
    return invoke<void>('dismiss_action_proposal', { proposalId });
}

export async function applyActionProposal(proposalId: string, approvedImageIds: string[], resultJson: string): Promise<ApplyActionProposalResult> {
    return invoke<ApplyActionProposalResult>('apply_action_proposal', { proposalId, approvedImageIds, resultJson });
}
```

- [ ] **Step 4: Add stores**

Add to `src/lib/stores.ts`:

```ts
export type AgentVisualLevel = 'text' | 'tiny' | 'preview' | 'full';

export const agentPanelPinned = writable<boolean>(false);
export const agentPanelVisible = writable<boolean>(false);
export const activeAgentProposalId = writable<string | null>(null);
export const agentVisualLevel = writable<AgentVisualLevel>('tiny');

const VISUAL_LEVELS: AgentVisualLevel[] = ['tiny', 'preview', 'full', 'text'];

export function cycleAgentVisualLevel() {
    agentVisualLevel.update(current => {
        const index = VISUAL_LEVELS.indexOf(current);
        return VISUAL_LEVELS[(index + 1) % VISUAL_LEVELS.length];
    });
}
```

- [ ] **Step 5: Run store tests**

Run:

```bash
npm test -- src/lib/stores.test.ts
```

Expected: PASS.

- [ ] **Step 6: Run frontend typecheck**

Run:

```bash
npm run check
```

Expected: PASS with `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 7: Commit**

```bash
git add src/lib/api.ts src/lib/stores.ts src/lib/stores.test.ts
git commit -m "feat: add agent proposal frontend state"
```

---

### Task 5: Add Right Agent Dock UI Shell

**Files:**
- Create: `src/lib/components/AgentProposalDock.svelte`
- Create: `src/lib/components/agent-proposal-dock.test.ts`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Consumes: `AgentActionProposal`, `AgentVisualLevel`, `agentPanelPinned`, `agentPanelVisible`, `agentVisualLevel`, `cycleAgentVisualLevel`
- Accepts callback props, matching existing Svelte 5 component style:
  - `onreviewproposal(proposalId: string)`
  - `ondismissproposal(proposalId: string)`
  - `onvisuallevelcycle()`
  - `onclose()`

- [ ] **Step 1: Add component test**

Create `src/lib/components/agent-proposal-dock.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import AgentProposalDock from './AgentProposalDock.svelte';
import type { AgentActionProposal } from '$lib/api';

function proposal(overrides: Partial<AgentActionProposal> = {}): AgentActionProposal {
    return {
        id: 'proposal_1',
        kind: 'trash_images',
        status: 'pending',
        persona: 'copilot',
        lens: 'near_duplicates',
        criteria: 'near duplicate cleanup',
        visual_level: 'tiny',
        estimated_input_tokens: 2100,
        estimated_output_tokens: 420,
        estimated_cost_eur: 0.014,
        source_context_json: '{}',
        items_json: JSON.stringify([
            { image_id: 'img_1', reason: 'lower focus', confidence: 0.91 },
        ]),
        guard_results_json: '{}',
        apply_result_json: null,
        undo_journal_json: null,
        created_at: '2026-06-23T10:00:00Z',
        updated_at: '2026-06-23T10:00:00Z',
        applied_at: null,
        ...overrides,
    };
}

describe('AgentProposalDock', () => {
    it('shows proposal summary and secondary context chip', () => {
        const { getByText } = render(AgentProposalDock, {
            props: { proposals: [proposal()], pinned: true, visible: true, visualLevel: 'tiny' },
        });
        expect(getByText('Trash proposal ready')).toBeTruthy();
        expect(getByText(/Context: Tiny/)).toBeTruthy();
        expect(getByText(/lower focus/)).toBeTruthy();
    });

    it('calls onreviewproposal from review gate button', async () => {
        const onreviewproposal = vi.fn();
        const { getByText } = render(AgentProposalDock, {
            props: {
                proposals: [proposal()],
                pinned: true,
                visible: true,
                visualLevel: 'tiny',
                onreviewproposal,
            },
        });
        await fireEvent.click(getByText('Open review gate'));
        expect(onreviewproposal).toHaveBeenCalledWith('proposal_1');
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm test -- src/lib/components/agent-proposal-dock.test.ts
```

Expected: FAIL because component does not exist.

- [ ] **Step 3: Create component**

Create `src/lib/components/AgentProposalDock.svelte`:

```svelte
<script lang="ts">
    import type { AgentActionProposal, AgentVisualLevel } from '$lib/api';

    type Candidate = {
        image_id: string;
        reason?: string;
        confidence?: number | string;
    };

    let {
        proposals,
        pinned,
        visible,
        visualLevel,
        onreviewproposal = () => {},
        ondismissproposal = () => {},
        onvisuallevelcycle = () => {},
        onclose = () => {},
    }: {
        proposals: AgentActionProposal[];
        pinned: boolean;
        visible: boolean;
        visualLevel: AgentVisualLevel;
        onreviewproposal?: (proposalId: string) => void;
        ondismissproposal?: (proposalId: string) => void;
        onvisuallevelcycle?: () => void;
        onclose?: () => void;
    } = $props();

    const activeProposal = $derived(proposals.find(p => p.status === 'pending') ?? null);
    const candidates = $derived(parseCandidates(activeProposal?.items_json));
    const contextLabel = $derived(visualLevel === 'text' ? 'Text-only' : visualLevel[0].toUpperCase() + visualLevel.slice(1));

    function parseCandidates(itemsJson: string | undefined): Candidate[] {
        if (!itemsJson) return [];
        try {
            const parsed = JSON.parse(itemsJson);
            return Array.isArray(parsed) ? parsed : [];
        } catch {
            return [];
        }
    }

</script>

{#if visible || pinned}
    <aside class:pinned class:floating={!pinned} class="agent-dock" aria-label="Claude Agent proposal panel">
        <header class="agent-header">
            <div>
                <strong>Claude Agent</strong>
                <span>{activeProposal?.lens ?? 'Curator'} - proposal mode</span>
            </div>
            <button class="icon-button" type="button" title="Close" onclick={onclose}>x</button>
        </header>

        <button class="context-chip" type="button" title="Change visual level" onclick={onvisuallevelcycle}>
            Context: {contextLabel} - EUR {activeProposal?.estimated_cost_eur?.toFixed(3) ?? '0.000'} est - {activeProposal?.estimated_input_tokens ?? 0} tokens
        </button>

        {#if activeProposal}
            <section class="summary">
                <div class="persona-row">
                    <span class:active={activeProposal.persona === 'curator'}>Curator</span>
                    <span class:active={activeProposal.persona === 'copilot'}>Copilot</span>
                    <span class:active={activeProposal.persona === 'operator'}>Operator</span>
                </div>
                <h3>Trash proposal ready</h3>
                <p>{activeProposal.criteria}</p>
                <button
                    class="primary"
                    type="button"
                    onclick={() => onreviewproposal(activeProposal.id)}
                >
                    Open review gate
                </button>
            </section>

            <section class="candidate-list" aria-label="Candidate reasons">
                {#each candidates as candidate}
                    <article class="candidate">
                        <div class="mini-thumb"></div>
                        <div>
                            <strong>{candidate.image_id}</strong>
                            <span>{candidate.reason ?? 'Candidate selected by proposal criteria'}</span>
                        </div>
                    </article>
                {/each}
            </section>
        {:else}
            <section class="empty">
                <h3>No active proposal</h3>
                <p>Ask Claude to analyze the current view or create a curation proposal.</p>
            </section>
        {/if}
    </aside>
{/if}

<style>
    .agent-dock {
        background: var(--bg);
        border-left: 1px solid var(--border);
        color: var(--text);
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
        min-width: 320px;
        padding: var(--spacing);
    }

    .agent-dock.floating {
        bottom: 36px;
        box-shadow: 0 20px 70px rgba(0, 0, 0, 0.48);
        position: absolute;
        right: 16px;
        top: 56px;
        width: 340px;
        z-index: 20;
    }

    .agent-header,
    .persona-row {
        align-items: center;
        display: flex;
        justify-content: space-between;
        gap: var(--spacing);
    }

    .agent-header span,
    .summary p,
    .candidate span,
    .empty p {
        color: var(--text-secondary);
        display: block;
        font-size: 11px;
    }

    .icon-button,
    .context-chip,
    .primary {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font: inherit;
        padding: 6px 8px;
    }

    .context-chip {
        color: var(--text-secondary);
        text-align: left;
    }

    .persona-row span {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text-secondary);
        font-size: 10px;
        padding: 4px 6px;
    }

    .persona-row span.active,
    .primary {
        border-color: var(--blue);
        color: var(--blue);
    }

    .summary,
    .candidate,
    .empty {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: var(--spacing);
    }

    .candidate-list {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
    }

    .candidate {
        display: grid;
        grid-template-columns: 40px 1fr;
        gap: var(--spacing);
    }

    .mini-thumb {
        aspect-ratio: 1 / 1;
        background: var(--border);
        border-radius: var(--radius);
    }
</style>
```

- [ ] **Step 4: Wire component into page**

Modify `src/routes/+page.svelte` imports:

```ts
import AgentProposalDock from '$lib/components/AgentProposalDock.svelte';
import { listActionProposals, type AgentActionProposal } from '$lib/api';
```

Extend the stores import with:

```ts
agentPanelPinned, agentPanelVisible, agentVisualLevel, cycleAgentVisualLevel, activeAgentProposalId
```

Add local state in the script:

```ts
let agentProposals = $state<AgentActionProposal[]>([]);

async function refreshAgentProposals() {
    agentProposals = await listActionProposals('pending', 20);
}

function handleReviewProposal(proposalId: string) {
    activeAgentProposalId.set(proposalId);
}
```

Call `refreshAgentProposals()` inside `onMount` after initial setup:

```ts
refreshAgentProposals().catch(e => console.error('Failed to load agent proposals:', e));
```

Mount the component near the main view container so pinned mode can later reflow:

```svelte
<AgentProposalDock
    proposals={agentProposals}
    pinned={$agentPanelPinned}
    visible={$agentPanelVisible}
    visualLevel={$agentVisualLevel}
    onreviewproposal={handleReviewProposal}
    onvisuallevelcycle={cycleAgentVisualLevel}
    onclose={() => agentPanelVisible.set(false)}
/>
```

- [ ] **Step 5: Run component test**

Run:

```bash
npm test -- src/lib/components/agent-proposal-dock.test.ts
```

Expected: PASS.

- [ ] **Step 6: Run Svelte check**

Run:

```bash
npm run check
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/AgentProposalDock.svelte src/lib/components/agent-proposal-dock.test.ts src/routes/+page.svelte
git commit -m "feat: add agent proposal dock shell"
```

---

### Task 6: Add Proposal Review Gate

**Files:**
- Create: `src/lib/components/ActionProposalReviewDialog.svelte`
- Create: `src/lib/components/action-proposal-review-dialog.test.ts`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Consumes: `AgentActionProposal`
- Accepts callback props:
  - `onapplyproposal(proposalId: string, approvedImageIds: string[])`
  - `oncancelreview()`

- [ ] **Step 1: Add review dialog test**

Create `src/lib/components/action-proposal-review-dialog.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import ActionProposalReviewDialog from './ActionProposalReviewDialog.svelte';
import type { AgentActionProposal } from '$lib/api';

const proposal: AgentActionProposal = {
    id: 'proposal_1',
    kind: 'trash_images',
    status: 'pending',
    persona: 'copilot',
    lens: 'near_duplicates',
    criteria: 'weak duplicates',
    visual_level: 'tiny',
    estimated_input_tokens: 2100,
    estimated_output_tokens: 420,
    estimated_cost_eur: 0.014,
    source_context_json: '{}',
    items_json: JSON.stringify([
        { image_id: 'img_1', reason: 'lower focus' },
        { image_id: 'img_2', reason: 'guarded review' },
    ]),
    guard_results_json: '{}',
    apply_result_json: null,
    undo_journal_json: null,
    created_at: '2026-06-23T10:00:00Z',
    updated_at: '2026-06-23T10:00:00Z',
    applied_at: null,
};

describe('ActionProposalReviewDialog', () => {
    it('lets the user deselect candidates before applying', async () => {
        const onapplyproposal = vi.fn();
        const { getByLabelText, getByText } = render(ActionProposalReviewDialog, {
            props: { proposal, visible: true, onapplyproposal },
        });

        await fireEvent.click(getByLabelText('Include img_2'));
        await fireEvent.click(getByText('Move approved to Trash'));

        expect(onapplyproposal).toHaveBeenCalledWith('proposal_1', ['img_1']);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm test -- src/lib/components/action-proposal-review-dialog.test.ts
```

Expected: FAIL because component does not exist.

- [ ] **Step 3: Create review dialog component**

Create `src/lib/components/ActionProposalReviewDialog.svelte`:

```svelte
<script lang="ts">
    import type { AgentActionProposal } from '$lib/api';

    type Candidate = { image_id: string; reason?: string };

    let {
        proposal,
        visible,
        onapplyproposal = () => {},
        oncancelreview = () => {},
    }: {
        proposal: AgentActionProposal | null;
        visible: boolean;
        onapplyproposal?: (proposalId: string, approvedImageIds: string[]) => void;
        oncancelreview?: () => void;
    } = $props();
    let approvedIds = $state<Set<string>>(new Set());

    const candidates = $derived(parseCandidates(proposal?.items_json));

    $effect(() => {
        approvedIds = new Set(candidates.map(candidate => candidate.image_id));
    });

    function parseCandidates(itemsJson: string | undefined): Candidate[] {
        if (!itemsJson) return [];
        try {
            const parsed = JSON.parse(itemsJson);
            return Array.isArray(parsed) ? parsed : [];
        } catch {
            return [];
        }
    }

    function toggle(imageId: string) {
        const next = new Set(approvedIds);
        if (next.has(imageId)) next.delete(imageId);
        else next.add(imageId);
        approvedIds = next;
    }

</script>

{#if visible && proposal}
    <div class="backdrop" role="presentation">
        <section class="dialog" role="dialog" aria-modal="true" aria-label="Trash proposal review">
            <header>
                <div>
                    <h2>Review Trash proposal</h2>
                    <p>{proposal.criteria}</p>
                </div>
                <button type="button" onclick={oncancelreview}>Cancel</button>
            </header>

            <div class="summary">
                <span>{approvedIds.size} of {candidates.length} approved</span>
                <span>Context: {proposal.visual_level}</span>
                <span>Estimated EUR {proposal.estimated_cost_eur?.toFixed(3) ?? '0.000'}</span>
            </div>

            <div class="candidate-list">
                {#each candidates as candidate}
                    <label class="candidate">
                        <input
                            type="checkbox"
                            checked={approvedIds.has(candidate.image_id)}
                            aria-label={`Include ${candidate.image_id}`}
                            onchange={() => toggle(candidate.image_id)}
                        />
                        <span>
                            <strong>{candidate.image_id}</strong>
                            <small>{candidate.reason ?? 'Candidate selected by proposal criteria'}</small>
                        </span>
                    </label>
                {/each}
            </div>

            <footer>
                <button type="button" onclick={oncancelreview}>Keep reviewing</button>
                <button
                    class="danger"
                    type="button"
                    onclick={() => onapplyproposal(proposal.id, Array.from(approvedIds))}
                    disabled={approvedIds.size === 0}
                >
                    Move approved to Trash
                </button>
            </footer>
        </section>
    </div>
{/if}

<style>
    .backdrop {
        align-items: center;
        background: rgba(0, 0, 0, 0.64);
        display: flex;
        inset: 0;
        justify-content: center;
        position: fixed;
        z-index: 100;
    }

    .dialog {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
        max-height: 80vh;
        max-width: 720px;
        padding: calc(var(--spacing) * 2);
        width: min(720px, calc(100vw - 32px));
    }

    header,
    footer,
    .summary {
        align-items: center;
        display: flex;
        gap: var(--spacing);
        justify-content: space-between;
    }

    h2 {
        font-size: 16px;
        margin: 0;
    }

    p,
    small,
    .summary {
        color: var(--text-secondary);
    }

    button {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font: inherit;
        padding: 6px 8px;
    }

    button.danger {
        border-color: var(--red);
        color: var(--red);
    }

    button:disabled {
        opacity: 0.45;
    }

    .candidate-list {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
        overflow: auto;
    }

    .candidate {
        align-items: center;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        display: grid;
        gap: var(--spacing);
        grid-template-columns: auto 1fr;
        padding: var(--spacing);
    }

    .candidate small {
        display: block;
        margin-top: 2px;
    }
</style>
```

- [ ] **Step 4: Wire review dialog into page**

In `src/routes/+page.svelte`, import:

```ts
import ActionProposalReviewDialog from '$lib/components/ActionProposalReviewDialog.svelte';
import { applyActionProposal, trashImagesDetailed } from '$lib/api';
```

Add state and handlers:

```ts
let reviewProposalId = $state<string | null>(null);
let reviewProposal = $derived(agentProposals.find(p => p.id === reviewProposalId) ?? null);

async function handleApplyProposal(proposalId: string, approvedImageIds: string[]) {
    const trashResult = await trashImagesDetailed(approvedImageIds);
    await applyActionProposal(proposalId, approvedImageIds, JSON.stringify(trashResult));
    reviewProposalId = null;
    await refreshAgentProposals();
    invalidateImageCache();
    await loadImages({ reset: true });
    showToast('Trash proposal applied', {
        detail: `${trashResult.succeeded} moved to Trash, ${trashResult.failed} failed`,
        type: trashResult.failed > 0 ? 'warning' : 'info',
        duration: 6000,
    });
}
```

Change the dock review handler to:

```ts
function handleReviewProposal(proposalId: string) {
    reviewProposalId = proposalId;
    activeAgentProposalId.set(proposalId);
}
```

Mount:

```svelte
<ActionProposalReviewDialog
    proposal={reviewProposal}
    visible={reviewProposal !== null}
    onapplyproposal={handleApplyProposal}
    oncancelreview={() => reviewProposalId = null}
/>
```

- [ ] **Step 5: Run review dialog test**

Run:

```bash
npm test -- src/lib/components/action-proposal-review-dialog.test.ts
```

Expected: PASS.

- [ ] **Step 6: Run Svelte check**

Run:

```bash
npm run check
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/ActionProposalReviewDialog.svelte src/lib/components/action-proposal-review-dialog.test.ts src/routes/+page.svelte
git commit -m "feat: add agent proposal review gate"
```

---

### Task 7: Manual Seed Proposal For End-To-End Testing

**Files:**
- Modify: `src/lib/components/AgentProposalDock.svelte`
- Modify: `src/routes/+page.svelte`
- Modify: `src/lib/api.ts` if needed for helper request typing
- Test: `tests/e2e/smoke.py` only if this surface enters the manual E2E gate

**Interfaces:**
- Consumes: `createActionProposal`
- Produces: a temporary deterministic UI action that creates a pending proposal from current visible images.

- [ ] **Step 1: Add a development-only seed action**

In `src/routes/+page.svelte`, import:

```ts
import { createActionProposal } from '$lib/api';
```

Add:

```ts
async function createManualTrashProposalFromSelection() {
    const ids = Array.from($selectedIds).slice(0, 6);
    if (ids.length === 0) {
        showToast('Select images before creating a proposal', { type: 'warning', duration: 4000 });
        return;
    }
    await createActionProposal({
        kind: 'trash_images',
        persona: 'copilot',
        lens: 'manual_review',
        criteria: 'Manual test proposal from current selection',
        visual_level: $agentVisualLevel,
        estimated_input_tokens: 0,
        estimated_output_tokens: 0,
        estimated_cost_eur: 0,
        source_context_json: JSON.stringify({ source: 'manual_seed', selected_count: ids.length }),
        items_json: JSON.stringify(ids.map(image_id => ({
            image_id,
            reason: 'Selected for manual proposal review',
            confidence: 'manual',
        }))),
        guard_results_json: JSON.stringify({ blocked: [] }),
    });
    await refreshAgentProposals();
    agentPanelPinned.set(true);
    agentPanelVisible.set(true);
}
```

Wire this to a small button in the dock empty state by passing a callback prop, or temporarily wire it to a `window` event that can be triggered from the command palette later:

```ts
window.addEventListener('create-agent-test-proposal', () => {
    createManualTrashProposalFromSelection().catch(e => console.error('Failed to create test proposal:', e));
});
```

This action must be labeled as a test/manual proposal in UI copy. Do not imply Claude created it.

- [ ] **Step 2: Verify manually in browser dev server**

Run:

```bash
npx vite dev --port 1420
```

In a browser test session, select images and run:

```js
window.dispatchEvent(new Event('create-agent-test-proposal'))
```

Expected:

- Agent panel opens pinned.
- Proposal appears.
- Review gate opens.
- Applying calls detailed Trash and removes successfully trashed images from the grid.

- [ ] **Step 3: Run frontend checks**

Run:

```bash
npm run check
npm test -- src/lib/components/agent-proposal-dock.test.ts src/lib/components/action-proposal-review-dialog.test.ts src/lib/stores.test.ts
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/routes/+page.svelte src/lib/components/AgentProposalDock.svelte src/lib/api.ts
git commit -m "feat: seed manual agent proposals"
```

---

### Task 8: Full Verification And Handoff

**Files:**
- Modify: `docs/superpowers/specs/2026-06-23-claude-agent-chat-design.md` only if implementation reveals a necessary correction.
- Modify: `.beads/issues.jsonl` when updating `imageview-64mk`.

**Interfaces:**
- Consumes all previous tasks.
- Produces a branch ready for review with issue status updated.

- [ ] **Step 1: Run focused tests**

Run:

```bash
npm run check
npm test -- src/lib/components/agent-proposal-dock.test.ts src/lib/components/action-proposal-review-dialog.test.ts src/lib/stores.test.ts
cd src-tauri && cargo test --lib db_core::agent_proposals::tests services::agent_proposals::tests
```

Expected: all PASS.

- [ ] **Step 2: Run full quick preflight**

Run:

```bash
npm run preflight -- quick
```

Expected: PASS.

- [ ] **Step 3: Run Rust formatting from the correct directory**

Run:

```bash
cd src-tauri && cargo fmt
```

Expected: command exits 0 and `git diff --check` has no whitespace errors.

- [ ] **Step 4: Run full Rust lib suite**

Run:

```bash
cd src-tauri && cargo test --lib
```

Expected: PASS.

- [ ] **Step 5: Update issue**

Run:

```bash
npm run bd -- update imageview-64mk --status in_progress
```

If the implementation is complete and accepted, close it:

```bash
npm run bd -- close imageview-64mk --reason "Implemented Claude Agent proposal slice"
```

- [ ] **Step 6: Commit final issue export if changed**

```bash
git add -f .beads/issues.jsonl
git commit -m "chore(bd): update claude agent proposal issue"
```

Skip this commit if `.beads/issues.jsonl` is unchanged.

- [ ] **Step 7: Land**

Run:

```bash
npm run land
```

Expected:

- Worktree clean.
- Full preflight passes or any existing environment limitation is documented.
- Branch pushes successfully.

---

## Self-Review

Spec coverage:

- Right dock/floating behavior: Task 5 creates the dock shell; Task 7 exercises it in-app.
- Proposal-first destructive boundary: Tasks 1, 3, and 6 persist proposals and gate apply.
- Per-file Trash results: Task 2 adds detailed results; Task 6 uses them.
- Thumbnail-first visual level: Task 4 creates visual-level state; Task 5 displays the secondary chip.
- Token/cost visibility: Tasks 1 and 5 store and display estimates.
- Undo journal foundation: Tasks 1 and 3 persist `undo_journal_json`; full undo execution remains follow-up because this slice focuses on apply safety and metadata.
- Claude SDK runtime and bundled skills: explicitly deferred to follow-up after proposal substrate exists.

Placeholder scan:

- No unresolved placeholder markers remain.
- All code-changing steps include concrete snippets or exact command text.

Type consistency:

- Rust DB types use `AgentActionProposal` and `CreateActionProposalDb`.
- Service command request uses `CreateActionProposalRequest`.
- Frontend API mirrors Rust JSON field names with snake_case properties.
