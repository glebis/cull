# External Drive Browse-in-Place Design

**Date:** 2026-08-30  
**Status:** Approved interaction model; implementation pending  
**Owner:** Cull

## Problem

Cull currently turns Finder drops, open-file events, and folder navigation into an eager import. The frontend waits for `import_folder` or `import_files`, while the backend recursively discovers files, hashes and decodes them, generates thumbnails, detects source metadata, and schedules post-import analysis. The originals are already referenced in place rather than copied, but the product presents the work as a blocking import and does not expose mounted media as a browsable source.

That interaction fails the primary job:

> Plug in an SD card or external drive, open a folder, and begin rating or rejecting photographs immediately without copying the originals.

Dragging is not the primary entry point. Mounted devices must be visible without user setup, and browsing must work like a file browser.

## Product Decisions

1. Mounted SD cards and external drives appear automatically in a `Devices` section at the top of the sidebar.
2. Clicking a device or folder opens it without an import dialog or copy step.
3. Originals remain on the device. Cull stores database records and review decisions locally. Generated thumbnails for referenced-only images are a disposable cache, not permanent library content.
4. Folder scope is configurable between `Current folder` and `Include subfolders`. Cull remembers the last choice per referenced source.
5. A dropped folder opens in browse-in-place mode. Copying files into Cull is a separate explicit action.
6. Review actions never modify the external source. Rating, colour label, accept, reject, and collection membership are allowed; rename, move, trash, and permanent delete are disabled for referenced external files in the first release.
7. Ejecting a device does not erase the review or mark every file individually missing. The source becomes offline, review decisions remain, referenced-only thumbnails are purged, and original-dependent actions explain that the device must be reconnected. Reconnecting regenerates thumbnails on demand.
8. Reconnecting the same device restores its source and file paths automatically when a stable platform volume identity is available. Cull never guesses when identity is ambiguous.
9. Browsing a referenced source does not grant permanent library membership. Referenced-only images remain available in the active referenced-source scope and in explicit user collections, but are excluded from All Images, Library folders, automatic filters, search, and smart collections such as Recent Imports. Library membership is durable per file record: explicitly importing a browsed path promotes that same record, and browsing a previously imported path never demotes it. An image with any library-member file record remains a library member. Removing a referenced source from Cull removes only Cull's browse-only references and cached derivatives, never originals or explicitly imported membership.

## Interaction Design

### Sidebar

`Devices` is the first content section in the sidebar, above sessions, recent scopes, and the library tree. It updates when volumes mount or unmount.

Each device row shows:

- volume name;
- device kind when known (`SD card`, `External drive`, or `Mounted volume`);
- capacity in compact form;
- online or offline state;
- an expansion control for its folder tree.

Internal system volumes, hidden recovery volumes, app bundles, and read-inappropriate mount points are excluded. Manually chosen folders remain available as referenced sources even when they are not removable devices.

Clicking a device opens its root. Clicking a folder changes the active source scope. The existing Library folders remain below and continue to represent all catalogued paths.

### Grid toolbar

When a referenced source is active, the grid toolbar shows:

- a breadcrumb from device to current folder;
- a scope control with `Current folder` and `Include subfolders`;
- a compact `Originals stay on drive` status;
- indexing progress only when background work is still needed.

Folder discovery must not block the entire view. Cull renders the folder shell and skeleton tiles immediately, prioritises the first visible page, and fills thumbnails progressively. Scrolling schedules subsequent pages. Changing folders cancels work that is no longer useful.

### Drag and open events

Outside Canvas:

- dropping one folder opens that folder as a referenced source;
- dropping image files opens a temporary referenced set backed by their parent source;
- Finder `Open With` follows the same referenced path;
- the overlay says `Open to review`, not `Drop to import`.

Canvas keeps its explicit add-to-canvas semantics, but registration still references the original path rather than copying it unless the user chooses a copy/export command.

### Offline behaviour

When a source disconnects:

- the device row becomes offline instead of disappearing if it has remembered review state;
- the active referenced-source grid keeps its review state, but purged previews show a reconnect state until the source returns;
- ratings and decisions remain editable because they live in Cull;
- reveal, full-resolution loupe, export from original, Open With, and copy actions show `Reconnect <device>`;
- no watcher event may convert a whole disconnected source into thousands of missing-file records.

If a different device appears at the same mount path, Cull treats it as a different source unless the stable volume identity matches.

## Architecture

### 1. Mounted source discovery

Add a platform-neutral `MountedSourceProvider` interface returning:

```rust
struct MountedSource {
    platform_volume_id: Option<String>,
    display_name: String,
    mount_path: PathBuf,
    kind: MountedSourceKind,
    capacity_bytes: Option<u64>,
    writable: bool,
}
```

