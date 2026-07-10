# User Data Safety Checklist

Cull features must preserve user work by default. Treat ratings, selections, canvas layout, notes, session state, app settings, generated metadata, and local files as user data once the user can change them.

## Design Checklist

- Persist user edits before navigation, scope changes, tab changes, window close, or component teardown can discard local state.
- Preserve existing stored fields and records when saving a subset view. A folder, filter, search, or current selection is not permission to delete off-screen data.
- Prefer non-destructive edits. Store transforms, crops, ordering, and annotations as reversible metadata unless the user explicitly chooses a file-level operation.
- Provide undo or a recoverable trail for destructive or bulk operations.
- Save optimistically to the in-memory app state before async persistence, then flush the same captured payload to durable storage.
- Keep failed saves visible and retryable. Do not silently clear pending user edits after an IPC, DB, or filesystem error.
- Never reset user state as a recovery strategy. If data looks wrong, investigate the code path before touching the database or files.

## Automatic Safety Actions

When adding or changing a feature that mutates user-visible state:

- Add a regression test that switches away from the edited scope and verifies the edit survives.
- Add a test for subset saves when the feature can run inside a folder, search, collection, smart collection, or filtered view.
- Audit save payloads for whole-document replacement bugs. Merge changed fields into existing persisted documents unless the user explicitly requested deletion.
- Flush pending debounced saves on lifecycle boundaries, including scope switch and teardown.
- Record follow-up issues for missing undo, missing retry, or partial-failure reporting before shipping the feature.

## Canvas-Specific Checklist

- Canvas positions, display sizes, crops, rotations, notes, groups, connectors, export intent, and viewport are all saved user data.
- Saving a canvas while a folder subset is visible must preserve canvas items from other folders.
- Canvas autosave must capture the edited canvas ID and layout JSON at edit time. It must not read the active canvas later after navigation may have changed.
