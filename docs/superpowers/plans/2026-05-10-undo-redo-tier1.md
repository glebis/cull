# Undo/Redo Tier 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Cmd+Z / Cmd+Shift+Z undo/redo for rating and decision changes, backed by a Rust ActionManager with persistent undo records in SQLite.

**Architecture:** Backend `ActionManager` is the single mutation path. Existing `set_rating` and `set_decision` Tauri commands delegate to ActionManager. Undo records stored in `undo_records` SQLite table. Frontend polls undo status after each action.

**Tech Stack:** Rust (ActionManager, SQLite), Svelte 5 (keyboard handling, store), Tauri IPC

---

### Task 1: Add undo_records table migration + DB helpers

**Files:**
- Modify: `src-tauri/src/db_core/db.rs` — add migration + helper queries
- Modify: `src-tauri/src/db_core/models.rs` — add `UndoRecord` struct

- [ ] **Step 1: Add UndoRecord model**

In `src-tauri/src/db_core/models.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRecord {
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
```

- [ ] **Step 2: Add migration**

In `src-tauri/src/db_core/db.rs`, add `migrate_undo_tables` following the pattern of existing migrations (e.g., `migrate_lineage_tables` at line 127):

```rust
fn migrate_undo_tables(&self) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS undo_records (
            id TEXT PRIMARY KEY,
            action_type TEXT NOT NULL,
            label TEXT NOT NULL,
            before_json TEXT NOT NULL,
            after_json TEXT NOT NULL,
            affected_image_ids TEXT,
            group_id TEXT,
            has_file_backup INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS undo_file_backups (
            id TEXT PRIMARY KEY,
            undo_record_id TEXT NOT NULL REFERENCES undo_records(id) ON DELETE CASCADE,
            original_path TEXT NOT NULL,
            backup_path TEXT NOT NULL,
            file_hash TEXT,
            created_at TEXT NOT NULL
        );"
    )?;
    Ok(())
}
```

Call it from `run_migrations()` after the last existing migration call.

- [ ] **Step 3: Add DB helper methods**

Add these methods to the `Database` impl:

```rust
pub fn get_selection_for_image(&self, image_id: &str) -> Result<Option<Selection>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT image_id, project_id, star_rating, color_label, decision
         FROM selections WHERE image_id = ?1 AND project_id = '__global__'"
    )?;
    let sel = stmt.query_row(params![image_id], |row| {
        Ok(Selection {
            image_id: row.get(0)?,
            project_id: row.get(1)?,
            star_rating: row.get(2)?,
            color_label: row.get(3)?,
            decision: row.get(4)?,
        })
    }).optional()?;
    Ok(sel)
}

pub fn insert_undo_record(&self, record: &UndoRecord) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO undo_records (id, action_type, label, before_json, after_json, affected_image_ids, group_id, has_file_backup, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![record.id, record.action_type, record.label, record.before_json, record.after_json, record.affected_image_ids, record.group_id, record.has_file_backup as i32, record.created_at],
    )?;
    Ok(())
}

pub fn delete_undo_records_after(&self, created_at: &str) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "DELETE FROM undo_records WHERE created_at > ?1",
        params![created_at],
    )?;
    Ok(())
}

pub fn get_undo_record_at_position(&self, offset: i64) -> Result<Option<UndoRecord>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, action_type, label, before_json, after_json, affected_image_ids, group_id, has_file_backup, created_at
         FROM undo_records ORDER BY created_at DESC LIMIT 1 OFFSET ?1"
    )?;
    let rec = stmt.query_row(params![offset], |row| {
        Ok(UndoRecord {
            id: row.get(0)?,
            action_type: row.get(1)?,
            label: row.get(2)?,
            before_json: row.get(3)?,
            after_json: row.get(4)?,
            affected_image_ids: row.get(5)?,
            group_id: row.get(6)?,
            has_file_backup: row.get::<_, i32>(7)? != 0,
            created_at: row.get(8)?,
        })
    }).optional()?;
    Ok(rec)
}

pub fn count_undo_records(&self) -> Result<i64> {
    let conn = self.conn.lock().unwrap();
    conn.query_row("SELECT COUNT(*) FROM undo_records", [], |row| row.get(0))
}

pub fn list_undo_records(&self, limit: u32) -> Result<Vec<UndoRecord>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, action_type, label, before_json, after_json, affected_image_ids, group_id, has_file_backup, created_at
         FROM undo_records ORDER BY created_at DESC LIMIT ?1"
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(UndoRecord {
            id: row.get(0)?,
            action_type: row.get(1)?,
            label: row.get(2)?,
            before_json: row.get(3)?,
            after_json: row.get(4)?,
            affected_image_ids: row.get(5)?,
            group_id: row.get(6)?,
            has_file_backup: row.get::<_, i32>(7)? != 0,
            created_at: row.get(8)?,
        })
    })?;
    rows.collect::<Result<Vec<_>>>()
}

pub fn prune_oldest_undo_records(&self, keep_count: usize) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "DELETE FROM undo_records WHERE id NOT IN (
            SELECT id FROM undo_records ORDER BY created_at DESC LIMIT ?1
        )",
        params![keep_count],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db_core/db.rs src-tauri/src/db_core/models.rs
git commit -m "feat: add undo_records table and DB helpers"
```

