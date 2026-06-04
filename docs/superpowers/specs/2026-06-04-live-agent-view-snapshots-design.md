# Live Agent View Snapshots Design

Date: 2026-06-04

## Purpose

Cull should let an MCP-connected agent understand the exact set of images the
user is looking at right now. The user should be able to press a shortcut, or an
agent should be able to request a fresh capture, and receive a visual artifact
plus a structured manifest that maps visible labels to stable Cull image IDs.

Primary workflow:

1. User opens Grid, Loupe, Compare, Canvas, or a fullscreen/zen variant.
2. User asks an agent something like "help me select the best five here".
3. Agent gets the current view snapshot, sends the annotated PNG to a
   multimodal model, and receives label choices.
4. Agent applies those labels back to Cull as current UI selection, ratings,
   decisions, or collection membership.

## Assumptions

- "Every image in my current view" means every image visible in the current
  viewport, not the entire loaded collection or all virtualized grid rows.
- "Pixel-perfect" means the visual capture must match the rendered app view.
  A hand-built contact sheet is not sufficient for this requirement.
- The agent needs both pixels and action targets. A screenshot alone is not
  enough because the agent must map visual choices back to image IDs.
- Local stdio MCP is the initial target. Remote HTTP access must be locked down
  because snapshots may expose private images, filenames, prompts, and UI state.
- The feature should not import snapshots into the library database. These are
  transient agent artifacts, not user artwork.

## Existing Context

- Frontend view state already lives in Svelte stores: `images`, `selectedIds`,
  `focusedIndex`, `viewMode`, `compareActiveSide`, `loupeScale`, `loupePanX`,
  `loupePanY`, `activeCanvas`, and scope stores.
- Grid renders only visible and overscanned cells via virtual positioning.
- Compare resolves two active images from selected IDs or focused plus next.
- Loupe resolves the focused image and has pan/zoom/overlay state.
- Canvas keeps live item geometry in component state and persists saved Canvas
  layouts through `canvases.layout_json`.
- MCP already exposes library, curation, display navigation, saved Canvas
  layout, and static publishing tools.
- The contact sheet exporter already demonstrates frontend canvas rendering
  plus a Rust `save_png_to_path` command, but it is a redraw, not a live view
  screenshot.

## Approaches Considered

### Approach A: Contact-Sheet Redraw

Build a custom canvas from `images`, selected IDs, and view mode, similar to the
existing contact sheet exporter.

Pros:

- Fast to implement.
- Easy to add numeric labels.
- Easy to test as pure layout logic.

Cons:

- Not pixel-perfect.
- Duplicates each view's layout rules.
- Hard to match Canvas transforms, Loupe overlays, Compare zen mode, scroll
  clipping, object boxes, NSFW blur, and future view UI.

This approach is useful as a fallback, but it does not satisfy the feature as
stated.

### Approach B: Native View Capture Plus Manifest

Capture the rendered app window or content area, then overlay labels using DOM
bounds gathered from the live view. Save both the raw PNG and annotated PNG,
with a manifest mapping labels to image IDs and actions.

Pros:

- Satisfies pixel-perfect visual capture.
- Avoids reimplementing Grid, Loupe, Compare, and Canvas rendering.
- Gives agents both vision input and stable action targets.
- Can preserve raw and annotated outputs separately.

Cons:

- Needs a platform capture provider, with macOS first and other platforms
  either implemented later or returning a clear unsupported error.
- Requires careful crop and device-pixel-ratio handling.
- Needs async request/response plumbing between MCP backend and live frontend.

This is the recommended approach.

### Approach C: MCP Metadata Only

Expose current image IDs, selected IDs, and view metadata to MCP without a PNG.

Pros:

- Simple and safe.
- Useful for non-vision automation.

Cons:

- Does not let a multimodal model judge the visible set.
- Cannot answer visual ranking questions from the current screen.

This should be part of the manifest, but not the whole feature.

## Recommended Design

Implement file-backed live view snapshots with two capture entry points:

- User action: command palette item plus default shortcut captures the current
  view and shows a toast with the destination.
- Agent action: MCP tool requests a fresh capture from the active frontend
  window, waits for completion, then returns the snapshot manifest.

The first implementation targets the main local window and local stdio MCP.
Remote HTTP clients receive a clear error in v1. Remote snapshot access is out
of scope for this spec because it needs a separate permission and privacy
design.

## Snapshot Package

Each capture creates a directory under app data:

```text
Agent Snapshots/
  <timestamp>-<short-id>/
    raw.png
    annotated.png
    manifest.json
```

The package is not imported into `cull.db`. It is treated as an app artifact.

Manifest schema:

