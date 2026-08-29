# External Drive Browse-in-Place Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users plug in an SD card or external drive, browse it from the top of Cull's sidebar, and persist review decisions without copying or modifying originals.

**Architecture:** Add stable referenced-source records beside the existing `images` and `image_files` catalog, discover mounted sources through an injected platform provider, and progressively register only requested pages through the existing image pipeline. The frontend adds a `referenced_folder` scope and focused Devices/toolbar components while continuing to feed `ImageWithFile` into Grid, Loupe, Compare, collections, and selections.

**Tech Stack:** Tauri 2, Rust, rusqlite, objc2 AppKit/Foundation on macOS, notify watcher, SvelteKit 5 runes, Svelte stores, Vitest, Playwright E2E mock.

**Spec:** `docs/superpowers/specs/2026-08-30-external-drive-browse-in-place-design.md`

## Global Constraints

- Originals remain on the source; browsing, rating, rejecting, and removing a source from Cull must never copy, move, rename, trash, or delete them.
- Preserve the existing `cull.db`; migrate with `Database::open()` and never reset or recreate it.
- Reuse `ImageWithFile` and the existing selections/collections/search surfaces; do not introduce a parallel external-image UI model.
- `src/lib/api.ts` continues to invoke the real Tauri backend; `tauri-mock.ts` remains E2E-only.
- Use existing supported image/RAW formats only; do not add model weights, decoders, or third-party dependencies.
- Use Svelte 5 runes and existing Tokyo Night design tokens; do not hardcode colours.
- Every destructive guard is enforced in Rust, not only hidden in the frontend.
- Keep the current Apple Photos and explicit CLI/MCP import flows compatible.
- Preserve unrelated dirty files and commit only exact paths owned by each task.

---

## File Map

**New Rust units**

- `src-tauri/src/db_core/referenced_sources.rs` — source/file persistence, reconnection, protection lookup, and removal transaction.
- `src-tauri/src/mounted_sources.rs` — platform-neutral provider trait plus macOS and fallback implementations.
- `src-tauri/src/services/referenced_sources.rs` — folder discovery, paging, progressive registration, cancellation, and service result types.
- `src-tauri/src/commands/referenced_sources.rs` — narrow Tauri command boundary and event emission.

**New frontend units**

- `src/lib/referenced-sources.ts` — source stores, refresh/open/cancel orchestration, and event subscriptions.
- `src/lib/components/DevicesSection.svelte` — mounted/remembered source tree only.
- `src/lib/components/ReferencedSourceToolbar.svelte` — breadcrumb, recursion control, status, and reconnect copy.

**Existing integration files**

- `src-tauri/src/db_core/db.rs`, `schema.sql`, `models.rs`, `mod.rs` — migration 27 and exported models.
- `src-tauri/src/lib.rs`, `commands/mod.rs`, `services/mod.rs` — state, command registration, source monitor startup.
- `src-tauri/src/commands/library.rs`, `commands/files.rs` — backend destructive-operation guards.
- `src/lib/api.ts`, `stores.ts`, `library-scope.ts`, `image-loading.ts` — typed API and referenced scope.
- `src/lib/components/Sidebar.svelte`, `src/routes/+page.svelte`, `src/lib/deeplink.ts` — compose Devices, toolbar, events, and repaired drag/drop semantics.
- `src/lib/tauri-mock.ts`, `tests/e2e/smoke.py` — browser-only mount/unmount and source paging fixtures.

---

### Task 1: Persistent Referenced Source Identity

**Files:**
- Create: `src-tauri/src/db_core/referenced_sources.rs`
- Modify: `src-tauri/src/db_core/db.rs`
- Modify: `src-tauri/src/db_core/schema.sql`
- Modify: `src-tauri/src/db_core/models.rs`
- Modify: `src-tauri/src/db_core/mod.rs`
- Test: `src-tauri/src/db_core/referenced_sources.rs`

**Interfaces:**
- Produces: `ReferencedSource`, `ReferencedFile`, `ReferencedSourceKind`, `Database::upsert_referenced_source`, `Database::list_referenced_sources`, `Database::attach_referenced_file`, `Database::reconnect_referenced_source`, `Database::referenced_source_for_image`, and `Database::remove_referenced_source`.
- Consumes: existing `image_files`, `images`, thumbnail removal helpers, and migration framework.

