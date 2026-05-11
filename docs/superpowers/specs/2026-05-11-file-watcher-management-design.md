# File Watcher + In-App File Management

## Problem
Images deleted, moved, or renamed outside the app leave broken references in all views. No mechanism exists to detect filesystem changes or manage files from within the app.

## Existing Schema (already supports this)
- `images` table: UUID `id`, `sha256_hash`, dimensions, metadata
- `image_files` table: UUID `id`, `image_id` FK, `path`, `last_seen_at`, `missing_at`
- All ratings/tags/collections reference `images.id` — location-independent

## Design

### 1. File Watcher (Rust, `notify` crate)

**Setup:**
- Add `notify = "6"` to Cargo.toml
- Add `FileWatcher` service to `AppState`, started on app launch
- Watch all library root folders (new `library_roots` table or app settings)
- Use `notify`'s `RecommendedWatcher` with debounced events

**Events handled:**
- `Remove` → call `mark_file_missing(path)` — sets `missing_at = NOW()`
- `Create` → cheap prefilter (extension + file size), then check DB:
  - If path matches a missing `image_files` row → clear `missing_at` (file restored)
  - If sha256 matches an existing image → call `restore_or_move_file_by_hash()` (external move detected)
  - Otherwise → trigger normal import for the new file
- `Rename(old, new)` → treat as Remove(old) + Create(new), let the logic above reconcile

**Emit:** Tauri event `images:changed` after any mutation so frontend re-fetches.

### 2. Move Intent Registry

Prevents feedback loops when the app itself moves files.

**Structure:** `DashMap<PathBuf, MoveIntent>` where:
```rust
struct MoveIntent {
    old_path: PathBuf,
    new_path: PathBuf,
    image_file_id: String,
    registered_at: Instant, // monotonic
}
```

**Flow:**
1. In-app move registers intent
2. Watcher event arrives → check registry by canonical path
3. If match → no-op (app already handled it)
4. Entries expire after **60 seconds** (macOS FSEvents coalesces/delays)
5. Cleanup is opportunistic (on next event check), not timer-based

**Watcher handling is also idempotent:** even without the registry, it checks DB + filesystem state before mutating. The registry is an optimization, not the correctness mechanism.

### 3. In-App File Operations (new Tauri commands)

**Order: filesystem first, DB second.**

#### `move_image(image_id, destination_folder)`
1. Validate destination exists and is within a library root
2. Register move intent
3. `std::fs::rename(old_path, new_path)`
4. Update `image_files.path` + `last_seen_at` in DB
5. If DB update fails → compensating rename back, surface error
6. Emit `images:changed`

#### `rename_image(image_id, new_name)`
Same flow as move, just changes the filename component of path.

#### `create_subfolder(parent_path, name)`
1. Validate parent is within a library root
2. `std::fs::create_dir(parent_path/name)`
3. Add new folder to watcher
4. Emit `folders:changed`

### 4. New DB Helpers

```sql
-- Mark a file as missing
UPDATE image_files SET missing_at = datetime('now') WHERE path = ?1 AND missing_at IS NULL

-- Restore a file (cleared missing)
UPDATE image_files SET missing_at = NULL, last_seen_at = datetime('now') WHERE path = ?1

-- Update path for in-app move
UPDATE image_files SET path = ?2, last_seen_at = datetime('now'), missing_at = NULL WHERE id = ?1

-- Restore/move by hash: find missing file with matching hash, update its path
UPDATE image_files SET path = ?2, missing_at = NULL, last_seen_at = datetime('now')
WHERE image_id = (SELECT id FROM images WHERE sha256_hash = ?1)
AND missing_at IS NOT NULL
LIMIT 1

-- Visible image count (excludes missing)
SELECT COUNT(DISTINCT i.id) FROM images i
JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
```

### 5. Library Roots

New concept — explicit registration of watched folders.

**Storage:** `library_roots` table or app settings JSON.
**Populated:** when user adds a folder via existing "Add Folder" flow.
**Used by:** watcher (what to watch), move validation (destination must be within a root), folder listing.

### 6. Frontend Changes

#### Types
- Add `missing_at: string | null` to `ImageWithFile` type
- Add `show_missing: boolean` to filter/query params

#### Central Event Listener
- One `images:changed` listener in `+page.svelte` root (replace current DOM `reload-images`)
- Triggers store refresh based on current folder/collection/filter
- Also refresh folder list and collection counts

#### Filter Toggle
- Add "Show missing files" toggle in filter bar
- Default: off (missing images hidden)
- When on: missing images shown with dimmed overlay + "Missing" badge

#### Image Error Handling
- Add `onerror` handler to the shared `<img>` rendering (Grid, Loupe, etc.)
- On error: show placeholder with file icon instead of browser broken image

#### Context Menu
- Add "Move to..." → folder picker submenu
- Add "Rename" → inline rename input
- Both call new Tauri commands

### 7. image_count() Fix
Update `image_count()` to join `image_files` and exclude `missing_at IS NOT NULL`, matching the pattern already used by `list_images()`.

## Out of Scope
- Auto-purge of stale missing records (separate library health feature)
- Network drive / polling fallback
- Batch move / drag-and-drop between folders (future enhancement)
- Undo for in-app moves (future, could use existing undo_records table)