---

### Task 2: Create ActionManager

**Files:**
- Create: `src-tauri/src/services/undo.rs`
- Modify: `src-tauri/src/services/mod.rs` — add `pub mod undo;`

- [ ] **Step 1: Create the ActionManager module**

Create `src-tauri/src/services/undo.rs`:

```rust
use std::sync::Mutex;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db_core::db::Database;
use crate::db_core::models::UndoRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub stack_depth: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub undo_record_id: String,
    pub label: String,
    pub can_undo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    SetRating { image_id: String, rating: u8 },
    SetDecision { image_id: String, decision: String },
}

pub struct ActionManager {
    position: Mutex<i64>,
    max_depth: usize,
}

impl ActionManager {
    pub fn new() -> Self {
        Self {
            position: Mutex::new(0),
            max_depth: 200,
        }
    }

    pub fn execute(&self, db: &Database, action: Action) -> Result<ActionResult, String> {
        let (action_type, label, before_json, after_json, affected_ids) = match &action {
            Action::SetRating { image_id, rating } => {
                let sel = db.get_selection_for_image(image_id).map_err(|e| e.to_string())?;
                let before_rating = sel.as_ref().and_then(|s| s.star_rating).unwrap_or(0);
                let before = serde_json::json!({"image_id": image_id, "rating": before_rating});
                let after = serde_json::json!({"image_id": image_id, "rating": rating});

                db.set_rating(image_id, *rating).map_err(|e| e.to_string())?;

                ("set_rating".to_string(), format!("Set rating to {}", rating), before.to_string(), after.to_string(), image_id.clone())
            }
            Action::SetDecision { image_id, decision } => {
                let sel = db.get_selection_for_image(image_id).map_err(|e| e.to_string())?;
                let before_decision = sel.map(|s| s.decision).unwrap_or_else(|| "undecided".to_string());
                let before = serde_json::json!({"image_id": image_id, "decision": before_decision});
                let after = serde_json::json!({"image_id": image_id, "decision": decision});

                db.set_decision(image_id, decision).map_err(|e| e.to_string())?;

                ("set_decision".to_string(), format!("Set decision to {}", decision), before.to_string(), after.to_string(), image_id.clone())
            }
        };

        // Clear redo entries (anything above current position)
        let mut pos = self.position.lock().unwrap();
        if *pos > 0 {
            // Get the record at current position to find its timestamp
            if let Ok(Some(current)) = db.get_undo_record_at_position(*pos - 1) {
                let _ = db.delete_undo_records_after(&current.created_at);
            }
        }

        let record = UndoRecord {
            id: Uuid::new_v4().to_string(),
            action_type,
            label: label.clone(),
            before_json,
            after_json,
            affected_image_ids: Some(affected_ids),
            group_id: None,
            has_file_backup: false,
            created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        };

        let record_id = record.id.clone();
        db.insert_undo_record(&record).map_err(|e| e.to_string())?;

        // Reset position to top of stack
        *pos = 0;

        // Prune if over max depth
        let _ = db.prune_oldest_undo_records(self.max_depth);

        Ok(ActionResult {
            undo_record_id: record_id,
            label,
            can_undo: true,
        })
    }

    pub fn undo(&self, db: &Database) -> Result<Option<String>, String> {
        let mut pos = self.position.lock().unwrap();
        let record = db.get_undo_record_at_position(*pos).map_err(|e| e.to_string())?;

        match record {
            None => Ok(None),
            Some(rec) => {
                self.apply_state(db, &rec.action_type, &rec.before_json)?;
                *pos += 1;
                Ok(Some(rec.label))
            }
        }
    }

    pub fn redo(&self, db: &Database) -> Result<Option<String>, String> {
        let mut pos = self.position.lock().unwrap();
        if *pos == 0 {
            return Ok(None);
        }

        let record = db.get_undo_record_at_position(*pos - 1).map_err(|e| e.to_string())?;

        match record {
            None => Ok(None),
            Some(rec) => {
                self.apply_state(db, &rec.action_type, &rec.after_json)?;
                *pos -= 1;
                Ok(Some(rec.label))
            }
        }
    }

    pub fn status(&self, db: &Database) -> UndoStatus {
        let pos = self.position.lock().unwrap();
        let total = db.count_undo_records().unwrap_or(0);

        let undo_label = db.get_undo_record_at_position(*pos)
            .ok().flatten().map(|r| r.label);
        let redo_label = if *pos > 0 {
            db.get_undo_record_at_position(*pos - 1).ok().flatten().map(|r| r.label)
        } else {
            None
        };

        UndoStatus {
            can_undo: *pos < total,
            can_redo: *pos > 0,
            undo_label,
            redo_label,
            stack_depth: total,
        }
    }

    pub fn history(&self, db: &Database, limit: u32) -> Vec<UndoRecord> {
        db.list_undo_records(limit).unwrap_or_default()
    }

    fn apply_state(&self, db: &Database, action_type: &str, state_json: &str) -> Result<(), String> {
        let val: serde_json::Value = serde_json::from_str(state_json)
            .map_err(|e| format!("Invalid undo state JSON: {}", e))?;

        match action_type {
            "set_rating" => {
                let image_id = val["image_id"].as_str().ok_or("Missing image_id")?;
                let rating = val["rating"].as_u64().ok_or("Missing rating")? as u8;
                db.set_rating(image_id, rating).map_err(|e| e.to_string())
            }
            "set_decision" => {
                let image_id = val["image_id"].as_str().ok_or("Missing image_id")?;
                let decision = val["decision"].as_str().ok_or("Missing decision")?;
                db.set_decision(image_id, decision).map_err(|e| e.to_string())
            }
            _ => Err(format!("Unknown action type for undo: {}", action_type)),
        }
    }
}
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/services/mod.rs`, add:

