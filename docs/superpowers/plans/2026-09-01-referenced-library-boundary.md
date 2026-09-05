# Referenced Media Library Boundary Implementation Plan

> **Historical plan — implementation supersedes the SQL approach below.**
> Verified on 2026-09-05: the delivered boundary uses explicit
> `image_files.library_member` state, including a migration, and
> `NORMAL_LIBRARY_FILE_PREDICATE = "f.library_member = 1"` in
> `src-tauri/src/db_core/referenced_sources.rs`. Do not implement the anti-join
> or the no-migration assumption below. An explicitly imported file may retain
> its referenced-source association and still belong to the library. Core
> boundary work is recorded in closed issue `imageview-n0cn`; original-read
> hardening is implemented under `imageview-t6g5`. Unchecked boxes below
> are historical planning text, not current issue status.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep SD-card and external-source browsing out of permanent library scopes, discard referenced-only previews when the source goes offline, restore previews on reconnect, and replace generic missing-file failures with source-aware reconnect guidance.

**Architecture:** Continue using the existing `images`, `image_files`, `referenced_sources`, and `referenced_files` records so image identity and review selections survive reconnects. Define permanent library membership at the file-row boundary: a normal library query may use only an `image_files` row that is not linked through `referenced_files`, while the referenced-folder query continues to read linked rows directly. Treat referenced thumbnails as rebuildable cache, purge safe referenced-only candidates on an offline transition, and resolve original-action paths through one backend function that prefers an available non-referenced file before returning `Reconnect <source> to open originals`.

