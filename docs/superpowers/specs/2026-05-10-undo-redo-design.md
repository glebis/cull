# Undo/Redo System — Design Spec

## Overview

Full undo/redo system for ImageView, covering all destructive actions from rating changes to file-level operations (crop/rotate). Backend-owned via a Rust `ActionManager` so both UI (Cmd+Z) and MCP agents share the same undo history.

## Architecture

```
UI invoke() ──→ Tauri command ──→ ActionManager.execute() ──→ DB + filesystem
MCP tool ─────→ ActionManager.execute() ──→ DB + filesystem
                      │
                      ├──→ undo_records table (SQLite)
                      └──→ undo_blobs/ (file backups for crop/rotate)
```

**Key principle:** ActionManager is the single mutation path. All existing Tauri commands (`set_rating`, `trash_images`, `crop_image`, etc.) are refactored to delegate to `ActionManager.execute()` internally. Their signatures stay the same — zero frontend changes needed.

## Data Model

### `undo_records` table

```sql
CREATE TABLE IF NOT EXISTS undo_records (
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
```

### `undo_file_backups` table

```sql
CREATE TABLE IF NOT EXISTS undo_file_backups (
    id TEXT PRIMARY KEY,
    undo_record_id TEXT NOT NULL REFERENCES undo_records(id) ON DELETE CASCADE,
    original_path TEXT NOT NULL,
    backup_path TEXT NOT NULL,
    file_hash TEXT,
    created_at TEXT NOT NULL
);
```

### Stack Mechanics

- The stack is an ordered list of `undo_records` by `created_at`
- A `stack_position` cursor tracks where we are (managed in memory, recoverable from DB)
- **Undo:** decrement position, apply `before_json` state
- **Redo:** increment position, apply `after_json` state
- **New action after undo:** delete all records above cursor (clears redo), insert new record
- **Undo-to-checkpoint:** `undo_to(record_id)` — sequentially undoes from current position back to the specified record

## ActionManager API

```rust
pub struct ActionManager {
    db: Database,
    app_data_dir: PathBuf,
    stack_position: Mutex<i64>,
    max_depth: usize,        // default 200
    max_blob_bytes: u64,     // default 5 GB
}

impl ActionManager {
    pub fn execute(&self, action: Action) -> Result<ActionResult>
    pub fn undo(&self) -> Result<Option<String>>
    pub fn redo(&self) -> Result<Option<String>>
    pub fn undo_to(&self, record_id: &str) -> Result<Vec<String>>  // returns labels of undone actions
    pub fn status(&self) -> UndoStatus
    pub fn history(&self, limit: u32) -> Vec<UndoRecord>
    pub fn clear(&self) -> Result<()>
}

pub struct UndoStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub stack_depth: usize,
}

pub struct ActionResult {
    pub undo_record_id: String,
    pub label: String,
    pub can_undo: bool,
}
```

## Action Enum

```rust
pub enum Action {
    // Tier 1: Simple state changes
    SetRating { image_id: String, rating: u8 },
    SetDecision { image_id: String, decision: String },

    // Tier 2: Data relationship changes
    TrashImages { image_ids: Vec<String> },
    DeletePermanently { image_ids: Vec<String> },
    AddToCollection { collection_id: String, image_ids: Vec<String> },
    RemoveFromCollection { collection_id: String, image_id: String },
    DeleteCollection { collection_id: String },
    DissolveLineageGroup { group_id: String },
    MergeLineageGroups { keep_id: String, merge_id: String },
    RenameLineageGroup { group_id: String, name: String },
    RemoveFromLineageGroup { image_id: String },

    // Tier 3: File-level destructive
    CropImage { image_id: String, x: u32, y: u32, w: u32, h: u32, save_as_copy: bool },
    RotateImage { image_id: String, degrees: i32 },
}
```

## Undo Logic Per Action Type

### Tier 1 — State Changes

**SetRating:** `before_json` = `{ "image_id": "x", "rating": 3 }`, `after_json` = `{ "image_id": "x", "rating": 5 }`. Undo calls `db.set_rating(image_id, before_rating)`.

**SetDecision:** Same pattern — store before/after decision string.

### Tier 2 — Relationship Changes

**TrashImages:** Before state records each image's file path. Undo moves files back from macOS trash. Note: macOS trash restore is unreliable programmatically — instead, use an app-managed trash: move files to `undo_blobs/{record_id}/` and record original paths. Undo = move back. Prune = truly delete.

**DeletePermanently:** Same as TrashImages but the user explicitly requested permanent delete. Move to `undo_blobs/` first (making it recoverable within undo window). When the undo record is pruned, the cached file is truly deleted.

**DeleteCollection:** `before_json` stores the full collection definition + all member image_ids. Undo recreates the collection and re-adds all members.

**DissolveLineageGroup:** `before_json` stores group metadata + all member image_ids with their lineage_order. Undo recreates the group and re-links images.

**MergeLineageGroups:** `before_json` stores both groups' metadata + member lists. Undo recreates the merged group and re-assigns its images.