- [ ] **Step 1: Write migration and round-trip tests**

Add tests that open a full version-26 fixture, rerun `Database::open`, and assert both tables and indexes exist. Add a round-trip test using these exact values:

```rust
let source = ReferencedSource {
    id: "source-1".into(),
    platform_volume_id: Some("volume-uuid-1".into()),
    display_name: "UNTITLED".into(),
    last_mount_path: Some("/Volumes/UNTITLED".into()),
    source_kind: ReferencedSourceKind::SdCard,
    capacity_bytes: Some(64_000_000_000),
    recursive_default: false,
    settings_json: "{}".into(),
    last_seen_at: "2026-08-30T10:00:00Z".into(),
    offline_at: None,
};
db.upsert_referenced_source(&source).unwrap();
assert_eq!(db.list_referenced_sources().unwrap(), vec![source]);
```

Add a reconnection test that attaches `DCIM/100CANON/IMG_0001.JPG`, changes the mount from `/Volumes/UNTITLED` to `/Volumes/UNTITLED 1`, and asserts `image_files.path` updates while the image ID and selection remain unchanged.

- [ ] **Step 2: Run the focused tests and verify failure**

Run: `cd src-tauri && cargo test --lib db_core::referenced_sources::tests -- --nocapture`

Expected: FAIL because the module, models, migration, and methods do not exist.

- [ ] **Step 3: Add migration 27 and models**

Set `CURRENT_SCHEMA_VERSION` to `27`, append `(27, "referenced_sources")`, add both tables and indexes from the approved spec to `schema.sql`, and add a `referenced_sources_schema()` migration closure. Add `capacity_bytes INTEGER` to `referenced_sources`; serialize it through `Option<u64>` using the existing checked integer helpers.

Use these enums and structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferencedSourceKind { SdCard, ExternalDrive, MountedVolume, Folder }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferencedSource {
    pub id: String,
    pub platform_volume_id: Option<String>,
    pub display_name: String,
    pub last_mount_path: Option<String>,
    pub source_kind: ReferencedSourceKind,
    pub capacity_bytes: Option<u64>,
    pub recursive_default: bool,
    pub settings_json: String,
    pub last_seen_at: String,
    pub offline_at: Option<String>,
}
```

- [ ] **Step 4: Implement atomic persistence and removal**

`attach_referenced_file` takes `(source_id, image_file_id, relative_path)` and rejects absolute paths or `..` components. `reconnect_referenced_source` runs one transaction that verifies the stored platform ID, rewrites every linked `image_files.path` using the new mount plus stored relative path, clears `offline_at`, and updates `last_mount_path`.

`remove_referenced_source` returns the orphaned image IDs after deleting only source-linked `image_files`; it deletes an `images` row only when no other `image_files` row references it. Call thumbnail cleanup after the transaction for those orphan IDs.

- [ ] **Step 5: Run migration and database suites**

Run: `cd src-tauri && cargo test --lib db_core::referenced_sources::tests -- --nocapture`

Run: `cd src-tauri && cargo test --lib migration_safety -- --nocapture`

Expected: PASS, including schema invariants with both new tables in `REQUIRED_TABLES`.

- [ ] **Step 6: Format and commit**

Run: `cd src-tauri && cargo fmt`

Commit exact Task 1 files with: `git commit -m "feat(sources): persist referenced source identity"`

---

### Task 2: Mounted Source Discovery and Lifecycle

**Files:**
- Create: `src-tauri/src/mounted_sources.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/mounted_sources.rs`

**Interfaces:**
- Consumes: `Database::upsert_referenced_source` and `Database::list_referenced_sources` from Task 1.
- Produces: `MountedSourceProvider`, `MountedSource`, `MountedSourceMonitor`, `MountedSourceKind`, and `refresh_mounted_sources(db, provider)`.

- [ ] **Step 1: Write provider reconciliation tests**

Use a fake provider whose result can be replaced between calls. Cover:

```rust
assert_eq!(refresh_mounted_sources(&db, &fake_with_sd()).unwrap().online.len(), 1);
fake.set(Vec::new());
let refresh = refresh_mounted_sources(&db, &fake).unwrap();
assert_eq!(refresh.offline_ids, vec![source_id.clone()]);
```

Also test same mount path/different volume ID creates a second source, while same volume ID/new path reconnects the first source.

- [ ] **Step 2: Run the tests and verify failure**

Run: `cd src-tauri && cargo test --lib mounted_sources::tests -- --nocapture`

Expected: FAIL because the provider and reconciliation functions do not exist.

- [ ] **Step 3: Implement the provider boundary**

Define:

```rust
pub trait MountedSourceProvider: Send + Sync {
    fn list_mounted_sources(&self) -> Result<Vec<MountedSource>, String>;
}

