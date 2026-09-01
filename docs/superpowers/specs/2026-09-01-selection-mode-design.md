# Selection Mode Design

**Date:** 2026-09-01

**Status:** Proposed interaction and state model

**Owner:** Cull

**Builds on:** collection membership, Collect Mode, persistent decisions, undo history, and referenced-source browsing

## Outcome

Selection Mode is an opt-in review workflow. It starts with an empty shortlist and lets the user deliberately add images that deserve a place in the result while continuing to browse, filter, compare, and inspect the full source.

The mode is for jobs such as selecting five photographs from a shoot, assembling a portfolio edit, choosing work for a client, or building a set across AI generation runs. It is not a different filter over accepted and rejected images.

> Start with nothing. Add only what earns a place.

## Problem

Cull currently has three adjacent concepts:

- a focused image;
- `selectedIds`, the transient set of highlighted images used as an action scope;
- persistent `accept`, `reject`, and `undecided` decisions.

It also has a lightweight Collect Mode that targets a manual collection and uses Space to add the focused image. That is a useful interaction seed, but it is not yet a complete mode: its active state is not durable, it does not preserve a source snapshot, membership is not visually distinct from transient highlighting or decisions, and it has no explicit finish contract.

Calling all of these concepts “selection” makes it unclear whether an action changed the current UI highlight, a shortlist, or the lasting review decision. The ambiguity is especially risky across filters, paged results, application restarts, agent proposals, and disconnected external media.

## Job to Be Done

**When** I am reviewing a large folder, device, collection, search, or filtered view and only a small number of images should survive into a deliverable set,

**I want** to build a shortlist by explicitly adding promising images while I navigate and compare the full source,

**so I can** finish with a defensible, resumable set without confusing browsing highlights with approval or changing the originals.

## Terminology Contract

Product copy, command names, accessibility labels, tests, and APIs must keep these meanings distinct:

| Term | Meaning | Persistence |
|---|---|---|
| **Focused** | The single image receiving navigation or image-level commands | View state |
| **Highlighted** | One or more images in `selectedIds`, used as the scope for a group action | Transient; may reset with navigation |
| **Source** | The stable image set captured when Selection Mode starts | Persistent for the selection run |
| **Shortlisted** | Explicit membership in the current Selection Mode result | Persistent and undoable |
| **Accepted / Rejected / Undecided** | A lasting review decision stored in `selections` | Persistent and independent of shortlist membership |

Avoid the unqualified word **selected** in new interface copy. Use `highlighted`, `shortlisted`, or `accepted` according to the actual state.

## Product Decisions

1. **Selection Mode always starts empty.** Existing highlighted images do not silently seed the shortlist.
2. **The source is explicit and stable.** Starting the mode captures every image ID in the current resolved scope and order, not merely the currently loaded page.
3. **Shortlist membership is independent of review decisions.** Starting, adding, removing, finishing, reopening, or archiving a selection does not change `accept`, `reject`, or `undecided`.
4. **Selection Mode supersedes the current Collect Mode UI.** It preserves the useful collection-backed membership and Space interaction rather than introducing a second collection mechanism.
5. **Space toggles the focused image.** In the Source view it adds or removes the focused image from the shortlist. In the Shortlist view it removes the focused image, with Undo available.
6. **Highlighted images require an explicit group command.** `Add N highlighted to shortlist` and `Remove N highlighted from shortlist` are distinct commands. Clicking or range-highlighting never changes the shortlist by itself.
7. **The shortlist autosaves after each mutation.** Closing the window, changing views, changing filters, or disconnecting a device cannot discard it.
8. **Finishing produces a normal named collection.** It does not automatically accept the shortlist or reject the rest of the source.
9. **File actions remain separate.** Shortlisting and finishing never copy, move, rename, trash, delete, or export originals.
10. **Agent work remains proposed, not silently applied.** An agent may propose shortlist additions or removals, but the user reviews the exact image set before it changes.

## Interaction Model

### Starting

Primary entry point: `Start Selection…` in the command palette. A compact toolbar action may be added after the command behavior is established.

The start sheet shows:

- an editable name, defaulted from the current source;
- the source label and resolved image count;
- an optional target count;
- `Starts with an empty shortlist`;
- `Originals and review decisions are not changed`.

`Start Selection` is disabled for an empty or unresolved source. A paged view is resolved by the backend so the source snapshot includes the whole scope, not only visible thumbnails.

