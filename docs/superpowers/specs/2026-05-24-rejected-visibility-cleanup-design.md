# Rejected Visibility and Cleanup Design

## Problem

Cull already lets users mark images as accepted, rejected, or undecided. Rejection is currently only a visible badge, so rejected images continue to appear in normal curation flows. That makes fast review less useful: once an image is rejected, the user expects it to leave the active working set while remaining recoverable and inspectable before cleanup.

Accepted images also need a clear downstream path: copy or move the accepted set into a different folder without turning the quick accept gesture into immediate file movement.

## Goals

- Treat `reject` as non-destructive metadata.
- Hide rejected images from normal curation by default.
- Add **View -> Show Rejected** as a persisted global visibility toggle.
- Keep rejected images visible in explicit rejected contexts and cleanup flows.
- Add configurable cleanup behavior for rejected images, with moving to macOS Trash as the default.
- Add batch actions for accepted images: copy accepted to a folder and move accepted to a folder.

## Non-Goals

- Do not permanently delete rejected files by default.
- Do not move or copy files at the moment an image is accepted or rejected.
- Do not hide rejected images from an explicit Rejected smart collection or cleanup confirmation.
- Do not introduce a mock Tauri API path.

## UX Model

Rejection is a curation decision, not a destructive file operation.

The default browsing model is:

1. `a` marks the focused image as accepted.
2. `x` marks the focused image as rejected.
3. Rejected images disappear from normal curation scopes when **Show Rejected** is off.
4. **View -> Show Rejected** includes rejected images again in normal scopes.
5. Cleanup/export commands operate on the current working set by default, with whole-library actions requiring an explicit scope choice.

Normal curation scopes include:

- all images
- folders
- collections
- sessions
- normal search results
- smart collections that do not explicitly request rejected images

Explicit rejected contexts include:

- the Rejected preset/smart collection
- cleanup review and confirmation dialogs
- searches or smart collections whose filter explicitly asks for rejected images

## Architecture

### Frontend State

Add a persisted `showRejected` store, defaulting to `false`.

`showRejected` is view state, similar to sidebar visibility. It should be saved in local app state and restored on launch. Toggling it reloads the current image scope without changing the current folder, collection, smart collection, or view mode.

### Native View Menu

Add a checked menu item:

- id: `view_show_rejected`
- label: `Show Rejected`
- type: `CheckMenuItem`
- default checked: `false`

The frontend menu bridge handles `view_show_rejected` by toggling `showRejected`. `update_menu_state` mirrors the current `showRejected` value into the native menu check state.

### Image Loading Filter

Image loading should exclude rejected images unless either condition is true:

- `showRejected` is true.
- the current scope explicitly targets rejected images.

The implementation should centralize this decision in the image-loading/query path rather than filtering only in the rendered component. Rendering-only filtering would break counts, paging, focus movement, and batch operations.

For smart collections and search results, the loader needs a helper that can detect whether the filter explicitly references `decision = reject` or equivalent natural-language output. If it does, rejected images must be included even when `showRejected` is false.

### Cleanup Actions

Rejected cleanup is a separate command path.

The primary entry point is the command palette. A secondary menu item can be added under the Image menu after the command behavior is proven, but Phase 2 should not depend on adding another native menu group.

Default action:

- Move rejected images to macOS Trash.
- Mark their file records missing in the database, reusing the existing trash behavior.
- Show a count before running.

Configurable future actions:

- Move rejected to Trash.
- Move rejected to a chosen `Rejected` folder.
- Hide rejected only, with no disk action.

Permanent delete should not be part of the default cleanup path.

### Accepted Batch Actions

Accepted images get explicit batch actions:

- Copy accepted to folder.
- Move accepted to folder.

The primary entry point is the command palette. Context menu entries are optional follow-up UI once the batch behavior is stable.

The default scope is the current working set. If images are selected, selected images take precedence. Whole-library export or move requires an explicit scope choice and count confirmation.

## Data Flow

Normal load:

```text
current scope + showRejected=false
    -> query images
    -> exclude decision=reject unless scope explicitly requests rejected
    -> update images/counts/focus
```

Toggle:

```text
View -> Show Rejected
    -> toggle showRejected
    -> persist app state
    -> reload current scope
    -> update native menu check state
```

Cleanup:

```text
cleanup command
    -> resolve scope
    -> collect rejected images in that scope
    -> show count and configured action
    -> run action
    -> reload current scope
```

Accepted file action:

```text
accepted copy/move command
    -> resolve selected images or current working set
    -> collect accepted images
    -> choose destination folder
    -> copy or move files
    -> refresh folders/images as needed
```

## Error Handling

- If toggling visibility reloads an empty view, keep focus at `0` and show the normal empty state.
- If a cleanup action partially succeeds, show `N/M` completion and reload the current scope.
- If moving to Trash fails for a file, leave its database record unchanged and report the failure.
- If a copy or move destination has a filename conflict, skip that file, continue the batch, and report the skipped count with the first conflicting filename.
- If a smart collection filter cannot be parsed for decision intent, use the safe default: hide rejected unless `showRejected` is true.

## Testing

### Unit Tests

- `showRejected` persistence round-trips through app state.
- native menu state includes `showRejected`.
- filter helper detects explicit rejected smart collection filters.
- image-loading scope options exclude rejected by default.
- explicit rejected scopes include rejected even when `showRejected` is false.

### Rust Tests

- query layer excludes `decision = reject` when requested.
- query layer includes rejected when requested.
- rejected cleanup uses the existing Trash/missing-file path.

### E2E Tests

- Mark an image rejected with `x`; it disappears from grid by default.
- Enable **View -> Show Rejected**; the rejected image reappears with its reject badge.
- Disable **Show Rejected**; the rejected image disappears again.
- Open the Rejected smart collection with **Show Rejected** off; rejected images still appear.
- Run rejected cleanup on a test fixture; files move to Trash/missing state and the current view refreshes.

## Implementation Phases

### Phase 1: Visibility

- Add `showRejected` store and persistence.
- Add **View -> Show Rejected** native menu item.
- Wire menu state and menu action handling.
- Apply rejected filtering in image loading/query paths.
- Preserve explicit Rejected smart collection visibility.
- Add unit and E2E coverage for visibility.

### Phase 2: Rejected Cleanup

- Add cleanup scope resolution.
- Add configurable cleanup setting.
- Reuse Trash behavior for the default cleanup action.
- Add confirmation with count and configured action.
- Add partial-failure reporting.

### Phase 3: Accepted File Actions

- Add accepted collection from selected/current scope.
- Add copy accepted to folder.
- Add move accepted to folder.
- Refresh folders and current scope after moves.
- Report filename conflicts as skipped files, not hard failures.
