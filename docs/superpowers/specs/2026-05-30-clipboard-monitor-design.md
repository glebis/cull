# Clipboard Monitor

## Goal

Add a Clipboard Monitor workflow for reference gathering. When enabled, Cull watches the OS clipboard for copied images, writes each captured image as a real file in a configurable folder, imports it into the library, appends it to a newly created collection, and focuses that collection in Grid mode.

Primary workflow: the user browses Pinterest or another reference source, copies about 30 images, then uses Cull's collection, static publishing, and MCP surfaces to hand the reference set to agents that extract design code, design decisions, and templates.

## Product Requirements

- A user can turn Clipboard Monitor on and off from the app.
- Turning it on creates a new manual collection and focuses it in Grid mode.
- Clipboard images copied while the monitor is on are saved as real files on disk.
- Saved files are imported into Cull and appended to the monitor collection.
- Saved filenames are readable and include local date, local 24h time, and the best available source or original filename.
- The capture folder is configurable and can be moved after captures exist.
- The feature must handle OS-level clipboard access clearly. If permission is denied or unavailable, the monitor must stop or remain inactive with a visible status.
- A monitored collection can be published through the static publishing pipeline.
- The resulting publish URL is shown in the app, copied to the OS clipboard, and available through MCP.
- External agents can discover the collection and publish result through MCP.

## Important Constraint

Browsers do not consistently put source URL or original filename on the clipboard when the user copies an image. For Pinterest and similar sites, Cull should extract these fields when the clipboard includes a file URL, source URL, HTML, or URL string. When the clipboard only exposes image bytes, Cull must still save the image using timestamp and a generic source label such as `clipboard`.

## Platform Assumption

Implement a cross-platform service boundary now, with macOS native support first.

- macOS: implemented with `NSPasteboard`.
- Windows/Linux: commands return an explicit unsupported status until native readers are added.

This matches the current app's macOS-heavy native surface and avoids pretending that clipboard permission behavior is portable.

## Existing Surfaces To Reuse

- Manual collections: `projects` and `collection_items`.
- Collection APIs: `create_collection`, `add_to_collection`, `list_collection_images`.
- Import pipeline: `db_core::import::import_file`, post-import thumbnail/source/sidecar/quality/detection hooks.
- Grid scoping: `activeCollection` plus `loadImagesForCurrentScope`.
- Navigation events: existing display service emits app navigation requests.
- Settings: `app_settings` via `get_app_setting` and `set_app_setting`.
- Static publishing: `export_static_publish_package` and `serve_static_publish_package`.
- MCP: existing collection tools, export tools, token scopes, and module gating.

## Data Model

### Settings

Use `app_settings` for user configuration and last active state:

| Key | Value |
| --- | --- |
| `clipboard_monitor_capture_dir` | Absolute capture folder path. Defaults to app data `Clipboard Captures`. |
| `clipboard_monitor_last_collection_id` | Last collection created by the monitor. |
| `clipboard_monitor_poll_ms` | Optional poll interval, default 750. |
| `clipboard_monitor_auto_publish_host` | Optional host for quick publish, default `127.0.0.1`. |
| `clipboard_monitor_auto_publish_port` | Optional port for quick publish, default static publishing setting or 8000. |

Do not persist "monitor is running" as an automatic app-start behavior in the first version. Clipboard monitoring is privacy-sensitive and should require an explicit user action per app run.

### Collection Metadata

`projects.settings_json` already exists and should record monitor metadata for created collections:

```json
{
  "source": "clipboard_monitor",
  "capture_dir": "~/Pictures/Cull Clipboard Captures",
  "started_at": "2026-05-30T12:35:00Z"
}
```

Add typed DB helpers so collection metadata can be written without ad hoc SQL in command code:

```rust
Database::set_collection_settings_json(collection_id, settings_json)
Database::get_collection_settings_json(collection_id)
```

### Capture Records

No new table is required for the first version. Captures are represented by:

- `images` row from normal import.
- `image_files.path` pointing at the capture folder file.
- `collection_items` row linking image to the monitor collection.
- Optional `raw_metadata` or source detection fields when available.

If implementation needs richer audit history later, add `clipboard_captures`, but do not block the first version on it.

## Rust Architecture

### New Module

Create `src-tauri/src/services/clipboard_monitor.rs`.

Responsibilities:

- Resolve and validate the capture directory.
- Start and stop the monitor session.
- Create a monitor collection.
- Poll the platform clipboard.
- Deduplicate clipboard changes.
- Write captured bytes to a durable file.
- Import the file through existing import code.
- Append imported image IDs to the active monitor collection.
- Emit progress and navigation events to the frontend.
- Move the capture folder and update DB paths.

### Service State

Add to `AppState`:

```rust
pub clipboard_monitor: Mutex<ClipboardMonitorState>
```

State:

```rust
pub struct ClipboardMonitorState {
    running: bool,
    collection_id: Option<String>,
    capture_dir: Option<PathBuf>,
    last_change_count: Option<i64>,
    last_hash: Option<String>,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}
```

The service must be idempotent:

- Starting while running returns the current status.
- Stopping while inactive returns inactive status.
- A duplicate clipboard change with the same content hash is skipped.

### Platform Boundary

Create:

```rust
trait ClipboardImageReader {
    fn status(&self) -> ClipboardAccessStatus;
    fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, ClipboardMonitorError>;
}
```

Capture:

```rust
pub struct ClipboardCapture {
    pub bytes: Vec<u8>,
    pub extension: String,
    pub original_filename: Option<String>,
    pub source_url: Option<String>,
    pub source_app: Option<String>,
    pub change_count: Option<i64>,
}
```

Status:

```rust
pub enum ClipboardAccessStatus {
    Supported,
    UnsupportedPlatform,
    PermissionRequired,
    PermissionDenied,
    Error(String),
}
```

### macOS Reader

Implement in `src-tauri/src/services/clipboard_monitor_macos.rs` behind `#[cfg(target_os = "macos")]`.

Use `NSPasteboard.generalPasteboard`:

- Track `changeCount` to avoid repeated reads.
- Prefer file URLs when the pasteboard contains image files.
- Otherwise read PNG/TIFF/image data and transcode to PNG or preserve encoded bytes when safe.
- Try to extract source URL from URL strings or HTML.
- Do not use private APIs for source app identity.

OS access behavior:

- The first read may trigger a macOS pasteboard privacy prompt on newer systems.
- The UI should show a "Waiting for clipboard access" status until the read succeeds or fails.
- If the OS denies access or repeated reads fail with a permission error, stop the monitor and show an actionable message.

### Filename Strategy

Add a pure helper with tests:

```rust
build_clipboard_capture_filename(capture, now, sequence) -> String
```

Format:

```text
YYYY-MM-DD_HH-mm-ss_<source-or-filename>_<sequence>.<ext>
```

Examples:

- `2026-05-30_14-35-22_pinterest_living-room-reference_001.png`
- `2026-05-30_14-36-08_dribbble_card-layout_002.png`
- `2026-05-30_14-36-41_clipboard_003.png`

Rules:

- Use local time and 24h format for readability.
- Sanitize to ASCII slug components.
- Prefer original filename stem.
- Else derive a source label from host, such as `pinterest`.
- Else use `clipboard`.
- Always include a sequence to avoid collisions.
- If a filename exists, increment the sequence.

### Capture Flow

```
start_clipboard_monitor()
  -> resolve capture dir
  -> create dir if missing
  -> create collection named "Clipboard YYYY.MM.DD HH:mm"
  -> store settings_json on collection
  -> keep capture dir out of asset protocol scope
  -> emit navigate collection event with view grid
  -> spawn guarded polling task

poll tick
  -> reader.read_if_changed()
  -> if no image, continue
  -> hash bytes, skip duplicate hash
  -> build readable filename
  -> atomic write to capture dir
  -> import_file(db, path, app_data_dir)
  -> UI uses app-owned thumbnail/generated assets; if no safe preview exists, show preview unavailable
  -> if imported image id exists, add_to_collection(collection_id, [id])
  -> create import batch with source "clipboard"
  -> set the imported image's batch id
  -> emit clipboard-monitor:capture event
  -> emit images:changed
```

Atomic write:

- Write to `.<filename>.tmp`.
- Flush.
- Rename to final path.
- On failure, remove the temp file with `trash` only if it is a user-visible file. Temp cleanup may use `std::fs::remove_file` for hidden incomplete temp files inside Cull-owned capture dir.

### Capture Folder Move

Command:

```rust
move_clipboard_capture_folder(new_path: String) -> Result<ClipboardMonitorStatus, String>
```

Flow:

1. Validate `new_path` under home or a user-selected absolute path accepted by Tauri dialog.
2. Pause monitor if running.
3. Create destination folder.
4. For every image file currently under the old capture dir and in monitor-created collections, copy to destination preserving relative filename.
5. Verify file size and hash after copy.
6. Update `image_files.path` for moved records.
7. Update `clipboard_monitor_capture_dir`.
8. Do not add destination to asset protocol scope; previews must come from app-owned generated/thumbnail assets.
9. Emit `images:changed`.
10. Resume monitor if it was running.
11. Leave old files in place by default for the first version, then show a cleanup option later. This avoids accidental data loss.

Because the requirement is "folder should be easily movable", the first implementation must move Cull's references and copy files. Deleting old files can be a later explicit cleanup action.

## Tauri Commands

Add `src-tauri/src/commands/clipboard_monitor.rs`.

```rust
start_clipboard_monitor(capture_dir: Option<String>) -> ClipboardMonitorStatus
stop_clipboard_monitor() -> ClipboardMonitorStatus
get_clipboard_monitor_status() -> ClipboardMonitorStatus
set_clipboard_monitor_capture_dir(path: String) -> ClipboardMonitorStatus
move_clipboard_capture_folder(new_path: String) -> ClipboardMonitorStatus
publish_clipboard_collection(collection_id: Option<String>) -> ClipboardPublishResult
```

Status:

```rust
pub struct ClipboardMonitorStatus {
    pub running: bool,
    pub supported: bool,
    pub access_status: String,
    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
    pub capture_dir: String,
    pub captured_count: u32,
    pub last_error: Option<String>,
}
```

Publish result:

```rust
pub struct ClipboardPublishResult {
    pub collection_id: String,
    pub image_count: u32,
    pub site_dir: String,
    pub url: String,
    pub manifest_path: String,
    pub instructions_path: String,
}
```

## Frontend Design

### Placement

Add a compact Clipboard Monitor section to the sidebar above Collections. This is an operational tool, not a landing page.

Controls:

- Toggle button: `Monitor Clipboard`.
- Status line: `Inactive`, `Waiting for access`, `Monitoring`, `Unsupported`, or `Error`.
- Capture folder row with current folder basename and buttons:
  - Choose folder.
  - Move folder.
  - Reveal folder.
- Active collection row with collection name and image count.
- Publish button once the collection has images.

Use existing Tokyo Night tokens from `app.css`. Do not hardcode colors.

### Start Interaction

When the user starts the monitor:

1. Call `startClipboardMonitor`.
2. Store returned status in a Svelte store.
3. Refresh collection list.
4. Set `activeCollection` to returned `collection_id`.
5. Clear `activeFolder`, `activeSmartCollection`, and `activeDetectedClass`.
6. Navigate to Grid mode.
7. Load the collection scope with cache invalidation.

### Capture Events

Listen for:

- `clipboard-monitor:status`
- `clipboard-monitor:capture`
- `clipboard-monitor:error`

On capture:

- Refresh collection list.
- If the active collection is the monitor collection, reload current scope without resetting focus unless this is the first image.
- Show a concise toast: `Captured clipboard image`, detail filename.

### Navigation Event Fix

Existing `services::display::show_collection` emits `navigate-collection`, but the frontend does not currently listen for it. Add a listener in `deeplink.ts` or root app initialization:

```ts
listen<{ collection_id: string }>('navigate-collection', async event => {
  activeCollection.set(event.payload.collection_id);
  activeFolder.set(null);
  activeSmartCollection.set(null);
  activeDetectedClass.set(null);
  navigateTo('grid');
  await loadImagesForCurrentScope({ force: true, invalidateCache: true });
});
```

This also improves current MCP `show_collection`.

## Publishing Design

Static publishing currently accepts image IDs. Add a helper that builds a `StaticPublishRequest` from a collection:

```rust
export_static_publish_collection(collection_id, output_dir, share_url, include_options)
```

Implementation:

- Resolve collection images via `list_collection_images`.
- Build `StaticPublishRequest` with one item per image ID.
- Use collection name as `canvas_name` and site title.
- Call existing `export_static_publish_package_inner`.
- Start the existing static server with `serve_static_publish_package_inner`.
- Return the local URL.
- Copy the URL to clipboard from the frontend after command success.

The publish button should show:

- URL.
- Site folder.
- Handoff file.
- Copy URL.
- Open URL.

If the Static Publishing module is disabled, the publish button should explain that it must be enabled rather than failing silently.

## MCP Design

Add MCP tools:

```text
get_clipboard_monitor_status
show_clipboard_collection
publish_clipboard_collection
get_last_clipboard_publish
```

Tool behavior:

- `get_clipboard_monitor_status`: read-only status and collection ID.
- `show_clipboard_collection`: navigates local app to the active monitor collection.
- `publish_clipboard_collection`: exports and serves a collection, returns URL and manifest paths.
- `get_last_clipboard_publish`: returns last successful publish result for agents that need the URL after the UI action.

Capabilities:

- Status and last publish: `library:read`.
- Show collection: display permission, matching existing `show_collection`.
- Publish: `export:read` and static publishing module gate.

Scope:

- If token scope has `collections`, the requested collection must be in scope.
- If token scope has only folder scope, publishing is allowed only when all collection images are in scope, following existing `export_images` behavior.

## Clipboard Write For URL

The frontend should copy the publish URL with `navigator.clipboard.writeText(url)` after a successful publish. If browser clipboard write fails, show the URL and Copy button error; do not treat publish as failed.

## Privacy And Safety

- Monitoring is explicit per app run.
- Do not upload clipboard data.
- Do not read or store non-image clipboard payloads except metadata needed to name the image.
- Do not log clipboard bytes.
- Do not use private macOS APIs for paste source identity.
- Do not delete old capture files during folder move in the first version.
- Do not import from or write into `cull.db` directly outside normal DB APIs.

## Error Handling

| Case | Behavior |
| --- | --- |
| Unsupported platform | Toggle disabled or returns unsupported status. |
| Clipboard access denied | Stop monitor, show status with instructions. |
| Clipboard contains no image | Ignore. |
| Duplicate copied image | Skip based on bytes hash. |
| Capture folder missing | Recreate if possible, else stop monitor with error. |
| Disk write fails | Keep monitor running but emit error and skip capture. |
| Import fails | Keep file on disk, emit error, do not add to collection. |
| Collection deleted while running | Stop monitor with error. |
| Static publishing disabled | Show module-disabled state, do not export. |

## Testing Plan

### Rust Unit Tests

- Filename generation uses timestamp, source host, original filename, and sequence.
- Filename sanitization removes unsafe characters and path separators.
- Duplicate hash detection skips repeated clipboard content.
- Capture folder resolution defaults to app data and accepts configured path.
- Capture folder move copies files and updates `image_files.path`.
- Unsupported platform reader returns `UnsupportedPlatform`.
- Collection settings JSON is written for monitor collections.

### Rust Integration Tests

- Starting monitor creates a manual collection with monitor metadata.
- Capturing image bytes writes a file, imports it, and adds it to the collection.
- Capturing the same bytes twice imports only once.
- Publish collection builds a static package using collection images.
- MCP scope checks block publishing a collection outside token scope.

Use a fake `ClipboardImageReader` for deterministic tests. Native macOS pasteboard behavior should be behind a small adapter so core logic remains testable without OS prompts.

### Frontend Tests

- Starting monitor sets active collection, clears other scopes, navigates to Grid.
- Capture event refreshes collection images when monitor collection is active.
- Publish success displays URL and attempts clipboard write.
- Unsupported status disables monitor controls cleanly.
- `navigate-collection` listener focuses Grid collection.

### Manual Test Scenario

1. Start Cull on macOS.
2. Choose or accept default capture folder.
3. Turn on Clipboard Monitor.
4. Confirm macOS clipboard access prompt if shown.
5. Verify new collection appears and Grid focuses it.
6. Open Pinterest in a browser.
7. Copy 30 image references.
8. Verify 30 readable files exist in the capture folder.
9. Verify the collection contains the images in capture order.
10. Publish the collection.
11. Verify the URL is visible and copied to clipboard.
12. Use MCP to read status, list collection images, and retrieve the publish URL.

## Out Of Scope For First Version

- Windows and Linux native clipboard readers.
- Automatic app-start monitoring.
- Deleting old files after capture folder move.
- Browser extension integration for guaranteed Pinterest source URLs.
- Full clipboard history.
- OCR or image analysis triggered by clipboard capture.
- Cloud publishing. The first version serves a local static package and returns handoff paths/URL.

## Implementation Order

1. Backend service core with fake reader and tests.
2. macOS pasteboard adapter.
3. Tauri commands and app state wiring.
4. Frontend store, controls, and collection navigation listener.
5. Collection static publish helper and UI result handling.
6. MCP tools for status, publish, and last publish.
7. E2E/manual verification.
