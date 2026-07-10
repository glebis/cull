# Detailed Action History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a detailed, preview-rich Action History whose newest contiguous actions can be selected and undone as one stack-safe batch.

**Architecture:** Keep `UndoRecord` as the persisted source of truth, enrich it in a focused Rust history service, and expose typed Tauri responses. Keep selection and display derivation in pure TypeScript, render rows through focused Svelte components, and let the panel orchestrate loading and batch commands. Batch undo accepts a count, never arbitrary IDs, so Rust independently preserves stack order.

**Tech Stack:** Rust, rusqlite, Tauri 2 commands, Svelte 5 runes, TypeScript, Vitest, existing Cull thumbnail/asset-protocol helpers.

## Global Constraints

- The database at `~/Library/Application Support/com.glebkalinin.cull/cull.db` is read-only during verification and must never be deleted, reset, or replaced.
- `UndoRecord` remains the persistence model; no migration or destructive history rewrite is allowed.
- Batch selection is a contiguous prefix of newest undoable actions only.
- Batch undo is ordered newest-first and stops on the first failure with precise partial-completion reporting.
- Image previews use generated thumbnails and existing asset-protocol safety rules; do not broaden protocol scope.
- Raw JSON, unexplained UUIDs, and full internal paths do not appear as primary UI text.
- Use Svelte 5 runes and `onclick`/`onkeydown`, not legacy event directives.
- Use existing Tokyo Night design tokens and token-based `color-mix`; do not hardcode new colors.
- All row cells are top-aligned at desktop and narrow widths.

---

## File Map

- Create `src-tauri/src/services/undo_history.rs`: enriched history response types and record-to-view enrichment.
- Modify `src-tauri/src/services/mod.rs`: export the enrichment service.
- Modify `src-tauri/src/services/undo.rs`: batch undo result and ordered `undo_many` execution.
- Modify `src-tauri/src/commands/undo.rs`: enriched `list_undo_history` response and `undo_many` command.
- Modify `src-tauri/src/lib.rs`: register `undo_many`.
- Modify `src/lib/api.ts`: TypeScript history types and command wrappers.
- Modify `src/lib/undo-api.test.ts`: API contract coverage.
- Create `src/lib/history-view-model.ts`: pure contiguous selection, copy, activity target, and preview helpers.
- Create `src/lib/history-view-model.test.ts`: unit tests for all pure history behavior.
- Create `src/lib/components/HistoryTargetPreview.svelte`: safe preview or target glyph.
- Create `src/lib/components/HistoryRow.svelte`: accessible top-aligned row presentation.
- Modify `src/lib/components/UndoHistoryPanel.svelte`: command orchestration, batch selection, toolbar, and row composition.
- Modify `src/lib/components/undo-history-panel.test.ts`: component source contract and mutation/event coverage.
- Modify `src/lib/tauri-mock.ts`: browser-only fixtures for enriched history and `undo_many`.
- Modify `tests/e2e/run-e2e.sh`: installed/browser smoke assertions if the current History route is covered by the manual suite.

---

### Task 1: Enriched Undo History Response

**Files:**
- Create: `src-tauri/src/services/undo_history.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/commands/undo.rs`
- Test: `src-tauri/src/services/undo_history.rs` (`#[cfg(test)]` module)

**Interfaces:**
- Consumes: `Database::list_undo_records(limit)`, `Database::get_images_by_ids(ids)`, `services::library::enrich_thumbnails`, and `AppState.app_data_dir`.
- Produces: `UndoHistoryEntry`, `HistoryTarget`, `HistoryImagePreview`, and `enrich_undo_history(db, app_data_dir, limit) -> Result<Vec<UndoHistoryEntry>, String>`.

- [ ] **Step 1: Write failing enrichment tests**

Add focused tests using a migrated temporary database. Insert one image/file and record rating and decision actions. Assert:

```rust
let entries = enrich_undo_history(&db, app_dir.path(), 20).unwrap();
assert_eq!(entries[0].action_title, "Set decision");
assert_eq!(entries[0].target.display_name, "portrait.jpg");
assert_eq!(entries[0].change_summary.as_deref(), Some("Decision: undecided → accepted"));
assert_eq!(entries[0].affected_count, 1);
assert_eq!(entries[0].previews[0].image_id, image_id);
```