Starting from a manual collection, smart collection, search, folder, import batch, detected class, or referenced folder is allowed. Starting from `All Images` is allowed after showing the resolved count. Adding images from outside the captured source is out of scope for the first release.

### Mode chrome

While active, a persistent compact bar shows:

```text
Selection: Client final       Source 300       Shortlist 5 / target 5       Finish
```

The target suffix is absent when no target was supplied. The mode does not claim a `reviewed` count: opening, focusing, or rendering an image is not reliable evidence that a person reviewed it.

The bar provides two scopes:

- **Source** — the captured source, with normal search and filtering layered within it;
- **Shortlist** — current shortlist members in the order they were added.

Each scope remembers its own focus and scroll position. Switching scope, Grid/Loupe/Compare view, filter, or sort never clears shortlist membership. Transient highlighted IDs may reset according to existing navigation rules.

### Adding and removing

- Space toggles the focused image's shortlist membership.
- A group command adds or removes the current highlighted set after displaying its count.
- Re-adding an existing member and removing a non-member are idempotent no-ops.
- Each successful mutation updates the tile marker and count immediately, then persists the captured payload.
- Persistence failure restores the prior visible state and presents a retryable error; it never reports success optimistically after the backend rejects the change.
- Every add or remove participates in the existing undo/redo history. Group changes are one undoable action.

The existing `A`, `X`, and `U` decision commands remain available and continue to mean Accept, Reject, and Undecided. They never add or remove shortlist membership.

### Visual language

The interface must not rely on colour alone:

- focus retains the existing focus treatment;
- transient highlighting retains the existing selection treatment;
- shortlist membership uses a persistent bookmark/pick marker, an accessible `Shortlisted` label, and `--purple` as its supporting token;
- accepted and rejected retain their green check and red cross decision language.

A shortlisted image that is later marked rejected remains in the shortlist with both states visible. Cull reports the conflict in the shortlist summary but does not silently remove or reinterpret the image.

### Finishing

`Finish Selection…` opens a summary with:

- selection name;
- source count;
- shortlist count and optional target difference;
- rejected items still present in the shortlist, if any;
- confirmation that the result becomes a collection and no files or decisions change.

Finishing an empty shortlist is blocked; the user can archive it instead. Finishing is one transaction:

1. mark the selection run finished;
2. expose its collection-backed membership as a normal manual collection;
3. preserve its source snapshot and history for provenance;
4. exit Selection Mode and open the resulting collection.

`Continue as Selection` on that collection explicitly reopens the run. It does not create a new empty run or duplicate membership.

### Closing and archiving

Closing the app or leaving the mode keeps the run active and resumable. `Archive Selection` changes only its lifecycle state after confirmation. It retains membership and history and can be restored; it does not delete image records, decisions, collections, cached thumbnails, or files.

## Relationship to Culling

Selection Mode is deliberately one half of a broader review model:

| Selection Mode | Culling workflow |
|---|---|
| Starts with an empty shortlist | Starts with the full source still in play |
| Primary action is add | Primary action is reject/remove from consideration |
| Untouched means not shortlisted | Untouched means still undecided/in play |
| Result is an explicitly chosen set | Result is the source minus exclusions |

This specification does not add a Culling Mode. It preserves the distinction so a later culling or multi-pass funnel design can compose with Selection Mode instead of overloading it.

For a 300-image shoot ending in five final images, a future funnel may use culling to remove obvious failures and Selection Mode to build the final shortlist. The five final images are not inferred by marking all 300 accepted at the start.

## Persistence and Data Model

Selection Mode should reuse the existing `projects` and `collection_items` machinery so shortlist members already work with collection loading, ordering, export, and downstream actions.

Create an internal project with `collection_type = 'selection'`. While active it is omitted from normal collection lists and appears in the Selection Mode resume surface. Its `collection_items` rows are the canonical shortlist membership and preserve addition order.

Add lifecycle and source metadata in dedicated tables in the next migration after the current schema head:

```sql
CREATE TABLE selection_runs (
    id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('active', 'finished', 'archived')),
    source_scope_json TEXT NOT NULL,
    source_count INTEGER NOT NULL,
    target_count INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    finished_at TEXT
);

CREATE TABLE selection_run_source_items (
    selection_id TEXT NOT NULL REFERENCES selection_runs(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (selection_id, image_id),
    UNIQUE (selection_id, position)
);

CREATE INDEX selection_run_source_items_image_idx
ON selection_run_source_items(image_id);
```

