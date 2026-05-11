# Plan: Cmd+Z Undo for File Trash

## Goal
Wire up Cmd+Z / Cmd+Shift+Z to undo/redo file trash operations with toast confirmation.

## Architecture
Extend the existing `ActionManager` pattern (used for SetRating/SetDecision) to support TrashImage. Add keyboard shortcut in the global section of `handleKeydown()`.

## Changes

### 1. Rust: Extend Action enum (`src-tauri/src/services/undo.rs`)

Add variant to the `Action` enum (line 25):
```rust
pub enum Action {
    SetRating { image_id: String, rating: u8 },
    SetDecision { image_id: String, decision: String },
    TrashImage { image_id: String, original_path: String },
}
```

In `execute()` (line 48), add match arm for `TrashImage`:
- `before_json`: `{"image_id": id, "path": original_path}`
- `after_json`: `{"image_id": id, "path": original_path, "trashed": true}`
- `action_type`: `"trash_image"`
- `label`: `"Trash {filename}"`
- **Do NOT perform the actual trash here** — the trash is done in `library.rs`, this just records the undo entry
- Set `has_file_backup = 1` to indicate file-system action

In `apply_state()` (line 296), add `"trash_image"` arm:
- For **undo** (applying before_json): restore file from `~/.Trash/` to `original_path` using osascript:
  ```
  tell application "Finder"
    set trashItems to items of trash
    repeat with anItem in trashItems
      if name of anItem is "{filename}" then
        move anItem to POSIX file "{parent_dir}" as alias
        return
      end if
    end repeat
  end tell
  ```
- For **redo** (applying after_json where `trashed=true`): re-trash using existing osascript pattern

**Important:** `execute()` currently performs the mutation inside its own transaction. For TrashImage, the file move happens outside the DB transaction (in library.rs). We need a new method `record_action()` that only inserts the undo record without performing a mutation:

```rust
pub fn record_action(&self, db: &Database, action_type: &str, label: String,
    before_json: String, after_json: String, affected_ids: String, has_file_backup: bool
) -> Result<ActionResult, String>
```

This extracts the "insert undo record + clear redo branch" logic from `execute()` into a reusable method.

### 2. Rust: Record undo in trash_images (`src-tauri/src/commands/library.rs`)

Modify `trash_images` (line 83) to accept `State<AppState>` and call `action_manager.record_action()` for each successfully trashed file:

```rust
// After successful osascript trash (line 101):
if output.status.success() {
    trashed += 1;
    let _ = state.action_manager.record_action(
        &state.db,
        "trash_image",
        format!("Trash {}", filename),
        json!({"image_id": image_id, "path": &img.path}).to_string(),
        json!({"image_id": image_id, "path": &img.path, "trashed": true}).to_string(),
        image_id.clone(),
        true,
    );
}
```

### 3. Rust: Add restore_from_trash command (`src-tauri/src/commands/library.rs`)

New command that `apply_state` calls internally — but since `apply_state` runs in Rust, we just inline the osascript restore logic in the `apply_state` match arm for `"trash_image"`.

### 4. Frontend: Add Cmd+Z / Cmd+Shift+Z (`src/lib/keys.ts`)

In `handleKeydown()`, add before line 324 (the mode switch):

```typescript
// Undo: Cmd+Z
if (e.key === 'z' && e.metaKey && !e.shiftKey && !e.ctrlKey && !e.altKey) {
    e.preventDefault();
    const label = await invoke<string | null>('undo');
    if (label) {
        showToast(`Undone: ${label}`, { type: 'info', duration: 4000 });
        window.dispatchEvent(new CustomEvent('reload-images'));
    }
    return;
}

// Redo: Cmd+Shift+Z
if (e.key === 'z' && e.metaKey && e.shiftKey && !e.ctrlKey && !e.altKey) {
    e.preventDefault();
    const label = await invoke<string | null>('redo');
    if (label) {
        showToast(`Redone: ${label}`, { type: 'info', duration: 4000 });
        window.dispatchEvent(new CustomEvent('reload-images'));
    }
    return;
}
```

Note: `handleKeydown` is not async currently. Need to change signature or use `.then()`.

### 5. Frontend: Add undo/redo to api.ts (`src/lib/api.ts`)

```typescript
export async function undo(): Promise<string | null> {
    return invoke('undo');
}

export async function redo(): Promise<string | null> {
    return invoke('redo');
}
```

### 6. Frontend: Listen for reload-images event (`src/routes/+page.svelte`)

Add listener for `reload-images` custom event that re-fetches the image list:
```typescript
window.addEventListener('reload-images', () => loadImages());
```

## File change summary

| File | Change |
|------|--------|
| `src-tauri/src/services/undo.rs` | Add `TrashImage` variant, `record_action()` method, osascript restore in `apply_state()` |
| `src-tauri/src/commands/library.rs` | Record undo entry after successful trash |
| `src/lib/keys.ts` | Add Cmd+Z / Cmd+Shift+Z handlers |
| `src/lib/api.ts` | Add `undo()` / `redo()` wrapper functions |
| `src/routes/+page.svelte` | Add `reload-images` event listener |

## Risks
- **Trash filename collision**: If two files with the same name are trashed, the restore osascript picks the first match. Mitigated: we store the full original path and match by name — collisions are rare for image files.
- **Trash emptied**: If the user empties Trash before undoing, the restore fails silently. The undo command should return an error in this case and show an error toast.
- **async in handleKeydown**: The keydown handler is sync; we'll use fire-and-forget with `.then()` to avoid making it async (which would break preventDefault).