### Tier 3 — File Operations

**CropImage / RotateImage:** Before mutation, copy original file to `undo_blobs/{record_id}/before.{ext}`. Store backup path in `undo_file_backups`. Undo = replace current file with backup. Redo = either reapply operation or restore a saved `after.{ext}` backup.

For `save_as_copy` crops: no undo needed (original untouched). But still record as an undo step so the copy creation itself can be undone (delete the copy).

## Coalescing

Rapid repeated changes to the same field collapse into one undo record.

**Rule:** If the most recent undo record has the same `action_type` + `affected_image_ids` and was created within 2 seconds, update its `after_json` instead of creating a new record. The `before_json` keeps the original "before" state, so undoing always goes back to the state before the first change.

**Applies to:** `SetRating`, `SetDecision`, `RenameLineageGroup`.

**Does not apply to:** Destructive actions (trash, delete, dissolve, crop, rotate).

## Grouped Transactions

Multi-select operations (e.g., rating 10 images at once, trashing 5 images) undo as one step.

When the UI or MCP batches multiple actions, they share a `group_id`. Undo/redo treats all records with the same `group_id` as one atomic step.

## Frontend Integration

### Svelte Store

```typescript
export const undoStatus = writable<{
    canUndo: boolean;
    canRedo: boolean;
    undoLabel: string | null;
    redoLabel: string | null;
}>({ canUndo: false, canRedo: false, undoLabel: null, redoLabel: null });
```

### Keyboard Shortcuts

- `Cmd+Z` → `invoke('undo')` → refresh affected UI state + update `undoStatus`
- `Cmd+Shift+Z` → `invoke('redo')` → refresh affected UI state + update `undoStatus`

### Status Refresh

After every action, undo, or redo: call `invoke('get_undo_status')` and update the store. The status bar can optionally show the undo label.

### UI Refresh After Undo

When undo/redo returns, the frontend needs to know what changed. The `ActionResult` includes `affected_image_ids` — the frontend refreshes those images' data (ratings, decisions, collection membership, etc.).

## MCP Tools

```rust
#[tool(description = "Undo the last action")]
fn undo(&self) -> String

#[tool(description = "Redo the last undone action")]
fn redo(&self) -> String

#[tool(description = "Undo all actions back to a specific point in history")]
fn undo_to(&self, record_id: String) -> String

#[tool(description = "Get current undo/redo availability and labels")]
fn get_undo_status(&self) -> String

#[tool(description = "List recent undo history with record IDs and labels")]
fn list_undo_history(&self, limit: Option<u32>) -> String
```

Every mutating MCP tool's response includes undo metadata:

```json
{
  "result": "Rating set to 5",
  "undo_record_id": "rec_abc123",
  "can_undo": true
}
```

## File Backup Management

### Storage

- Location: `{app_data_dir}/undo_blobs/{record_id}/`
- Contains: `before.{ext}` (original file), optionally `after.{ext}` (for redo)

### Budget

- Default max: 5 GB
- Configurable via settings
- Pruning: oldest file-backed undo records removed first when over budget
- Pruning never removes records still reachable by redo (above cursor)

### Startup Validation

On app launch, scan `undo_file_backups` table:
- Check backup file exists on disk
- If missing, mark the undo record as non-undoable (keep in history but skip during undo)

## Migration Path

Existing commands are migrated incrementally. Each command wraps its DB calls with ActionManager:

**Before:**
```rust
pub async fn set_rating(state: State<'_, AppState>, image_id: String, rating: u8) -> Result<(), String> {
    state.db.set_rating(&image_id, rating).map_err(|e| e.to_string())
}
```

**After:**
```rust
pub async fn set_rating(state: State<'_, AppState>, image_id: String, rating: u8) -> Result<(), String> {
    let action_mgr = state.app_handle.state::<ActionManager>();
    action_mgr.execute(Action::SetRating { image_id, rating }).map_err(|e| e.to_string())?;
    Ok(())
}
```

Commands not yet migrated continue to work without undo. Migration order:

1. **Tier 1:** SetRating, SetDecision (most common, simplest)
2. **Tier 2:** TrashImages, DeletePermanently, collection ops, lineage ops
3. **Tier 3:** CropImage, RotateImage (requires file backup infra)

## Menu Integration

The existing `PredefinedMenuItem::undo()` and `PredefinedMenuItem::redo()` in `menu.rs` need handlers added to `handle_menu_event()` that call ActionManager.

## Implementation Order

1. ActionManager struct + undo_records table + execute/undo/redo for SetRating + SetDecision (~1 session)
2. Frontend Cmd+Z/Cmd+Shift+Z wiring + undoStatus store + menu handlers (~1 session)
3. Tier 2 actions: trash, delete, collection, lineage operations (~2 sessions)
4. MCP undo/redo tools + undo metadata in mutating tool responses (~1 session)
5. Tier 3: file backup infra + crop/rotate undo (~1-2 sessions)
6. Coalescing, grouped transactions, startup validation, pruning (~1 session)