`source_scope_json` is audit and resume context, not the source of membership truth. `selection_run_source_items` is the stable source snapshot. If an image is intentionally removed from Cull later, normal foreign-key cleanup may remove it from the source snapshot; counts are then recomputed and the run records that the source changed.

Finishing updates the backing project's `collection_type` from `selection` to `manual` and marks the run `finished` in the same transaction. Reopening performs the inverse lifecycle transition without altering `collection_items`.

Do not store shortlist membership in:

- the global `selections` table;
- `selectedIds` or local storage;
- a JSON array in `settings_json`;
- copied or symlinked files.

## Frontend State

Add a focused state model rather than expanding `selectedIds`:

```typescript
type SelectionRunStatus = 'active' | 'finished' | 'archived';

type SelectionRun = {
  id: string;
  name: string;
  status: SelectionRunStatus;
  source_count: number;
  shortlist_count: number;
  target_count: number | null;
  source_scope: LibraryScopeSnapshot;
  created_at: string;
  updated_at: string;
  finished_at: string | null;
};

type SelectionModeScope = 'source' | 'shortlist';
```

Frontend state may cache current shortlist IDs for immediate markers, but the database remains canonical. Scope loaders return ordinary `ImageWithFile` records so Grid, Loupe, Compare, ratings, decisions, context menus, Preview Display, and accessibility behavior remain shared.

The existing `collectMode` and `collectModeTarget` stores and commands are retired once Selection Mode reaches parity. Do not keep both modes visible.

## Commands and Events

Proposed Tauri commands:

- `create_selection_run(name, source_scope, target_count)`
- `list_selection_runs(status)`
- `get_selection_run(selection_id)`
- `list_selection_source(selection_id, filters, cursor, limit)`
- `list_selection_shortlist(selection_id, cursor, limit)`
- `add_to_shortlist(selection_id, image_ids)`
- `remove_from_shortlist(selection_id, image_ids)`
- `finish_selection_run(selection_id)`
- `reopen_selection_run(selection_id)`
- `archive_selection_run(selection_id)`
- `restore_selection_run(selection_id)`

Mutating commands validate that every target image belongs to the captured source and return the canonical membership/count result. They log user or agent provenance through `session_events` or the successor activity-event path without pretending a Selection Mode run is a filesystem-backed Cull Session.

Proposed events:

- `selection-run:updated` — lifecycle, count, or membership changed outside the current component;
- existing undo/history events for add/remove actions;
- existing source online/offline events for referenced media.

## Agent, MCP, and Automation Contract

Selection Mode is available to agents only through explicit, scoped operations:

- read a run, its source summary, and shortlist;
- propose additions or removals with reasons;
- apply only the IDs approved in the proposal review;
- finish only after a separate explicit user confirmation.

The existing agent proposal path that applies a proposed image set to transient `selectedIds` must not be reused as if that changed shortlist membership. Add a distinct proposal kind for shortlist changes and keep proposal review, partial approval, undo, and event provenance.

MCP and CLI surfaces should mirror the native commands after the native interaction is stable. Tokens need collection/library read access to inspect and curation write access to mutate. No agent receives file-delete authority from Selection Mode.

## External Drives and Offline State

Selection Mode composes with referenced-source browsing:

- originals remain on the device;
- source and shortlist membership are stored locally by stable image ID;
- cached thumbnails and shortlist markers remain available when the device disconnects;
- full-resolution inspection and original-dependent export explain which device must be reconnected;
- reconnecting the verified source restores normal operation without rebuilding the shortlist;
- removing a referenced source from Cull follows the referenced-source safety contract and never treats a selection as permission to delete originals.

## Accessibility

- Mode entry, source/shortlist toggle, membership controls, counts, Finish, and Archive are keyboard reachable.
- Space behavior is exposed in shortcut help and visible mode guidance.
- `Shortlisted` is conveyed through accessible name/state, not colour or an unlabeled icon.
- Count changes and add/remove outcomes use concise status announcements without moving focus.
- Source and Shortlist behave as an announced two-option scope control with a single selected state.
- Finishing, archiving, persistence failures, target mismatch, and rejected-shortlist conflicts are understandable without relying on colour.
- Reduced motion disables any fly-to-shortlist animation; membership still updates instantly.

## Safety and Error Handling