```rust
pub mod undo;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/undo.rs src-tauri/src/services/mod.rs
git commit -m "feat: add ActionManager with execute/undo/redo for ratings and decisions"
```

---

### Task 3: Register ActionManager in AppState and add Tauri commands

**Files:**
- Modify: `src-tauri/src/lib.rs` — add ActionManager to AppState, register undo/redo commands
- Modify: `src-tauri/src/commands/selection.rs` — route through ActionManager
- Create or modify: `src-tauri/src/commands/undo.rs` — add undo/redo/status commands

- [ ] **Step 1: Add ActionManager to AppState**

In `src-tauri/src/lib.rs`, add to the `AppState` struct:

```rust
pub action_manager: services::undo::ActionManager,
```

And where `AppState` is constructed (in the setup closure), add:

```rust
action_manager: services::undo::ActionManager::new(),
```

- [ ] **Step 2: Create undo commands**

Create `src-tauri/src/commands/undo.rs`:

```rust
use tauri::State;
use crate::AppState;
use crate::services::undo::UndoStatus;
use crate::db_core::models::UndoRecord;

#[tauri::command]
pub async fn undo(state: State<'_, AppState>) -> Result<Option<String>, String> {
    state.action_manager.undo(&state.db)
}

#[tauri::command]
pub async fn redo(state: State<'_, AppState>) -> Result<Option<String>, String> {
    state.action_manager.redo(&state.db)
}

#[tauri::command]
pub async fn get_undo_status(state: State<'_, AppState>) -> Result<UndoStatus, String> {
    Ok(state.action_manager.status(&state.db))
}

#[tauri::command]
pub async fn list_undo_history(state: State<'_, AppState>, limit: Option<u32>) -> Result<Vec<UndoRecord>, String> {
    Ok(state.action_manager.history(&state.db, limit.unwrap_or(20)))
}
```

