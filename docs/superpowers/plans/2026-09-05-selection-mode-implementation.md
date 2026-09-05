# Selection Mode implementation

Approved for implementation on 2026-09-05. Implements the native workflow in
`../specs/2026-09-01-selection-mode-design.md`; tracked by `imageview-60yo` and
children `.1` (persistence), `.2` (review UI), `.3` (proposal and acceptance).

## Delivery contract

- Capture a complete, ordered backend-resolved source and start empty.
- Keep highlighted IDs, shortlist membership and review decisions independent.
- Persist ordered membership and lifecycle in the database; use existing undo.
- Reuse image paging and per-scope view memory for Source and Shortlist.
- Replace visible Collect Mode with explicit start, resume, finish and archive.
- Shortlist proposals require review of the exact approved IDs. Native finish
  requires the Finish summary. MCP/CLI mirrors remain deferred until native UX
  is stable, as the spec allows; no ambient agent mutation authority is added.

## Shared native contract

`SelectionSourceScope` is the frontend `LibraryScope` tagged union, including
`referenced_folder`. Also support `search { base: SelectionSourceScope, query }`
for a backend-resolved text search intersected with its base scope. Do not use
visible thumbnail IDs as a substitute for resolving ordinary paged scopes.

`SelectionRun` has `id`, `name`, `status` (`active|finished|archived`),
`source_count`, `shortlist_count`, `target_count: number|null`,
`source_scope: SelectionSourceScope`, `created_at`, `updated_at`,
`finished_at: string|null` and `rejected_shortlist_count`.
`SelectionState` is `{run: SelectionRun, shortlist_ids: string[]}`.
`SelectionPage` is `{items: ImageWithFile[], total: number}`.

Commands use existing camelCase Tauri argument convention from frontend:

- `preview_selection_source(source_scope)` returns `{count: number}`.
- `create_selection_run(name, source_scope, target_count)` returns SelectionState.
- `list_selection_runs(status?)` returns SelectionRun[].
- `get_selection_run(selection_id)` returns SelectionState.
- `list_selection_source(selection_id, offset, limit, query?, min_size?, include_rejected?)`
  and `list_selection_shortlist` with the same arguments return SelectionPage.
- `add_to_shortlist(selection_id, image_ids)` and `remove_from_shortlist` return
  SelectionState after one undoable group operation.
- `finish_selection_run`, `reopen_selection_run`, `archive_selection_run`,
  `restore_selection_run`, each with `selection_id`, return SelectionState.

All command failures return actionable errors. Membership groups are atomic:
validate all IDs before writing and roll back the whole group on database
failure, so retry and one-step undo cannot conceal partially applied changes.
Idempotent no-ops do not add undo records. Emit `selection-run:updated` after
mutations and undo/redo. Count reads reflect surviving foreign-key references.

Offset/limit pagination follows existing Cull APIs rather than introducing a
second cursor protocol. Creation and lifecycle changes are transactional.
Paused UI work is not stored as a filesystem Cull Session.

## Implementation and evidence

1. Database migration, complete scope resolver, lifecycle, membership and undo.
   Tests: >200-image source, referenced-only IDs, reopen database, failures,
   idempotency, order, group undo/redo, unchanged decisions and file records.
2. Shared state/actions, native API, mode bar/dialogs, scoped loading, markers,
   keyboard/palette/sidebar entry points and retirement of visible Collect Mode.
   Tests: empty start despite highlights, capture IDs before awaits, rollback,
   focus/scroll separation, source filtering, finish summary, keyboard access.
3. Shortlist-specific proposal review and browser/native verification.
   Tests: exact approved subset, stale/outside-source rejection, no file actions;
   browser lifecycle/error coverage and isolated native restart/offline fixtures.
4. Independent review, rendered walkthrough, full preflight, update tracked
   acceptance status, and feature landing with required CI on the exact head.

Never reset or test against the real user database or removable media. Existing
RAW/device edits are preserved on their separate WIP branch.

## Accepted implementation, 2026-09-05

- Full preflight passed: 1,491 frontend tests in 204 files; 1,022 Rust tests
  across all targets, with three existing ignored tests. Svelte check reported
  zero errors and warnings; formatting and Clippy completed successfully.
- All 56 browser smoke scenarios passed, including nine Selection Mode journeys
  covering full sources, membership, grouped undo, scoped search, failed saves,
  reload, finish conflicts, archive, and a 900 px toolbar layout.
- The packaged macOS smoke passed two actual process launches against the same
  isolated database. It verifies source capture, empty start, saved membership,
  restart persistence, undo/redo, finish, archive/restore, unchanged image paths
  and decisions, plus the existing native pointer/navigation interactions.
- License/provenance audit passed. GLM-assisted implementation through Pi and
  subsequent review are disclosed in AUTHORSHIP.md.

Review fixes include atomic proposal consumption, preserving unrelated manual
collection edits during undo, exact parsed search capture, session-owned queued
membership intentions, number-input handling, toolbar placement, the native
preview response shape, and matching native/browser collection identities.

The native harness uses a short temporary HOME and seeds its exact runtime data
directory. It requires both a clean process exit and a phase-specific result;
a stale success manifest cannot satisfy restart verification.

External-drive safety is covered by database/source fixtures; this delivery did
not test against a real removable drive or the user's library. MCP/CLI mirrors
are explicitly deferred in imageview-60yo.4. No signed release or installation
is part of this feature landing.

The full preflight is repeated by the feature landing script before publication.
Commit/push hooks use the lightweight hook tier to avoid duplicating that same
full verification; no checks in the landing script are bypassed.
