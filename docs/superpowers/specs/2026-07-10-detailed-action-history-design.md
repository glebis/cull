# Detailed Action History Design

## Summary

Cull's Action History will become an understandable, actionable timeline rather than a compact diagnostic list. Every row will explain what changed and which image, collection, or item it affected. Image-related rows will include safe local previews. Users will be able to select and undo several recent actions in one operation without violating the undo stack.

## Goals

- Identify the affected image, collection, canvas, session, folder, or other item with a human-readable name.
- Explain the change, including useful before and after values where available.
- Show a thumbnail for image-related actions and a stable placeholder when no preview is available.
- Let users select and undo several actions as one explicit batch.
- Preserve strict stack ordering and existing redo behavior.
- Add clear hover, focus, selected, pending, success, and failure states.
- Top-align all row cells so multi-line content remains easy to scan.
- Keep raw JSON, UUIDs, internal paths, and other diagnostic output out of the default presentation.

## Non-goals

- Arbitrary out-of-order undo.
- Replaying newer actions around a removed historical action.
- Turning non-undoable activity events into undoable operations.
- Expanding the current undo action coverage beyond actions already recorded by `ActionManager`.
- Deleting or rewriting existing undo records or session events as part of the UI migration.

## Interaction Model

### Selection

The undo stack is ordered newest first. Selection must always be a contiguous prefix of that stack:

- Selecting the newest row selects one action.
- Selecting a lower row selects that row and every newer row above it.
- Clicking an already selected row shortens the selection to the rows above it; clicking the newest selected row clears selection.
- Non-undoable activity rows never display selection controls.
- Keyboard users can focus a row checkbox and use Space to apply the same contiguous-prefix behavior.

This rule is enforced in both the frontend and Rust. The frontend cannot request arbitrary record IDs, and the backend accepts only a count of newest actions to undo.

### Batch action

When no rows are selected, the toolbar retains the normal single-action `Undo` and `Redo` controls. When rows are selected, the primary control becomes `Undo N actions`; a secondary `Clear selection` control appears.

Activating `Undo N actions` calls `undo_many(count)`. The backend undoes newest-first, exactly as repeated single undo would. The operation returns the labels of completed actions and an optional failure. If action K fails:

- actions 1 through K-1 remain undone and are available to redo;
- action K and older actions remain unchanged;
- processing stops immediately;
- the response reports the completed count and failure message;
- the panel reloads and shows a toast such as `Undid 2 of 4 actions` with the failure detail.

The operation is intentionally not described as database-atomic because undo steps may include filesystem effects. Its contract is deterministic, ordered, stop-on-first-failure execution with precise reporting.

## Presentation

### Undoable row

Each row contains these top-aligned regions:

1. Selection checkbox.
2. 56 × 56 px preview or target glyph.
3. Primary content:
   - human-readable action title;
   - affected target name;
   - concise change description, such as `Decision: undecided → accepted`;
   - optional secondary location, collection, or item context.
4. Affected count, using `1 image`, `3 images`, or the appropriate target noun.
5. Local timestamp in 24-hour European formatting.

The action title is not duplicated as both a type and a label. The row prioritizes target identity and change meaning.

### Image previews

For records with affected image IDs, the backend enriches the history response with preview descriptors. Each descriptor contains only UI-safe fields:

- image ID;
- filename or display title;
- thumbnail path when the generated thumbnail exists;
- original path only when it already passes Cull's asset-protocol safety policy;
- missing-state flag.

The frontend selects the thumbnail first and uses the existing safe preview-path utilities before `convertFileSrc`. It never broadens asset protocol scope. Multi-image actions show the first preview with a `+N` overlay. Missing or inaccessible images show an image placeholder with the filename retained in text.

### Non-image targets

Enrichment resolves known identifiers into names:

- collections and smart collections: collection name and item count when available;
- canvases: canvas name and owning session name when available;
- sessions: session name;
- folders/files: display name, with a shortened parent context rather than a raw full path;
- import batches: imported/skipped/error counts and source;
- unknown or deleted targets: localized target type plus `Unavailable`, never a bare UUID as the primary label.

Non-image rows use a target-specific glyph in the preview slot so alignment stays consistent.

### Hover, focus, and selected states

- Hover raises border contrast and adds a subtle token-based surface tint.
- `:focus-within` uses the existing blue focus language and remains visible independently of hover.
- Selected rows use a blue-tinted surface, blue border, and checked control.
- Busy rows reduce secondary contrast but keep text readable; the toolbar shows progress.
- All colors use existing app tokens or `color-mix` based on those tokens.
- Reduced-motion mode removes nonessential transitions.

### Responsive behavior

At narrow widths, timestamp and count move below the primary content instead of disappearing. The preview remains visible. Text wraps naturally and stays top-aligned; no cell uses vertical centering.