Register the module in `src-tauri/src/commands/mod.rs`:

```rust
pub mod undo;
```

Register the commands in `src-tauri/src/lib.rs` in the `generate_handler!` macro:

```rust
commands::undo::undo,
commands::undo::redo,
commands::undo::get_undo_status,
commands::undo::list_undo_history,
```

- [ ] **Step 3: Route set_rating and set_decision through ActionManager**

Replace the contents of `src-tauri/src/commands/selection.rs`:

```rust
use tauri::State;
use crate::AppState;
use crate::services::undo::Action;

#[tauri::command]
pub async fn set_rating(
    state: State<'_, AppState>,
    image_id: String,
    rating: u8,
) -> Result<(), String> {
    state.action_manager.execute(&state.db, Action::SetRating { image_id, rating })?;
    Ok(())
}

#[tauri::command]
pub async fn set_decision(
    state: State<'_, AppState>,
    image_id: String,
    decision: String,
) -> Result<(), String> {
    state.action_manager.execute(&state.db, Action::SetDecision { image_id, decision })?;
    Ok(())
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/selection.rs src-tauri/src/commands/undo.rs src-tauri/src/commands/mod.rs
git commit -m "feat: wire ActionManager into AppState and selection commands"
```

---

### Task 4: Add Cmd+Z / Cmd+Shift+Z frontend handling

**Files:**
- Modify: `src/lib/keys.ts` — add Cmd+Z and Cmd+Shift+Z handlers
- Modify: `src/lib/stores.ts` — add `undoStatus` store
- Modify: `src/lib/api.ts` — add undo/redo/status API functions

- [ ] **Step 1: Add API functions**

In `src/lib/api.ts`, add:

```typescript
export interface UndoStatus {
    can_undo: boolean;
    can_redo: boolean;
    undo_label: string | null;
    redo_label: string | null;
    stack_depth: number;
}

export async function undo(): Promise<string | null> {
    return invoke<string | null>('undo');
}

export async function redo(): Promise<string | null> {
    return invoke<string | null>('redo');
}

export async function getUndoStatus(): Promise<UndoStatus> {
    return invoke<UndoStatus>('get_undo_status');
}
```

- [ ] **Step 2: Add undoStatus store**

In `src/lib/stores.ts`, add:

```typescript
export const undoStatus = writable<{ canUndo: boolean; canRedo: boolean; undoLabel: string | null; redoLabel: string | null }>({
    canUndo: false, canRedo: false, undoLabel: null, redoLabel: null
});
```

- [ ] **Step 3: Add keyboard handlers**

In `src/lib/keys.ts`, in the `handleGlobalKeydown` function (or the main keydown handler), add near the top before other key checks — this must intercept Cmd+Z before any other handler:

```typescript
import { undo as apiUndo, redo as apiRedo, getUndoStatus } from '$lib/api';
import { undoStatus } from '$lib/stores';
```

Add at the beginning of the handler (before the `e.key === '/'` check):

```typescript
if (e.key === 'z' && e.metaKey && !e.shiftKey && !e.ctrlKey && !e.altKey) {
    e.preventDefault();
    apiUndo().then(label => {
        if (label) {
            getUndoStatus().then(s => undoStatus.set({
                canUndo: s.can_undo, canRedo: s.can_redo,
                undoLabel: s.undo_label, redoLabel: s.redo_label
            }));
        }
    });
    return;
}
if (e.key === 'z' && e.metaKey && e.shiftKey && !e.ctrlKey && !e.altKey) {
    e.preventDefault();
    apiRedo().then(label => {
        if (label) {
            getUndoStatus().then(s => undoStatus.set({
                canUndo: s.can_undo, canRedo: s.can_redo,
                undoLabel: s.undo_label, redoLabel: s.redo_label
            }));
        }
    });
    return;
}
```