Add separate tests for multi-image IDs, a missing image, malformed legacy JSON, and a trash record whose path is reduced to a filename.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cd src-tauri && cargo test --lib services::undo_history::tests -- --nocapture
```

Expected: compilation fails because `undo_history` and its types do not exist.

- [ ] **Step 3: Define response types and enrichment**

Implement these serialized types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryTarget {
    pub kind: String,
    pub display_name: String,
    pub context: Option<String>,
    pub unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryImagePreview {
    pub image_id: String,
    pub display_name: String,
    pub thumbnail_path: Option<String>,
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoHistoryEntry {
    pub record: UndoRecord,
    pub action_title: String,
    pub target: HistoryTarget,
    pub change_summary: Option<String>,
    pub previews: Vec<HistoryImagePreview>,
    pub affected_count: u32,
    pub can_undo: bool,
}
```

Parse `affected_image_ids` once, fetch all images in one query, enrich thumbnail paths, index results by image ID, and preserve original record order. Derive `rating` and `decision` before/after copy from JSON; use `Unknown previous value` only when legacy payloads are malformed. For missing IDs, return `Unavailable image` plus the last eight ID characters only in accessible context, not as the primary label.

- [ ] **Step 4: Change the list command to return enriched entries**

Replace the command body with:

```rust
pub async fn list_undo_history(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<UndoHistoryEntry>, String> {
    enrich_undo_history(&state.db, &state.app_data_dir, limit.unwrap_or(20))
}
```

- [ ] **Step 5: Run enrichment tests and full Rust library tests**

Run:

```bash
cd src-tauri && cargo fmt && cargo test --lib services::undo_history::tests && cargo test --lib
```

Expected: enrichment tests pass; full library suite passes with only pre-existing warnings.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/undo_history.rs src-tauri/src/services/mod.rs src-tauri/src/commands/undo.rs
git commit -m "feat(history): enrich undo targets and previews"
```

---

### Task 2: Stack-safe Batch Undo

**Files:**
- Modify: `src-tauri/src/services/undo.rs`
- Modify: `src-tauri/src/commands/undo.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/services/undo.rs` (`#[cfg(test)]` module)

**Interfaces:**
- Consumes: existing `ActionManager::undo(&Database) -> Result<Option<String>, String>` semantics.
- Produces: `UndoManyResult { requested: u32, completed: Vec<String>, failure: Option<String> }`, `ActionManager::undo_many(&Database, u32) -> Result<UndoManyResult, String>`, and Tauri command `undo_many(count)`.

- [ ] **Step 1: Write failing batch tests**

Create three actions against a temporary database, then assert newest-first order and state:

```rust
let result = manager.undo_many(&db, 2).unwrap();
assert_eq!(result.requested, 2);
assert_eq!(result.completed, vec!["Set rating to 5", "Set rating to 4"]);
assert!(result.failure.is_none());
assert_eq!(db.get_selection_for_image(&image_id).unwrap().unwrap().star_rating, Some(3));
assert!(manager.status(&db).can_redo);
```

Add tests for `count == 0`, count greater than currently undoable depth, and a malformed newest record that causes zero completions and a populated failure.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cd src-tauri && cargo test --lib services::undo::tests::undo_many -- --nocapture
```

Expected: compilation fails because `undo_many` is missing.

- [ ] **Step 3: Implement batch result and ordered execution**

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UndoManyResult {
    pub requested: u32,
    pub completed: Vec<String>,
    pub failure: Option<String>,
}
```

Validate `count >= 1` and `count <= currently_undoable_count(db)`. Loop exactly `count` times, call the existing internal single-step undo path, push each label, and stop on the first error. Return a transport-level `Err` only for invalid input or inability to inspect the stack; action application failures belong in `failure` so partial completion reaches the UI.

- [ ] **Step 4: Register the command**

Add:

```rust
#[tauri::command]
pub async fn undo_many(
    state: State<'_, AppState>,
    count: u32,
) -> Result<UndoManyResult, String> {
    state.action_manager.undo_many(&state.db, count)
}
```

Register `commands::undo::undo_many` beside the existing undo commands in `src-tauri/src/lib.rs`.

- [ ] **Step 5: Run tests and formatting**

Run:

```bash
cd src-tauri && cargo fmt && cargo test --lib services::undo::tests && cargo test --lib
```

Expected: all new and existing Rust tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/undo.rs src-tauri/src/commands/undo.rs src-tauri/src/lib.rs
git commit -m "feat(history): add stack-safe batch undo"
```

---

### Task 3: Typed API and Pure History View Model

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/lib/undo-api.test.ts`
- Create: `src/lib/history-view-model.ts`
- Create: `src/lib/history-view-model.test.ts`

**Interfaces:**
- Consumes: serialized Rust types from Tasks 1 and 2; `safeAssetPreviewPath` from `src/lib/view-utils.ts`.
- Produces: `UndoHistoryEntry`, `UndoManyResult`, `undoMany(count)`, `selectedPrefixLength(current, clickedIndex)`, `historyPreviewPath(entry)`, and `activityHistoryRow(event)`.