pub struct MountedSource {
    pub platform_volume_id: Option<String>,
    pub display_name: String,
    pub mount_path: PathBuf,
    pub kind: ReferencedSourceKind,
    pub capacity_bytes: Option<u64>,
    pub writable: bool,
}
```

The macOS provider enumerates mounted volume URLs through Foundation/AppKit metadata, excludes `/`, hidden/system/recovery mounts, and classifies as `SdCard` only when removable/ejectable metadata is affirmative. Uncertain devices use `MountedVolume`; classification must not depend only on the volume name.

Use existing objc2 crates. Add only required feature flags to the existing dependencies; do not add a new crate.

- [ ] **Step 4: Add a debounced lifecycle monitor**

`MountedSourceMonitor` owns a cancellation flag and background listener/poller. It calls reconciliation at startup and on mount/unmount notifications, then emits `sources:changed` once per settled refresh. The non-macOS implementation exposes manually remembered folders and an empty automatic list without failing compilation.

Store the monitor in `AppState` so it lives for the app lifetime and stops on drop.

- [ ] **Step 5: Run tests and compile all targets**

Run: `cd src-tauri && cargo test --lib mounted_sources::tests -- --nocapture`

Run: `cd src-tauri && cargo check --all-targets`

Expected: PASS with no new dependency.

- [ ] **Step 6: Format and commit**

Commit exact Task 2 files with: `git commit -m "feat(sources): discover mounted external volumes"`

---

### Task 3: Progressive Browse-in-Place Service and Commands

**Files:**
- Create: `src-tauri/src/services/referenced_sources.rs`
- Create: `src-tauri/src/commands/referenced_sources.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/db_core/import.rs`
- Test: `src-tauri/src/services/referenced_sources.rs`
- Test: `src-tauri/src/commands/referenced_sources.rs`

**Interfaces:**
- Consumes: mounted source resolution and Task 1 database methods.
- Produces: `list_referenced_sources`, `list_source_folders`, `open_referenced_folder`, `set_source_recursive_default`, `remove_referenced_source`, `cancel_referenced_source_job`; emits `referenced-source:page-updated`.

- [ ] **Step 1: Write discovery, paging, and cancellation tests**

Create a temporary source containing direct images, a nested directory, an unsupported text file, and 300 supported fixtures. Assert:

```rust
let page = service.open_folder(OpenReferencedFolder {
    source_id: source.id.clone(),
    relative_path: "DCIM".into(),
    recursive: false,
    cursor: None,
    limit: 50,
}).unwrap();
assert_eq!(page.requested_paths.len(), 50);
assert!(page.next_cursor.is_some());
assert!(page.requested_paths.iter().all(|p| !p.contains("nested")));
```

Add a recursive counterpart, a sorted cursor continuation, a `..` rejection, disconnect during work, and cancellation after the first batch.

- [ ] **Step 2: Run focused tests and verify failure**

Run: `cd src-tauri && cargo test --lib services::referenced_sources::tests -- --nocapture`

Expected: FAIL because the service does not exist.

- [ ] **Step 3: Extract reusable per-file registration**

Expose the existing cancellable per-file registration as a crate-visible function that returns the canonical image ID and outcome without scheduling post-import batch work. Preserve hash, decode, thumbnail, metadata, and dedupe semantics. Do not alter CLI/MCP `import_folder` or `import_files` behaviour.

- [ ] **Step 4: Implement paged folder discovery**

Use a stable lexicographic relative-path cursor, clamp `limit` to `1..=250`, and keep current-folder discovery non-recursive. For recursive mode, traverse lazily and stop after enough entries to return one page plus a next cursor. Every resolved candidate must remain beneath the canonical source mount and requested folder.

The command returns immediately with:

```rust
pub struct ReferencedFolderPage {
    pub job_id: String,
    pub source_id: String,
    pub relative_path: String,
    pub image_ids: Vec<String>,
    pub discovered_count: u32,
    pub next_cursor: Option<String>,
    pub indexing: bool,
}
```

Existing records populate `image_ids` immediately. New paths run through the background job in page order and emit batches no larger than 25 IDs. A later page or folder switch cancels the prior job for that source scope.

- [ ] **Step 5: Implement commands and source removal**

Commands validate source online state before path access. `remove_referenced_source` calls the Task 1 transaction and removes orphan thumbnails from Cull app data only. Register all commands in `lib.rs`.

- [ ] **Step 6: Run focused and import regression tests**

Run: `cd src-tauri && cargo test --lib referenced_sources::tests -- --nocapture`

Run: `cd src-tauri && cargo test --lib commands::import::tests -- --nocapture`

Run: `cd src-tauri && cargo test --lib db_core::import::tests -- --nocapture`

Expected: PASS; explicit imports retain existing results.

- [ ] **Step 7: Format and commit**

Commit exact Task 3 files with: `git commit -m "feat(sources): browse folders progressively in place"`

---

### Task 4: Referenced Scope, Devices Sidebar, and Toolbar

**Files:**
- Create: `src/lib/referenced-sources.ts`
- Create: `src/lib/referenced-sources.test.ts`
- Create: `src/lib/components/DevicesSection.svelte`
- Create: `src/lib/components/devices-section.behavior.test.ts`
- Create: `src/lib/components/ReferencedSourceToolbar.svelte`
- Create: `src/lib/components/referenced-source-toolbar.behavior.test.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/library-scope.ts`
- Modify: `src/lib/image-loading.ts`
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Consumes: Task 3 commands/events.
- Produces: `referencedSources`, `activeReferencedScope`, `referencedSourceLoadState`, `refreshReferencedSources`, and the `referenced_folder` `LibraryScope` variant.

- [ ] **Step 1: Write API/store/scope tests**

Test the exact scope key:

```typescript
expect(libraryScopeKey({
  type: 'referenced_folder', source_id: 'source-1', relative_path: 'DCIM/100CANON',
  recursive: true, include_rejected: false,
})).toBe('referenced:source-1:DCIM/100CANON:recursive:without-rejected');
```

Test stale event rejection by opening folder A, switching to B, and emitting A's page event; only B remains in `images`.

- [ ] **Step 2: Run frontend tests and verify failure**

Run: `npm test -- src/lib/referenced-sources.test.ts src/lib/library-scope.test.ts`

Expected: FAIL because the stores and scope variant do not exist.

- [ ] **Step 3: Add typed API and orchestration store**

Mirror the spec's `ReferencedSource` shape in `api.ts`. `openReferencedFolder` returns `ReferencedFolderPage`; `referenced-sources.ts` owns event listeners, current request identity, cancellation, source refresh, and recursive preference updates.

`image-loading.ts` handles `referenced_folder` by requesting the current page, resolving returned IDs through `getImagesByIds`, and merging later batch events without duplicates or focus jumps.

- [ ] **Step 4: Build DevicesSection**

Render it as the first section inside `.sidebar-scroll`, before `SessionSwitcher`. Use existing `.section`, `.section-header`, `.section-item`, `.count`, and `.active` classes and design tokens. Device click opens the root; twisties call `listSourceFolders`; offline rows remain visible and announce `offline`.

The component accepts callbacks rather than importing `Sidebar.svelte` internals:

```typescript
let { sources, activeSourceId, onopen, onremove } = $props<{
  sources: ReferencedSource[];
  activeSourceId: string | null;
  onopen: (sourceId: string, relativePath: string) => void;
  onremove: (sourceId: string) => void;
}>();
```

- [ ] **Step 5: Build the referenced toolbar**

Show source breadcrumb, two-state scope control, `Originals stay on drive`, indexing progress, and offline copy. The scope control calls `setSourceRecursiveDefault` and reloads from cursor zero. Do not show this toolbar in ordinary library scopes.

- [ ] **Step 6: Run component and full frontend tests**

Run: `npm test -- src/lib/referenced-sources.test.ts src/lib/components/devices-section.behavior.test.ts src/lib/components/referenced-source-toolbar.behavior.test.ts`

Run: `npm run check && npm test`

Expected: PASS with Devices above all other sidebar content.

- [ ] **Step 7: Commit**

Commit exact Task 4 files with: `git commit -m "feat(ui): browse mounted devices from the sidebar"`

---

### Task 5: Protect Referenced Originals and Handle Offline State

**Files:**
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/commands/files.rs`
- Modify: `src-tauri/src/watcher.rs`
- Modify: `src/lib/components/ContextMenu.svelte`
- Modify: `src/lib/menu.ts`
- Modify: `src/lib/command-palette.ts`
- Test: `src-tauri/src/commands/library.rs`
- Test: `src-tauri/src/commands/files.rs`
- Test: `src-tauri/src/watcher.rs`
- Test: `src/lib/context-menu-shortcuts.behavior.test.ts`