```json
{
  "schema_version": 1,
  "snapshot_id": "snap_...",
  "created_at": "2026-06-04T09:00:00Z",
  "view_mode": "grid",
  "capture_reason": "shortcut",
  "destination": {
    "saved_locally": true,
    "copied_to_clipboard": false
  },
  "files": {
    "raw_png": "/path/to/raw.png",
    "annotated_png": "/path/to/annotated.png",
    "manifest_json": "/path/to/manifest.json"
  },
  "window": {
    "label": "main",
    "device_pixel_ratio": 2,
    "viewport_css": { "x": 0, "y": 0, "width": 1440, "height": 900 },
    "screenshot_px": { "width": 2880, "height": 1800 }
  },
  "scope": {
    "folder": null,
    "collection_id": null,
    "smart_collection_id": null,
    "session_id": null,
    "canvas_id": null
  },
  "visible_images": [
    {
      "label": 1,
      "image_id": "img_...",
      "filename": "example.png",
      "path": "/local/path/example.png",
      "thumbnail_path": "/local/path/thumb.png",
      "bounds_css": { "x": 24, "y": 92, "width": 160, "height": 160 },
      "bounds_px": { "x": 48, "y": 184, "width": 320, "height": 320 },
      "visible_ratio": 1.0,
      "focused": true,
      "selected": false,
      "rating": 4,
      "decision": "undecided",
      "view_role": "grid-cell"
    }
  ]
}
```

Remote-safe manifests must redact filesystem paths and omit local file URLs.

## Frontend Capture Contract

Add `src/lib/agent-view-snapshot.ts` for pure capture helpers:

- `collectVisibleImageTargets(context)` returns visible image targets with
  labels, image IDs, DOM bounds, state flags, and view roles.
- `buildAgentSnapshotManifest(context, files)` builds the JSON manifest.
- `drawAnnotatedSnapshot(rawPng, targets)` overlays numeric markers and optional
  focus/selection outlines.

The frontend is the authority for live view state because Rust cannot directly
read Svelte stores or transient Canvas component state.

Each view needs stable selectors or a small registration hook:

- Grid: `.grid-cell` and nested thumbnail image, filtered to viewport
  intersection.
- Compare: left/right panel image wrappers, roles `compare-left` and
  `compare-right`.
- Loupe/fullscreen: focused image element, role `loupe`.
- Canvas: `.canvas-item`, Canvas item ID, current transform, role `canvas-item`.

The capture should include only targets with meaningful intersection with the
main viewport. A threshold of 20 percent visible area avoids labeling cells that
are barely clipped at the edge.

## Backend Capture And Registry

Add a Rust service, `services::agent_snapshots`, with:

- Snapshot directory resolution under `app_data_dir`.
- Atomic package writes for `raw.png`, `annotated.png`, and `manifest.json`.
- In-memory latest snapshot pointer.
- JSON manifest validation and local/remote redaction helpers.
- Retention pruning that keeps only the latest 25 snapshot packages.

Add Tauri commands:

- `request_agent_view_snapshot(options)` emits an event to the main frontend and
  waits for a response with a timeout.
- `complete_agent_view_snapshot(request_id, manifest, raw_png_base64,
  annotated_png_base64, clipboard_mode)` writes files and stores the latest
  snapshot.
- `get_last_agent_view_snapshot(snapshot_id?)` returns the stored manifest.

The native screenshot provider should be isolated behind a trait:

```rust
trait WindowSnapshotProvider {
    fn capture_main_window_png(&self, app: &AppHandle) -> Result<Vec<u8>, String>;
}
```

The v1 provider is macOS-only and uses the platform screenshot utility.
Unsupported platforms return a clear error rather than falling back to a
misleading non-pixel-perfect image.

## Clipboard Behavior

The shortcut should save locally by default and show a toast:

```text
Agent snapshot saved
Agent Snapshots/<id>/annotated.png
```

Add a second command or option, `Capture Agent Snapshot to Clipboard`, that:

- Saves the same package locally.
- Copies the annotated PNG to the system clipboard when platform support exists.
- Copies a concise text fallback containing the snapshot ID and manifest path if
  image clipboard copy fails.
- Shows a toast that explicitly says whether the output went to the clipboard,
  local files, or both.

This avoids ambiguity and preserves a durable local package for MCP even when
clipboard access fails.

## MCP Tools

Add MCP tools:

- `capture_current_view_snapshot`:
  Requests a fresh frontend capture. Parameters: `annotated`, `include_base64`,
  `destination` (`local_file`, `clipboard`, `both`). Local stdio only in v1.

- `get_last_view_snapshot`:
  Returns the latest manifest, or a specific snapshot by ID. Local clients get
  file paths. Remote clients receive the same local-only error as capture.