**Tech Stack:** Rust, rusqlite, Tauri 2 commands, SvelteKit 5, TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-08-30-external-drive-browse-in-place-design.md`

## Global Constraints

- Never delete, trash, recreate, or reset `cull.db`; migrations are unnecessary for this change.
- Originals on referenced sources must never be copied, moved, renamed, trashed, or deleted as a side effect of browsing, review, cache cleanup, or reconnect.
- Review selections and referenced image identity must survive offline transitions and reconnects.
- An image with at least one non-referenced `image_files` row remains in permanent library scopes and must resolve to that normal file for original actions.
- Referenced-folder scope and explicit user collections may continue to return referenced-only images.
- Referenced-only thumbnails are disposable derived files; cache deletion must never delete an original and must not delete a thumbnail shared with a normal library file or another online referenced source.
- All production changes follow red-green-refactor: write one behavior test, run it to observe the expected failure, implement the minimum change, and rerun the focused test.
- Run `cargo fmt` from `src-tauri/`, the full `cargo test --lib`, and `npm run preflight:full` before landing.

---

### Task 1: Define permanent library membership in SQL

**Files:**
- Modify: `src-tauri/src/db_core/referenced_sources.rs`
- Modify: `src-tauri/src/db_core/queries/images.rs`
- Modify: `src-tauri/src/db_core/queries/smart_collections.rs`
- Test: `src-tauri/src/db_core/referenced_sources.rs`

**Interfaces:**
- Produces: `pub(crate) const NORMAL_LIBRARY_FILE_PREDICATE: &str`, containing the literal anti-join predicate for an `image_files` alias named `f`.
- Consumes: existing `referenced_files(image_file_id)` relation and `idx_referenced_files_image_file` index.
- Preserves: `Database::list_images_in_referenced_folder(...)` remains unchanged and continues to query `referenced_files` directly.

- [ ] **Step 1: Write failing membership tests**

Add focused tests beside the existing referenced-source tests. The fixture must create a referenced-only image, attach it to `source-1`, and assert these literal outcomes:

```rust
assert!(db.list_images_with_visibility(20, 0, true).unwrap().is_empty());
assert_eq!(db.image_count_with_visibility(true).unwrap(), 0);
assert!(db
    .evaluate_smart_collection(r#"{"type":"rule","field":"imported_at","op":"last_n_days","value":7.0}"#)
    .unwrap()
    .is_empty());
assert_eq!(
    db.list_images_in_referenced_folder("source-1", "", true, 20, 0, true)
        .unwrap()
        .len(),
    1
);
```

Add a second test that inserts another `image_files` row for the same image at `/Pictures/kept.jpg` without a `referenced_files` link, then asserts All Images and Recent Imports each return one item whose `path` is `/Pictures/kept.jpg`.

- [ ] **Step 2: Run the focused test and verify RED**

Run from `src-tauri/`:

```bash
cargo test --lib db_core::referenced_sources::tests::referenced_only_images_are_not_permanent_library_members -- --exact
cargo test --lib db_core::referenced_sources::tests::normal_file_keeps_a_referenced_image_in_permanent_scopes -- --exact
```

Expected: the first test fails because All Images/count/smart collection include the referenced file; the second fails if the returned path is the referenced path.

- [ ] **Step 3: Add the shared normal-library predicate**

Add this file-row predicate to `db_core/referenced_sources.rs`:

```rust
pub(crate) const NORMAL_LIBRARY_FILE_PREDICATE: &str =
    "NOT EXISTS (SELECT 1 FROM referenced_files rf_library WHERE rf_library.image_file_id = f.id)";
```

Use it in `list_images_with_visibility`, `list_images_in_scope`, `list_images_filtered_with_visibility`, `list_folders_with_visibility`, `list_images_by_folder_with_visibility`, and `image_count_with_visibility`. Place it on the `f` row condition or in the `WHERE` clause before grouping so a mixed image returns its normal path.

Use the same predicate in all smart-collection count and page queries, including the counts populated by `list_smart_collections_with_visibility`. This excludes referenced-only rows from Recent Imports without changing the preset JSON.

- [ ] **Step 4: Verify GREEN and query consistency**

Run from `src-tauri/`:

```bash
cargo test --lib db_core::referenced_sources::tests
cargo test --lib db_core::db::tests
```

Expected: all referenced-source and database query tests pass, including the active referenced-folder and mixed-file assertions.

- [ ] **Step 5: Commit the membership boundary**

```bash
git add src-tauri/src/db_core/referenced_sources.rs src-tauri/src/db_core/queries/images.rs src-tauri/src/db_core/queries/smart_collections.rs
git commit -m "fix(library): exclude referenced-only media"
```

### Task 2: Make referenced thumbnails disposable and reconnectable

**Files:**
- Modify: `src-tauri/src/db_core/referenced_sources.rs`
- Modify: `src-tauri/src/services/referenced_sources.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db_core/referenced_sources.rs`
- Test: `src-tauri/src/services/referenced_sources.rs`

**Interfaces:**
- Produces: `Database::thumbnail_purge_candidates_for_offline_sources(&[String]) -> rusqlite::Result<Vec<String>>`.
- Produces: referenced registration ensures the base managed thumbnail exists after `sync_file_cancellable`, even when the sync outcome is `Unchanged` or `Restored`.
- Consumes: `MountedSourceRefresh.offline_ids` and `thumbnails::remove_thumbnails_for_image(app_data_dir, image_id)`.

- [ ] **Step 1: Write failing purge-safety tests**

Create two sources, attach `referenced-only` to the source being marked offline, attach `also-online` to both the offline source and a second online source, and give `mixed` both a referenced file and a normal file. After setting only the first source offline, assert:

```rust
assert_eq!(
    db.thumbnail_purge_candidates_for_offline_sources(&["source-1".into()])
        .unwrap(),
    vec!["referenced-only".to_string()]
);
```

The query must return distinct, sorted image IDs; it must exclude `also-online` and `mixed`.

- [ ] **Step 2: Run the purge test and verify RED**

Run from `src-tauri/`:

```bash
cargo test --lib db_core::referenced_sources::tests::offline_thumbnail_purge_keeps_normal_and_online_references -- --exact
```

Expected: compile failure because the database method does not exist.

- [ ] **Step 3: Implement safe purge candidate selection and monitor cleanup**

Implement the method with one read-only SQL query that selects images attached to the supplied offline source IDs and excludes any image that has either:

```sql
EXISTS (
  SELECT 1 FROM image_files normal
  WHERE normal.image_id = f.image_id
    AND NOT EXISTS (
      SELECT 1 FROM referenced_files normal_rf
      WHERE normal_rf.image_file_id = normal.id
    )
)
```

or a referenced file attached to a source whose `offline_at IS NULL`.

In the mounted-source monitor callback in `src-tauri/src/lib.rs`, query candidates after the refresh has persisted offline state, remove only their managed thumbnails with `remove_thumbnails_for_image`, and then emit `sources:changed`. Cache cleanup is best-effort and must not block source-state delivery.

- [ ] **Step 4: Write the failing reconnect-regeneration test**

In the referenced-source service tests, register a small real JPEG, verify its base thumbnail exists, call `remove_thumbnails_for_image`, register the unchanged path again, and assert the base thumbnail exists again.

- [ ] **Step 5: Run the reconnect test and verify RED**

Run from `src-tauri/`:

```bash
cargo test --lib services::referenced_sources::tests::unchanged_referenced_file_regenerates_a_purged_thumbnail -- --exact
```

Expected: assertion failure because an unchanged sync returns before regenerating its thumbnail.

- [ ] **Step 6: Regenerate missing referenced thumbnails**

After resolving the registered `ImageFile` in `register_referenced_paths`, check:

```rust
let thumbnail = crate::db_core::thumbnails::thumbnail_path(app_data_dir, &file.image_id);
if !thumbnail.exists() {
    crate::db_core::thumbnails::generate_thumbnail(path, app_data_dir, &file.image_id)?;
}
```

Perform this before emitting the image ID. Preserve cancellation checks and return the existing thumbnail error as a referenced-indexing error rather than silently claiming the page is usable.

- [ ] **Step 7: Verify GREEN and commit cache lifecycle**

Run from `src-tauri/`:

```bash
cargo test --lib db_core::referenced_sources::tests
cargo test --lib services::referenced_sources::tests
```

Then:

```bash
git add src-tauri/src/db_core/referenced_sources.rs src-tauri/src/services/referenced_sources.rs src-tauri/src/lib.rs
git commit -m "fix(devices): discard offline preview cache"
```

### Task 3: Resolve originals through source-aware backend policy

**Files:**
- Modify: `src-tauri/src/db_core/referenced_sources.rs`
- Modify: `src-tauri/src/commands/files.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Test: `src-tauri/src/db_core/referenced_sources.rs`
- Test: `src-tauri/src/commands/files.rs`

**Interfaces:**
- Produces: `Database::original_file_candidates(image_id: &str) -> rusqlite::Result<Vec<(String, bool)>>`, ordered with non-referenced file rows before referenced rows; the boolean is `true` for referenced rows.
- Produces: pure backend helper `resolve_image_original_path_for_db(db: &Database, image_id: &str) -> Result<String, String>` used by tests and every original-opening command.
- Produces: Tauri command `resolve_image_original_path(image_id: String) -> Result<String, String>`.
- Produces: TypeScript `resolveImageOriginalPath(imageId: string): Promise<string>`.
- Consumes: `Database::referenced_source_for_image(image_id)` for source display name and offline state.

- [ ] **Step 1: Write failing resolution tests**

Add tests for these hand-derived outcomes:

```rust
assert_eq!(resolve_image_original_path_for_db(&db, "mixed").unwrap(), local_path);
assert_eq!(
    resolve_image_original_path_for_db(&db, "offline-only").unwrap_err(),
    "Reconnect UNTITLED to open originals"
);
```

Use real temporary files for the available path. The offline-only referenced row should retain `missing_at = NULL` while its source has `offline_at` set, matching whole-volume disconnect behavior.

- [ ] **Step 2: Run resolution tests and verify RED**

Run from `src-tauri/`:

```bash
cargo test --lib commands::files::tests::original_resolution_prefers_an_available_normal_file -- --exact
cargo test --lib commands::files::tests::offline_referenced_original_names_the_source_to_reconnect -- --exact
```

Expected: compile failure because the resolver does not exist.

- [ ] **Step 3: Implement candidate ordering and the resolver command**

Query all image-file candidates ordered by:

```sql
ORDER BY (rf.image_file_id IS NOT NULL) ASC, (f.missing_at IS NOT NULL) ASC, f.id ASC
```

The resolver returns the first path that exists. If none exists and `referenced_source_for_image` returns an offline source, return exactly `Reconnect <display_name> to open originals`. Otherwise return `Image '<id>' has no available original`.

Register `resolve_image_original_path` in the Tauri invoke handler. Make `open_images_with_application` and `list_open_with_applications` call the same resolver so all Open With paths share the rule. Add the typed wrapper to `src/lib/api.ts`.

- [ ] **Step 4: Verify GREEN and command integration**

Run from `src-tauri/`:

```bash
cargo test --lib commands::files::tests
cargo test --lib db_core::referenced_sources::tests
```

Expected: resolver tests pass and existing file-action tests remain green.

- [ ] **Step 5: Commit backend original resolution**

```bash
git add src-tauri/src/db_core/referenced_sources.rs src-tauri/src/commands/files.rs src-tauri/src/lib.rs src/lib/api.ts
git commit -m "fix(files): explain offline referenced originals"
```

### Task 4: Route default-open UI through the resolver and preserve reconnect copy

**Files:**
- Modify: `src/lib/menu.ts`
- Modify: `src/lib/components/ContextMenu.svelte`
- Modify: `src/lib/menu.test.ts`
- Modify: `src/lib/components/context-menu-shortcuts.behavior.test.ts`

**Interfaces:**
- Consumes: `resolveImageOriginalPath(imageId)` from Task 3.
- Produces: `originalActionError(error: unknown): { title: string; detail?: string }` in `menu.ts` or a small shared utility if both UI entry points require identical behavior.
- Behavior: default open resolves by image ID before calling `openPath`; reconnect errors use the reconnect sentence as the toast title instead of `Open failed` or `Open With app list unavailable`.

- [ ] **Step 1: Write failing menu behavior tests**

For the default-open menu action, mock `resolveImageOriginalPath` to return `/Pictures/kept.jpg`, trigger `image_open_default`, and assert the real handler passes that returned path to the opener rather than `img.path`.

For the error path, reject with `Reconnect UNTITLED to open originals` and assert the toast receives:

```typescript
expect(showToast).toHaveBeenCalledWith(
    'Reconnect UNTITLED to open originals',
    expect.objectContaining({ type: 'warning' })
);
```

Add the equivalent context-menu assertion for loading the Open With submenu.

- [ ] **Step 2: Run UI tests and verify RED**

Run:

```bash
npm test -- src/lib/menu.test.ts src/lib/components/context-menu-shortcuts.behavior.test.ts
```

Expected: failures because default open still uses `img.path` and reconnect errors are wrapped in generic toast titles.

- [ ] **Step 3: Implement resolver-backed default open and reconnect error formatting**

Import `resolveImageOriginalPath` in both entry points. Resolve by image ID before `openPath`. When a caught error string starts with `Reconnect `, show it as a warning title; retain the current generic title/detail behavior for all other errors.

Do not disable ratings, decisions, or collection membership for offline referenced images.

- [ ] **Step 4: Verify GREEN, format, and run the feature suite**

Run:

```bash
npm test -- src/lib/menu.test.ts src/lib/components/context-menu-shortcuts.behavior.test.ts src/lib/referenced-sources.test.ts src/lib/components/devices-section.behavior.test.ts
cd src-tauri && cargo fmt --all -- --check
cd src-tauri && cargo test --lib
npm run check
```

Expected: all commands exit zero with no Svelte errors or warnings.

- [ ] **Step 5: Commit UI reconnect guidance**

```bash
git add src/lib/menu.ts src/lib/components/ContextMenu.svelte src/lib/menu.test.ts src/lib/components/context-menu-shortcuts.behavior.test.ts
git commit -m "fix(ui): guide offline source reconnection"
```

### Task 5: Final review, project gates, issue closure, and landing

**Files:**
- Verify: all files changed by Tasks 1–4
- Update: `.beads/issues.jsonl` through `npm run bd -- ...`

**Interfaces:**
- Consumes: issue `imageview-n0cn`, feature branch `codex/referenced-library-boundary`, and repository landing script.
- Produces: reviewed, tested, pushed pull request merged through the repository's fail-closed feature landing flow.

- [ ] **Step 1: Review the full branch diff against the spec**

Inspect:

```bash
git log --oneline origin/main..HEAD
git diff --stat origin/main...HEAD
git diff --check origin/main...HEAD
```

Confirm every acceptance criterion has a direct automated test and no query used by active referenced-folder scope gained the normal-library predicate.

- [ ] **Step 2: Run the full preflight with fresh evidence**

Run:

```bash
npm run preflight:full
```

Expected: frontend checks/tests, Rust formatting, Clippy, and the full Rust test suite all pass.

- [ ] **Step 3: Close and persist the bd issue**

```bash
npm run bd -- close imageview-n0cn --reason "Referenced-only media is excluded from permanent scopes, offline previews are disposable and regenerate, and original actions provide reconnect guidance."
npm run bd -- vc status
npm run bd -- vc commit -m "Close imageview-n0cn"
```

If `vc commit` reports there is nothing to commit, retain the tracked `.beads/issues.jsonl` update and continue.

- [ ] **Step 4: Land through the repository feature flow**

From the primary repository checkout, run:

```bash
npm run land:feature -- codex/referenced-library-boundary
```

Expected: the script verifies the scoped commits and diff, runs full preflight, pushes only the feature branch, creates or updates the PR, waits for required GitHub checks, merges through GitHub, fast-forwards local main, and removes the merged remote feature branch.

- [ ] **Step 5: Verify final state**

Run:

```bash
git status --short --branch
git branch --contains HEAD
npm run bd -- show imageview-n0cn
```

Report the merged commit/PR, exact test evidence, any retained worktree, and any follow-up issue created during review.