- [ ] **Step 1: Write failing API wrapper tests**

Update the history fixture to the enriched nested shape and add:

```ts
invokeMock.mockResolvedValueOnce({ requested: 3, completed: ['A', 'B', 'C'], failure: null });
await expect(undoMany(3)).resolves.toEqual({ requested: 3, completed: ['A', 'B', 'C'], failure: null });
expect(invokeMock).toHaveBeenCalledWith('undo_many', { count: 3 });
```

- [ ] **Step 2: Run API tests and verify RED**

Run:

```bash
npm test -- --run src/lib/undo-api.test.ts
```

Expected: FAIL because enriched types and `undoMany` are missing.

- [ ] **Step 3: Add typed interfaces and wrapper**

Mirror the Rust field names exactly and change `listUndoHistory` to `Promise<UndoHistoryEntry[]>`. Add:

```ts
export async function undoMany(count: number): Promise<UndoManyResult> {
    return invoke<UndoManyResult>('undo_many', { count });
}
```

- [ ] **Step 4: Write failing pure view-model tests**

Cover contiguous selection with table tests:

```ts
expect(selectedPrefixLength(0, 0)).toBe(1);
expect(selectedPrefixLength(1, 2)).toBe(3);
expect(selectedPrefixLength(3, 1)).toBe(1);
expect(selectedPrefixLength(1, 0)).toBe(0);
```

Also assert safe thumbnail selection, `+N` count derivation, missing-image fallback, import copy, canvas copy that hides UUIDs, deleted collection fallback, and 24-hour date formatting.

- [ ] **Step 5: Run view-model tests and verify RED**

Run:

```bash
npm test -- --run src/lib/history-view-model.test.ts
```

Expected: compilation fails because the module does not exist.

- [ ] **Step 6: Implement pure helpers**

Implement deterministic helpers without Svelte state or Tauri calls. `activityHistoryRow` must use payload names when present and otherwise produce phrases such as `Deleted collection · no longer available` or `Canvas updated`, never `subject_id` as the primary label.

- [ ] **Step 7: Run focused and full frontend tests**

Run:

```bash
npm test -- --run src/lib/undo-api.test.ts src/lib/history-view-model.test.ts
npm test -- --run
```

Expected: both focused files and all frontend tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/lib/api.ts src/lib/undo-api.test.ts src/lib/history-view-model.ts src/lib/history-view-model.test.ts
git commit -m "feat(history): add typed batch history view model"
```

---

### Task 4: Preview-rich Accessible History Rows

**Files:**
- Create: `src/lib/components/HistoryTargetPreview.svelte`
- Create: `src/lib/components/HistoryRow.svelte`
- Modify: `src/lib/components/UndoHistoryPanel.svelte`
- Modify: `src/lib/components/undo-history-panel.test.ts`
- Modify: `src/lib/tauri-mock.ts`

**Interfaces:**
- Consumes: `UndoHistoryEntry`, pure helpers, `undoMany`, `convertFileSrc`, and existing toast/reload events.
- Produces: reusable target preview and row components plus batch-selection orchestration in the panel.

- [ ] **Step 1: Write failing component contract tests**

Extend the contract test to require:

```ts
expect(panel).toContain('Undo {selectedCount} actions');
expect(panel).toContain('undoMany(selectedCount)');
expect(panel).toContain('aria-live="polite"');
expect(panel).toContain('<HistoryRow');
expect(historyRow).toContain('align-items: start;');
expect(historyRow).toContain('.history-row:hover');
expect(historyRow).toContain('.history-row:focus-within');
expect(historyRow).toContain('.history-row.selected');
expect(historyPreview).toContain('safeAssetPreviewPath');
expect(historyPreview).toContain('convertFileSrc');
```

Also require that narrow-width CSS retains preview, count, and timestamp rather than hiding them.

- [ ] **Step 2: Run component tests and verify RED**

Run:

```bash
npm test -- --run src/lib/components/undo-history-panel.test.ts
```

Expected: FAIL because the new components and batch UI do not exist.

- [ ] **Step 3: Implement target preview component**

Accept `previews`, `targetKind`, and `targetName`. Use `safeAssetPreviewPath` before `convertFileSrc`; render a 56 px image with alt text when safe, a target glyph with `aria-hidden="true"` otherwise, and `+N` when more than one preview exists. Use `onerror` to switch to the placeholder without exposing the original path.

- [ ] **Step 4: Implement accessible history row**

Accept row view data, `selectable`, `selected`, `busy`, and `onselect`. Render a native checkbox whose label contains action and target context. Use a grid whose cells all have `align-self: start`; set the row itself to `align-items: start`. Add token-based hover, focus-within, selected, and busy styles plus a reduced-motion override.

- [ ] **Step 5: Refactor the panel orchestration**

Replace inline row markup with `HistoryRow`. Track `selectedCount` as the contiguous prefix length. When selected, render `Undo {selectedCount} actions` and `Clear selection`; otherwise keep single Undo/Redo. On batch completion, show exact completed/requested copy, clear selection, reload history, and dispatch `reload-images`.

- [ ] **Step 6: Update browser-only mock fixtures**

Return enriched image and non-image history fixtures and a deterministic `undo_many` response from `src/lib/tauri-mock.ts`. Keep this file browser-test-only; do not import it from `api.ts` or app components.

- [ ] **Step 7: Run component, check, and full frontend suite**

Run:

```bash
npm test -- --run src/lib/components/undo-history-panel.test.ts src/lib/history-view-model.test.ts
npm run check
npm test -- --run
```

Expected: zero Svelte diagnostics and all frontend tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/lib/components/HistoryTargetPreview.svelte src/lib/components/HistoryRow.svelte src/lib/components/UndoHistoryPanel.svelte src/lib/components/undo-history-panel.test.ts src/lib/tauri-mock.ts
git commit -m "feat(history): add detailed preview-rich batch UI"
```