- `select_snapshot_labels`:
  Parameters: `snapshot_id`, `labels`, `mode` (`replace`, `add`, `remove`,
  `toggle`), `focus_first`. Resolves labels through the manifest and updates
  frontend `selectedIds`.

- `select_images_in_view`:
  Parameters: `image_ids`, `mode`, `focus_first`. Updates frontend selection for
  loaded images after scope validation.

Tool capability mapping:

- Snapshot capture and latest snapshot access: `display:navigate` for local
  stdio, remote disabled in v1.
- Selection changes: `display:navigate` because it changes live UI state but not
  database curation fields.
- Rating, decisions, and collections continue to use existing curation tools.

## Agent Workflow

Example flow:

1. Agent calls `capture_current_view_snapshot`.
2. Agent submits `annotated.png` to a multimodal model with the manifest labels.
3. Model returns labels like `[3, 7, 9, 12, 15]`.
4. Agent calls `select_snapshot_labels` with those labels and `mode: replace`.
5. Agent optionally calls existing tools such as `set_decision` or
   `add_to_collection` for the resolved image IDs.

The label overlay should be high contrast, compact, and positioned inside each
image bounds near the top-left corner. It must not resize or shift the app UI
because it is drawn onto the exported annotated PNG, not into the live DOM.

## Error Handling

- No frontend window: return `Error: No active Cull window is available for live
  view capture`.
- Unsupported screenshot provider: return `Error: Pixel-perfect capture is not
  supported on this platform yet`.
- Timeout waiting for frontend: return `Error: Timed out waiting for the current
  view to render`.
- Empty view: create a manifest with no visible images and no annotated labels,
  then warn in the response.
- Clipboard failure: still save locally and show the clipboard failure detail.
- Remote HTTP request in v1: return `Error: live view snapshots are local-only`.

## Privacy And Safety

- Snapshot files are private app artifacts and should not be imported into the
  library.
- Remote HTTP clients cannot receive PNG bytes or local paths in v1.
- Remote-safe JSON redaction must reuse the existing MCP path redaction pattern.
- Snapshot manifests may contain prompts, filenames, and UI state. Keep prompt
  text out of the default manifest.
- Retain only the latest 25 snapshot packages. A Settings history UI is not part
  of v1.

## First Implementation Boundary

Slice 1 should deliver:

- Frontend visible target collection for Grid, Compare, Loupe, and Canvas.
- Manifest creation and unit tests.
- Local snapshot package writer.
- Command palette command plus default `Cmd+Shift+C` shortcut.
- MCP `get_last_view_snapshot`.
- MCP `select_snapshot_labels` and `select_images_in_view`.
- A macOS pixel-perfect capture provider using the platform screenshot utility
  behind the provider trait. If capture fails, return a clear error instead of
  a non-pixel-perfect fallback.

Slice 2 should add:

- MCP-triggered fresh capture with frontend request/response timeout.
- Clipboard image copy for annotated snapshots.
- Settings UI for snapshot history after v1 usage validates the need.
- Remote snapshot access as a separate privacy design.

This boundary makes the first slice useful for user-triggered local agent work
without overcommitting to remote screenshot semantics.

## Tests

Frontend unit tests:

- Grid target collection filters to visible viewport items and preserves labels.
- Compare target collection labels left/right consistently.
- Loupe target collection emits only the focused image.
- Canvas target collection includes Canvas item IDs and bounds.
- Manifest builder includes scope, selection, focus, rating, and decision fields.
- Label-to-image mapping rejects labels not present in the manifest.

Rust unit tests:

- Snapshot package writer creates expected files.
- Manifest path redaction removes local paths for remote contexts.
- MCP capability mapping covers new tools.
- `select_snapshot_labels` resolves labels deterministically and rejects stale
  or unknown snapshot IDs.
- Local-only enforcement rejects remote capture requests.

Browser/manual tests:

- Shortcut in Grid creates a package and toast with destination.
- Shortcut in Compare captures both panels and preserves active side.
- Shortcut in Loupe zen/fullscreen captures the image-only view.
- Shortcut in Canvas captures visible arranged items.
- Agent flow can select five labeled images and update the current UI
  selection.

## Implementation Decisions

- Use `Cmd+Shift+C` as the default shortcut because `Cmd+C` already copies the
  focused image and Shift naturally maps to "copy the current view for an
  agent". The command remains available through palette hotkey customization.
- Use the macOS screenshot utility behind `WindowSnapshotProvider` for v1
  because it is lower integration risk than direct Cocoa/CoreGraphics capture.
  The provider boundary keeps direct capture available as a later replacement.
- Do not build snapshot history UI in v1. Keep the latest 25 packages and expose
  only the latest snapshot through MCP unless a specific snapshot ID is given.