**Interfaces:**
- Consumes: `Database::referenced_source_for_image` and source online state.
- Produces: `Database::ensure_original_mutation_allowed(image_id)` returning a user-facing error for referenced images.

- [ ] **Step 1: Write backend guard tests**

Attach a fixture image to a referenced source and call trash, permanent delete, rename, and move. Assert each returns:

```text
Originals on external sources are protected. Copy the image into a managed folder before modifying it.
```

Assert ratings, decisions, collection membership, reveal, and reads remain allowed.

- [ ] **Step 2: Run focused tests and verify failure**

Run: `cd src-tauri && cargo test --lib referenced_originals -- --nocapture`

Expected: FAIL because mutations are currently permitted.

- [ ] **Step 3: Enforce one shared guard**

Call the shared database guard before any filesystem mutation in `trash_images_detailed_with`, `delete_images_permanently`, `move_image`, and `rename_image`. Batch trash returns a per-image `protected_source` result without touching other records; permanent delete returns an error if any requested ID is protected so the operation is fail-closed.

- [ ] **Step 4: Preserve whole-source offline state**

Extend watcher tests so removing a mounted root emits `watcher:volume-offline` and leaves linked `image_files.missing_at` null. The referenced-source service translates this to `offline_at`; cached items stay queryable.

- [ ] **Step 5: Gate frontend actions**

