# Implementation Plan: File Watcher + In-App File Management

Spec: `docs/superpowers/specs/2026-05-11-file-watcher-management-design.md`

## Phase 1: Backend Foundation (parallel tasks)

### Task 1A: DB Helpers + Schema
**Files:** `src-tauri/src/db_core/db.rs`
- Add `mark_file_missing(path)` — sets `missing_at = NOW()` where path matches
- Add `restore_file(path)` — clears `missing_at`, updates `last_seen_at`
- Add `update_image_file_path(id, new_path)` — updates path + last_seen_at + clears missing_at
- Add `restore_or_move_file_by_hash(sha256, new_path)` — finds missing image_files row by hash, updates path
- Fix `image_count()` to exclude missing files (join image_files, filter missing_at IS NULL)
- Add `list_images_including_missing()` variant or `show_missing` flag to existing list methods
- Add `library_roots` table: `id TEXT PK, path TEXT UNIQUE, added_at TEXT`
- Add DB helpers: `add_library_root(path)`, `remove_library_root(path)`, `list_library_roots()`

### Task 1B: File Watcher Service
**Files:** `src-tauri/Cargo.toml`, new file `src-tauri/src/watcher.rs`, `src-tauri/src/lib.rs`
- Add `notify = "6"` and `dashmap = "6"` to Cargo.toml
- Create `FileWatcher` struct:
  - `watcher: RecommendedWatcher`
  - `intent_registry: DashMap<PathBuf, MoveIntent>`
  - Methods: `start(roots)`, `stop()`, `watch_folder(path)`, `unwatch_folder(path)`
  - `register_move_intent(old, new, image_file_id)`
  - `check_and_clear_intent(path) -> Option<MoveIntent>`
- Event handler processes Remove/Create/Rename events
- Calls DB helpers, emits Tauri `images:changed` event
- Cheap prefilter on Create: check extension is image type before hashing
- Add `FileWatcher` (wrapped in Arc<Mutex>) to AppState
- Start watcher on app setup in lib.rs

### Task 1C: Tauri Commands for File Operations
**Files:** `src-tauri/src/commands/library.rs` (or new `src-tauri/src/commands/files.rs`)
- `move_image(image_id, destination_folder)` command
- `rename_image(image_id, new_name)` command
- `create_subfolder(parent_path, name)` command
- All follow: validate → register intent → fs operation → DB update → emit event
- Compensating rename on DB failure
- Register commands in lib.rs invoke_handler

## Phase 2: Frontend (after Phase 1 merges)

### Task 2A: Types, Store, Event Listener
**Files:** `src/lib/api.ts`, `src/lib/stores.ts`, `src/routes/+page.svelte`
- Add `missing_at` to ImageWithFile type
- Add `show_missing` writable store
- Add `move_image`, `rename_image`, `create_subfolder` API wrappers
- Replace DOM `reload-images` with Tauri `images:changed` listener in +page.svelte
- Refresh images, folders, collections on event

### Task 2B: UI — Filter Toggle + Missing Badge + onerror
**Files:** `src/lib/components/FilterBar.svelte` (or equivalent), `src/lib/components/Grid.svelte`, shared image component
- Add "Show missing files" toggle in filter area
- When missing shown: dimmed overlay + "Missing" badge on grid items
- Add `onerror` handler to `<img>` tags — show placeholder icon
- Apply to Grid, Loupe, Compare, Tinder, EmbeddingExplorer, Canvas, Export, LineageView

### Task 2C: Context Menu — Move/Rename
**Files:** `src/lib/components/ContextMenu.svelte`
- Add "Move to..." menu item → folder picker (list library folders)
- Add "Rename" menu item → inline input or modal
- Wire to new Tauri commands

## Build Sequence
1. Tasks 1A, 1B, 1C can run in parallel (1C depends on 1A's DB helpers being defined, but can stub)
2. Tasks 2A, 2B, 2C can run in parallel after Phase 1
3. Integration test: start app, move a file externally, verify it disappears from grid