The macOS implementation uses mounted-volume metadata and workspace mount/unmount notifications. Platform code emits one debounced `sources:changed` event; the frontend then refreshes the canonical list. Tests use an injected fake provider rather than real volumes.

Stable platform volume identity is preferred. If a volume does not expose one, Cull may remember the path for the current mount but must request confirmation before reconnecting an offline review to a later ambiguous mount.

### 2. Referenced source service

Add a backend service responsible for:

- listing mounted and remembered sources;
- listing child folders without cataloguing their full contents;
- starting or cancelling progressive reference jobs for a folder scope;
- resolving an online absolute path from source identity plus relative path;
- transitioning sources between online and offline states;
- removing Cull references without touching originals.

This service is the only layer allowed to reconcile volume identity and mount paths. UI code never constructs `/Volumes/...` paths itself.

### 3. Progressive registration, not a second image model

Do not create a parallel `ExternalImage` frontend model. Grid, Loupe, Compare, selections, collections, export, MCP, and search already depend on `ImageWithFile` and stable image IDs. A second model would duplicate or disable most of Cull's working review surface.

Instead, split the existing eager import pipeline into independently schedulable stages:

1. discover supported paths for the requested page;
2. reuse an unchanged existing `image_files` record when path, size, and modification time match;
3. register and decode visible items first without adding them to permanent library scopes;
4. generate the thumbnail required for the current grid size as a disposable referenced-source cache;
5. emit a batched `referenced-source:page-updated` event;
6. run source detection, lineage, and optional analysis at lower priority after the item is usable.

The first release may keep hashing in the registration stage where the current schema requires it, but it must process visible files incrementally and must not wait for a recursive folder to finish before showing the first page. A later deferred-hash optimisation requires a separate migration and image-identity reconciliation design; it is not smuggled into this feature.

The existing `import_folder` and `import_files` commands remain for explicit headless import workflows. UI browsing calls new source commands and uses neutral terms such as `Open`, `Review`, and `Indexing`.

### 4. Persistent identity and schema

Add a migration after schema version 26 with two tables:

```sql
CREATE TABLE referenced_sources (
    id TEXT PRIMARY KEY,
    platform_volume_id TEXT,
    display_name TEXT NOT NULL,
    last_mount_path TEXT,
    source_kind TEXT NOT NULL,
    recursive_default INTEGER NOT NULL DEFAULT 0,
    settings_json TEXT NOT NULL DEFAULT '{}',
    last_seen_at TEXT NOT NULL,
    offline_at TEXT
);

CREATE UNIQUE INDEX referenced_sources_platform_id_uq
ON referenced_sources(platform_volume_id)
WHERE platform_volume_id IS NOT NULL;

CREATE TABLE referenced_files (
    image_file_id TEXT PRIMARY KEY REFERENCES image_files(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES referenced_sources(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    first_seen_at TEXT NOT NULL,
    UNIQUE(source_id, relative_path)
);
```

`image_files.path` remains the currently resolved absolute path so existing image queries continue to work. On verified reconnection, one transaction updates descendant absolute paths from `last_mount_path` to the new mount path, clears source-offline state, and preserves image IDs and selections. `referenced_files` provides the stable source-relative mapping required for safe reconnection.

Removing a referenced source is one explicit transaction: delete its `referenced_files` rows, delete associated browse-only `image_files` rows, preserve promoted library-member rows, delete only images that no longer have any file record, and then remove source-owned cached thumbnails. Images still retained through explicit membership or another path remain intact. Originals are never opened for writing or deleted.

The existing `library_roots` watcher contract remains in force. Referenced roots are watched only while online; whole-root disconnects follow the current offline guard and are surfaced as source state rather than destructive missing-file churn.

### 5. Frontend state and API

Add these API concepts:

```typescript
type ReferencedSource = {
  id: string;
  display_name: string;
  mount_path: string | null;
  kind: 'sd_card' | 'external_drive' | 'mounted_volume' | 'folder';
  capacity_bytes: number | null;
  online: boolean;
  recursive_default: boolean;
};

type LibraryScope =
  | ExistingScopes
  | {
      type: 'referenced_folder';
      source_id: string;
      relative_path: string;
      recursive: boolean;
      include_rejected: boolean;
    };
```

The new scope owns breadcrumb, recursion preference, paging, cancellation, offline state, and cache key. Existing selection and view stores continue to operate on returned `ImageWithFile` records. Permanent automatic library queries require an `image_files.library_member` row; referenced-folder queries continue to read source-linked rows directly, including browse-only records.

`Sidebar.svelte` should not absorb volume discovery logic. Add a focused `DevicesSection.svelte` and a small referenced-source store/service, then compose the section at the top of the existing sidebar.

## Commands and Events

Proposed Tauri commands:

- `list_referenced_sources()`
- `list_source_folders(source_id, relative_path)`
- `open_referenced_folder(source_id, relative_path, recursive, cursor, limit)`
- `set_source_recursive_default(source_id, recursive)`
- `remove_referenced_source(source_id)`
- `cancel_referenced_source_job(job_id)`

Proposed events:

- `sources:changed` — mount, unmount, or remembered-source state changed;
- `referenced-source:page-updated` — a batch of visible image IDs became usable;
- existing job progress events — indexing progress and cancellation;
- existing `watcher:volume-offline` — translated into source state by the service.

All filesystem paths pass the existing path validation and asset-protocol confinement. Remote MCP responses continue to apply path redaction.

## Safety and Error Handling

- Never copy, move, rename, trash, or delete an original as a side effect of browsing or reviewing.
- Destructive commands validate source ownership in the backend; hiding a menu item is not sufficient protection.
- A source disappearing during discovery cancels cleanly, retains completed records, and reports `Device disconnected` without treating partial progress as corruption.
- Unsupported files remain absent from the grid. If a folder contains no supported files, the empty state explains the supported-format boundary.
- Permission errors identify the affected folder and provide `Choose folder…` when manual access can resolve the problem.
- Ambiguous reconnection never rewrites stored paths automatically.
- Removing a source presents explicit copy: `Remove this source from Cull? Originals on <device> will not be changed.`
- Cull's real database is migrated in place and never reset or recreated for this feature.

## Testing and Verification

### Rust

- fake mounted-source provider: mount, unmount, rename, same path/different identity;
- migration 27 from a full version-26 database and schema invariant checks;
- source-relative path round trip and transactional remount path update;
- ambiguous identity refuses automatic reconnection;
- recursive and current-folder discovery with pagination and cancellation;
- disconnect during a reference job preserves completed records;
- backend rejection of rename, move, trash, and permanent delete for protected referenced files;
- watcher root-offline regression coverage;
- full `cargo test --lib`, formatting, and clippy.

### Frontend

- `Devices` is above the other sidebar sections and updates from `sources:changed`;
- current-folder versus recursive scope and persisted preference;
- folder click begins progressive loading without calling `importFolder`;
- the drag/drop overlay reads `Open to review` and routes to referenced browsing;
- skeleton, indexing, empty, permission, disconnect, offline, and reconnect states;
- cached review remains navigable offline while original-dependent actions are blocked;
- existing Library, selection, Compare, Loupe, and collection tests remain green.

### Browser and native smoke

- browser smoke uses the E2E-only Tauri mock for mount/unmount and paging states;
- native macOS smoke uses a disposable mounted disk image or dedicated test volume, never the user's real SD card;
- verify first visible batch before recursive discovery completes;
- verify eject/reconnect preserves ratings and does not mark the whole source missing;
- run Cull's full preflight before landing.

## Delivery Slices

1. **Mounted sources and sidebar:** provider abstraction, schema, source list, mount events, Devices UI.
2. **Browse-in-place:** folder listing, referenced scope, progressive first-page registration, recursion control, cancellation.
3. **Safety and continuity:** offline/reconnect reconciliation, backend destructive-action guard, cached review states.
4. **Entry-point repair:** drag/drop and Finder Open With route to review; explicit import remains available for CLI and deliberate workflows.
5. **Native validation and landing:** disposable-volume smoke, full preflight, changelog, tracked issue closure, feature landing flow.

Each slice must leave the app usable and preserve the user's existing `cull.db`.

## Out of Scope

- Automatically copying selects at the end of a review;
- deleting or moving rejects on the external device;
- editing files or writing XMP sidecars to the device;
- replacing PhotoKit/Apple Photos import;
- inventing support for image or RAW formats Cull cannot currently decode;
- background indexing of every mounted drive without the user opening it;
- deferred content hashes and cross-source identity reconciliation beyond the existing hash pipeline;
- signed release packaging.

## Acceptance Criteria

1. A newly mounted supported external volume appears in `Devices` without relaunching Cull.
2. The user can click into a folder and see progressive grid results without an import dialog and without copying originals.
3. The first visible page is prioritised; recursive discovery does not block it.
4. `Current folder` and `Include subfolders` both work and the last setting is remembered per source.
5. Ratings, labels, accepts, rejects, and collection membership persist across app restart and source disconnect.
6. Reconnecting the verified same source restores paths and review state automatically.
7. Reusing a mount path for a different source does not attach the old review.
8. Dragging a folder outside Canvas opens it for review and never calls the eager UI import flow.
9. Browse and review cannot modify external originals; backend guards cover every destructive command.
10. Removing the source from Cull leaves every original untouched.
11. The existing Apple Photos flow, explicit CLI/MCP imports, Library scopes, and real database remain compatible.