## Data Model and APIs

### Enriched history record

The current `UndoRecord` remains the persistence model. The command layer maps it to a new response model:

```rust
struct UndoHistoryEntry {
    record: UndoRecord,
    action_title: String,
    target: HistoryTarget,
    change_summary: Option<String>,
    previews: Vec<HistoryImagePreview>,
    affected_count: u32,
    can_undo: bool,
}
```

`HistoryTarget` includes a target kind, display name, optional secondary context, and unavailable state. `HistoryImagePreview` carries the safe preview fields described above. Enrichment is performed in Rust to avoid N+1 frontend IPC calls and to ensure deleted/missing records receive consistent fallbacks.

`list_undo_history` returns `Vec<UndoHistoryEntry>`. Session activity remains a separate response and is mapped to the same visual row component through a frontend view model, but it has no checkbox or undo action.

### Batch undo response

```rust
struct UndoManyResult {
    requested: u32,
    completed: Vec<String>,
    failure: Option<String>,
}
```

`undo_many(count)` validates `1 <= count <= current stack depth` and applies the same cursor and redo bookkeeping as repeated `undo()`. The implementation must not bypass the existing action-specific restore logic.

## Component Boundaries

- `history-view-model.ts`: pure mapping and selection helpers; derives row copy, target noun, selection prefix, and preview choice.
- `HistoryTargetPreview.svelte`: renders safe image preview, multi-image badge, or non-image glyph.
- `HistoryRow.svelte`: accessible row layout and hover/focus/selected presentation.
- `UndoHistoryPanel.svelte`: loading, commands, selection orchestration, toolbar, and activity sections.
- Rust history enrichment service: resolves persisted record identifiers and before/after payloads into UI-safe response objects.
- `ActionManager::undo_many`: ordered batch execution and result reporting.

These boundaries keep formatting and selection behavior independently testable and prevent the panel from accumulating backend parsing logic.

## Error Handling

- A failed history load keeps the existing explicit error state and retry action.
- A missing thumbnail falls back without failing the row or history request.
- Malformed legacy payload JSON produces an action-specific fallback summary.
- Missing/deleted targets remain visible as unavailable history rather than being filtered out.
- Batch failure reports partial completion precisely and immediately reloads undo/redo state.
- Double submission is prevented while an undo or redo operation is running.

## Accessibility

- The history remains a labelled modal dialog.
- Undoable records form a labelled selection group; each checkbox includes action and target context.
- Selected count and batch results are announced through a live status region.
- Rows are navigable without pointer hover; all hover information is also present as visible text or an accessible label.
- Preview images use concise target alt text; decorative target glyphs are hidden from assistive technology.
- Focus indicators meet the app's established pattern.
- No essential metadata disappears at narrow widths.

## Testing

### Rust

- Enriches rating and decision records with filename, thumbnail, and before/after summary.
- Enriches multi-image records and reports the correct count.
- Resolves collection/canvas/session names and handles deleted targets.
- Rejects unsafe preview paths and preserves missing-image fallbacks.
- `undo_many` processes newest-first.
- `undo_many` validates count and stack depth.
- Partial failure stops processing and preserves accurate cursor/redo state.

### Frontend

- Prefix selection cannot contain gaps.
- Batch toolbar label and disabled state match selection and busy state.
- Image previews use the safe preview helper and render `+N` for batches.
- Missing images and non-image targets render stable placeholders.
- Target, change, count, and timestamp remain visible at desktop and narrow widths.
- Row cells are top-aligned.
- Hover, focus-within, selected, and reduced-motion styles exist and use design tokens.
- Raw JSON and UUID-only primary labels are absent.

### End-to-end and visual verification

- Open History against the real local database and confirm image previews render.
- Select three newest rows, undo them, and verify the panel, affected image state, Undo/Redo status, and toast.
- Redo the same three actions individually and confirm state returns.
- Verify a collection or canvas activity row displays its resolved name.
- Capture desktop and narrow-width screenshots showing readable top-aligned rows and hover/selected states.
- Rebuild, reinstall, restart, and repeat the installed-app smoke check.

## Acceptance Criteria

1. Every visible history row identifies an affected target with meaningful user-facing text.
2. Image-related undo rows display a safe preview or explicit missing-image placeholder.
3. Users can select a contiguous prefix of two or more newest undoable actions and undo it with one command.
4. Rust independently rejects invalid batch counts and preserves stack order.
5. Partial batch failure is accurately reported without claiming uncompleted actions were undone.
6. Hover, keyboard focus, and selected states are visibly distinct.
7. All row cells are top-aligned, including wrapped text and responsive layouts.
8. No raw JSON, unexplained UUID, or full internal path is the primary presentation.
9. Automated backend/frontend tests and installed-app visual verification cover the behavior.