- Starting a run never writes selection decisions or file changes.
- A filter, search, sort, page, scope change, or app restart never removes off-screen shortlist members.
- Group mutations use the IDs captured when the user invoked the action, not a later mutable `selectedIds` value.
- Partial group failure returns per-item results, keeps successful mutations, and offers a retry for failed IDs.
- A disconnected source does not invalidate the run or mark every image missing.
- A shortlisted image may be accepted, rejected, or undecided; the states remain visible and independent.
- Finishing does not reject non-members, accept members, export files, or clear prior ratings/labels.
- Archiving is recoverable. Permanent deletion of a selection run is outside the first release.
- The real Cull database is migrated in place and is never reset as a recovery strategy.

## Non-Goals

- A new Culling Mode or guided multi-pass funnel.
- Automatically treating all source images as accepted.
- Automatically converting shortlist membership into accept/reject decisions.
- Copying, moving, exporting, trashing, deleting, or renaming originals.
- AI auto-selection without proposal review.
- Collaborative or remote multi-user selection state.
- Adding arbitrary images from outside the captured source.
- Ranking, pairwise tournament logic, or target-count optimisation.
- Website changes before the feature is implemented and verified.

## Testing and Verification

### Rust and database

- migrate a fully current database and verify both tables, constraints, and indexes;
- create a run from a paged scope and capture every resolved source ID exactly once in stable order;
- verify a new run has zero `collection_items` and does not create `selections` rows;
- add/remove single and grouped members idempotently and preserve addition order;
- reject adding an image outside the captured source;
- undo and redo grouped membership changes;
- finish atomically changes lifecycle and collection visibility without changing decisions;
- archive/restore preserves source and shortlist membership;
- deleting an image through an independent authorised workflow cleans up references without corrupting the run;
- external-source offline/reconnect preserves stable membership;
- full Rust library tests, formatting, and clippy remain green.

### Frontend

- starting from a non-empty `selectedIds` set still creates an empty shortlist;
- Space toggles only the focused image's shortlist membership;
- group commands use the captured highlighted IDs and are one undo step;
- Source/Shortlist switching preserves independent focus and scroll positions;
- search, filter, sorting, paging, and view changes preserve off-screen shortlist members;
- shortlist, transient highlight, accept, and reject indicators remain visually and semantically distinct;
- persistence failure restores the previous marker/count and exposes retry;
- Finish summary reports target mismatch and rejected shortlist members;
- existing Grid, Loupe, Compare, collection, decision, and shortcut tests remain green;
- current Collect Mode tests are replaced with Selection Mode behavior tests rather than kept as a parallel contract.

### Browser and native smoke

- E2E-only Tauri mock covers start, toggle, group add/remove, resume, finish, archive, and error states;
- native smoke starts from a real local folder and verifies persistence across app restart;
- dedicated test media verifies external-device disconnect/reconnect without touching the user's real SD card;
- accessibility smoke verifies keyboard-only completion and announced membership/count changes;
- run Cull's full preflight before landing implementation.

## Acceptance Criteria

- [ ] Starting Selection Mode from any supported explicit scope creates an empty, durable shortlist over the full resolved source.
- [ ] Transient highlighted IDs, shortlist membership, and review decisions remain separate in state, copy, visuals, and commands.
- [ ] Space and explicit group commands add/remove shortlist members with undo/redo and restart persistence.
- [ ] Source and Shortlist scopes share Grid, Loupe, and Compare without losing membership or off-screen items.
- [ ] Finishing creates a normal manual collection and changes no accept/reject decisions or files.
- [ ] Closing, filtering, paging, app restart, and referenced-source disconnect cannot discard the run.
- [ ] Agent changes require proposal review and use a shortlist-specific mutation path.
- [ ] All membership and lifecycle controls are keyboard accessible and do not rely on colour alone.
- [ ] The old Collect Mode is removed from the user-facing model once Selection Mode reaches parity.
- [ ] Focused tests and Cull's full preflight pass before implementation lands.

## Product Communication After Shipping

The useful website distinction is behavioral, not technical:

> Build a shortlist from an empty set. Browse everything, add only what earns a place, and finish without changing the originals.

Do not advertise Selection Mode until native behavior, persistence, external-drive safety, accessibility, and release verification have passed. Culling remains a separate workflow claim.

## Implementation Boundary

This document specifies behavior and architecture. It does not authorize product code, website changes, database migration, release packaging, or publishing. After approval, create a separate TDD implementation plan with narrow slices and trace each slice to the acceptance criteria above.