---

### Task 5: Integration, Installed-app Verification, and Landing

**Files:**
- Modify only if needed after verification: files from Tasks 1–4 and `tests/e2e/run-e2e.sh`.
- Verify: `/Applications/Cull.app` using the real local database.

**Interfaces:**
- Consumes: complete backend/frontend feature.
- Produces: verified installed application, pushed branch, and evidence for every acceptance criterion.

- [ ] **Step 1: Run the full project gate**

Run:

```bash
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CULL_PREFLIGHT_SKIP_E2E=1 npm run preflight -- full
```

Expected: shell contracts, Svelte check, all frontend tests, Rust formatting, Clippy, and all Rust targets pass. Pre-existing non-fatal Clippy warnings may remain.

- [ ] **Step 2: Build the macOS app bundle**

Run:

```bash
npm run tauri build -- --bundles app
```

Expected: `src-tauri/target/release/bundle/macos/Cull.app` exists.

- [ ] **Step 3: Reinstall and restart safely**

Run:

```bash
osascript -e 'tell application "Cull" to quit' 2>/dev/null || true
sleep 2
trash /Applications/Cull.app
ditto --rsrc src-tauri/target/release/bundle/macos/Cull.app /Applications/Cull.app
open -a /Applications/Cull.app
```

Expected: one main `/Applications/Cull.app/Contents/MacOS/cull` process is running and its SHA-256 matches the built executable.

- [ ] **Step 4: Verify the live UI with real data**

Open Action History and capture evidence for:

- filename and before → after copy on image rows;
- thumbnail or explicit missing-image placeholder;
- visible collection/canvas/import target text without UUID-primary labels;
- hover border/surface change;
- keyboard focus indicator;
- top-aligned multi-line text;
- contiguous three-row selection and `Undo 3 actions` toolbar;
- successful batch result and updated image state;
- individual redo restoring the original state;
- narrow-width layout retaining preview, count, and timestamp.

If mutating real curation state for verification, record the three initial values first and restore them through redo before finishing.

- [ ] **Step 5: Add or refine E2E coverage if a gap is found**

If the manual browser suite can exercise the mock panel, assert row preview, target copy, prefix selection, batch button label, and hover/selected CSS state in `tests/e2e/run-e2e.sh`. Run:

```bash
bash tests/e2e/run-e2e.sh
```

Expected: manual smoke suite passes; if prerequisites are unavailable, retain the installed-app Computer Use verification as evidence and document the missing browser prerequisite.

- [ ] **Step 6: Commit verification-driven fixes**

```bash
git add <only-files-changed-by-verification>
git commit -m "test(history): cover detailed batch timeline"
```

Skip this commit only when verification required no file changes.

- [ ] **Step 7: Land and push**

Run:

```bash
npm run land
```

Expected: clean worktree, full checks pass, branch rebases cleanly, and push succeeds. If the full gate has just passed with disk-saving Cargo flags, use the repository-supported skip-check landing rerun only after recording that evidence.

- [ ] **Step 8: Completion audit**

Check each acceptance criterion in `docs/superpowers/specs/2026-07-10-detailed-action-history-design.md` against source, automated test output, installed-app screenshots, and git remote state. Do not mark the goal complete until all nine criteria have direct evidence.