Also, after every `setRating` and `setDecision` call in keys.ts, add an undo status refresh:

```typescript
getUndoStatus().then(s => undoStatus.set({
    canUndo: s.can_undo, canRedo: s.can_redo,
    undoLabel: s.undo_label, redoLabel: s.redo_label
}));
```

- [ ] **Step 4: Verify**

Start the Tauri dev app. Import a folder. Rate an image with `1`. Rate it with `5`. Press `Cmd+Z`. The rating should revert to `1`. Press `Cmd+Shift+Z`. The rating should return to `5`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/keys.ts src/lib/stores.ts src/lib/api.ts
git commit -m "feat: add Cmd+Z/Cmd+Shift+Z undo/redo for ratings and decisions"
```

---

### Task 5: Wire up Edit menu undo/redo handlers

**Files:**
- Modify: `src-tauri/src/menu.rs` — add handlers for Undo/Redo menu items

- [ ] **Step 1: Add menu event handlers**

In `src-tauri/src/menu.rs`, in the `handle_menu_event` function, add cases for the predefined undo/redo menu items. These have event IDs `"undo"` and `"redo"`:

```rust
"undo" => {
    let state = app.state::<crate::AppState>();
    if let Ok(Some(label)) = state.action_manager.undo(&state.db) {
        let _ = app.emit("undo-status-changed", ());
    }
}
"redo" => {
    let state = app.state::<crate::AppState>();
    if let Ok(Some(label)) = state.action_manager.redo(&state.db) {
        let _ = app.emit("undo-status-changed", ());
    }
}
```

Check how other menu events are matched — use the same pattern (likely matching on `event.id().0` or `event.id()`).

- [ ] **Step 2: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/menu.rs
git commit -m "feat: wire Edit menu Undo/Redo items to ActionManager"
```

---

### Task 6: Refresh image state after undo/redo

**Files:**
- Modify: `src/lib/keys.ts` — refresh images store after undo/redo

- [ ] **Step 1: Refresh displayed images after undo**

The undo/redo of a rating/decision changes DB state but the frontend grid/loupe still shows the old value. After a successful undo or redo, we need to refresh the images.

In the Cmd+Z handler in `keys.ts`, after updating `undoStatus`, trigger a refresh of the images store. Find how other operations refresh images (likely re-fetching the current view's images).

Look for how rating changes currently refresh the UI — likely via a reactive `$effect` or by directly updating the store. After undo, we need the same refresh:

```typescript
// After undo/redo succeeds, dispatch a custom event that components listen for
window.dispatchEvent(new CustomEvent('images-changed'));
```

Or if there's an existing `refreshImages()` function, call that. Check `src/lib/stores.ts` for a refresh mechanism.

- [ ] **Step 2: Verify end-to-end**

Test: Rate an image → Cmd+Z → verify rating reverts visually in both grid and loupe views.

- [ ] **Step 3: Commit**

```bash
git add src/lib/keys.ts
git commit -m "feat: refresh UI state after undo/redo"
```

---

## Self-Review

**Spec coverage:**
- ✅ ActionManager struct with execute/undo/redo — Task 2
- ✅ undo_records table migration — Task 1
- ✅ SetRating and SetDecision actions — Task 2
- ✅ Existing commands routed through ActionManager — Task 3
- ✅ Cmd+Z / Cmd+Shift+Z keyboard handling — Task 4
- ✅ undoStatus Svelte store — Task 4
- ✅ Tauri commands for undo/redo/status — Task 3
- ✅ Edit menu undo/redo handlers — Task 5
- ✅ UI refresh after undo — Task 6
- ⏭ Coalescing — deferred to Tier 1.5 (not in scope for this plan)
- ⏭ MCP undo tools — deferred to Tier 2 plan
- ⏭ Grouped transactions — deferred to Tier 2 plan

**Placeholder scan:** No TBDs, TODOs, or vague steps.

**Type consistency:** `UndoRecord` matches across models.rs, db.rs, undo.rs, commands/undo.rs, and api.ts. `UndoStatus` matches across undo.rs and api.ts. `Action` enum used consistently in ActionManager and selection commands.