Use source ownership data on the active item to disable Rename, Move, Trash, and Permanent Delete with the same explanation. Keep backend guards authoritative. Offline Reveal/Loupe original/Export actions show `Reconnect <device>`.

- [ ] **Step 6: Run safety suites and commit**

Run: `cd src-tauri && cargo test --lib commands::library::tests -- --nocapture`

Run: `cd src-tauri && cargo test --lib commands::files::tests -- --nocapture`

Run: `cd src-tauri && cargo test --lib watcher::tests -- --nocapture`

Run: `npm test -- src/lib/components/context-menu-shortcuts.behavior.test.ts src/lib/menu.test.ts`

Commit exact Task 5 files with: `git commit -m "fix(sources): protect referenced originals"`

---

### Task 6: Repair Drag, Drop, and Finder Open Semantics

**Files:**
- Modify: `src-tauri/src/commands/deeplink.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/deeplink.ts`
- Modify: `src/routes/+page.svelte`
- Modify: `src/lib/deeplink-integration.test.ts`
- Test: `src-tauri/src/commands/deeplink.rs`

**Interfaces:**
- Consumes: `openReferencedFolder` and source creation/resolution from Tasks 3–4.
- Produces: `OpenParams.open_mode = "review" | "import"` with drag/drop and Finder open defaulting to `review` outside Canvas.

- [ ] **Step 1: Write routing tests**

Update Rust tests so a dropped folder yields:

```rust
assert_eq!(params.open_mode.as_deref(), Some("review"));
assert_eq!(params.folder.as_deref(), Some(folder.to_str().unwrap()));
```

Frontend tests assert `handleParams` calls `openPathForReview` and does not call `importFolder` or `importFiles`. Retain a Canvas test proving its add-to-canvas flow still works.

- [ ] **Step 2: Run tests and verify failure**

Run: `cd src-tauri && cargo test --lib commands::deeplink::tests::drag_drop -- --nocapture`

Run: `npm test -- src/lib/deeplink-integration.test.ts`

Expected: FAIL because current code eagerly imports.

- [ ] **Step 3: Add explicit open mode**

Extend `OpenParams` with `open_mode`. Drag/drop and OS-open paths set `review`; explicit CLI/MCP imports do not travel through this UI navigation path and remain unchanged. In `deeplink.ts`, handle review mode before legacy folder/file import branches.

- [ ] **Step 4: Repair visible copy**

Change the global overlay from `Drop to import` to `Open to review`. Folder and file drops acknowledge navigation after the referenced scope is active, not after background registration finishes.

- [ ] **Step 5: Run tests and commit**

Run both focused suites, then `npm test -- src/lib/deeplink-integration.test.ts`.

Commit exact Task 6 files with: `git commit -m "fix(import): open dropped sources for review"`

---

### Task 7: Browser Fixtures, Native-Safe Smoke, Documentation, and Landing

**Files:**
- Modify: `src/lib/tauri-mock.ts`
- Modify: `tests/e2e/smoke.py`
- Modify: `docs/e2e-testing-policy.md`
- Modify: `CHANGELOG.md`
- Modify: `.beads/issues.jsonl`

**Interfaces:**
- Consumes: complete feature from Tasks 1–6.
- Produces: automated browser contract, native disposable-volume procedure, release note, and closed issue `imageview-xyh5`.

- [ ] **Step 1: Add mock source fixtures**

Under query parameter `?referencedSources=1`, return one online SD card, one offline remembered drive, child folders, a first page, and a later `referenced-source:page-updated` batch. Expose a mock mount/unmount event through `__CULL_E2E_EMIT__`.

- [ ] **Step 2: Add browser smoke coverage**

Add a smoke case that asserts Devices is the first sidebar section, opens `UNTITLED / DCIM`, toggles recursive mode, receives a progressive second batch, ejects the source, and verifies ratings remain while destructive actions are unavailable.

Run: `bash tests/e2e/run-e2e.sh`

Expected: PASS in Chrome Beta/CDP when prerequisites are available; if unavailable, report the exact prerequisite and retain unit-level mock coverage.

- [ ] **Step 3: Run native-safe source smoke**

Create a disposable test volume or temporary provider fixture; never use the user's real SD card. Verify mount appearance, first visible page before recursive completion, eject, cached review, same-identity reconnect, and same-path/different-identity refusal. Record the exact observed result in the issue comment.

- [ ] **Step 4: Update policy and changelog**

Document referenced-source browser coverage and native-only mount limitations in `docs/e2e-testing-policy.md`. Add an Unreleased changelog entry: `Browse and cull mounted SD cards and external drives in place without copying originals.`

- [ ] **Step 5: Run full gates**

Run: `npm run preflight -- full`

Expected: `npm run check`, all frontend tests, Rust formatting, clippy, and all Rust targets pass. Existing clippy warnings are non-blocking; no new warning or failure is accepted.

- [ ] **Step 6: Close the tracked issue and commit**

Run: `npm run bd -- close imageview-xyh5 --reason "Mounted-source browse-in-place implemented and verified"`

Export the database with: `npm run bd -- export -o .beads/issues.jsonl`

Commit exact Task 7 files with: `git commit -m "test(sources): verify external-drive review flow"`

- [ ] **Step 7: Land and verify remote state**

Run the repository feature landing flow for the implementation branch:

```bash
npm run land:feature -- codex/external-drive-browse-in-place
```

Verify GitHub reports the exact head checks passing and merge completion, local `main` fast-forwards from `origin/main`, and the remote feature branch is removed. Preserve unrelated dirty work in its original worktree.
