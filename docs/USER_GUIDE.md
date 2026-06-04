# Cull User Guide

Cull is a local-first desktop image viewer for reviewing, comparing, curating, searching, monitoring, and exporting AI-generated image sets. The app stores its library metadata in SQLite and never modifies original image files during normal viewing or curation.

## Install And Run From Source

Prerequisites:

- macOS
- Node.js 20+
- Rust stable

```bash
git clone https://github.com/glebis/cull.git
cd cull
npm install
npm run tauri dev
```

For a production build:

```bash
npm run tauri build
```

## Import Images

Use **File > Open File...** to import one or more images. A single imported image opens in Loupe; multiple imported images stay in the library grid.

Use **File > Open Folder...** to import a folder. Cull scans recursively and switches the active scope to that folder.

Supported import formats are JPEG, PNG, WebP, GIF, BMP, TIFF, and ICO. On macOS, Cull also decodes HEIC/HEIF, SVG, AVIF, JPEG XL, and PSD through ImageIO for thumbnails and analysis. RAW preview import supports CR2, CR3, NEF, ARW, DNG, ORF, RAF, and RW2 when the RAW module is enabled.

## Navigate Views

Cull has seven primary views:

| Shortcut | View | Purpose |
|---|---|---|
| `Cmd+1` | Grid | Browse many images quickly |
| `Cmd+2` | Loupe | Inspect one image with zoom and pan |
| `Cmd+3` | Compare | Review two images side by side |
| `Cmd+4` | Canvas | Arrange selected images spatially |
| `Cmd+5` | Lineage | Inspect generation runs and image relationships |
| `Cmd+6` | Embedding Explorer | Explore visual clusters and similarity |
| `Cmd+7` | Export | Build exports from selected or collected images |

The same view shortcuts are used by the tab bar, keyboard handler, and native View menu.

## Command Palette

Use `Cmd+P` or **View > Command Palette...** to open the Command Palette in command-only mode. Use `Cmd+K` to open the broader palette that includes commands and destinations such as folders, collections, and smart collections. `Cmd+Shift+P` is also supported as a command-only shortcut for VS Code-style muscle memory.

Type to fuzzy-search the available commands. Non-matching rows are hidden while you search, rows show their category and shortcut, `ArrowUp` and `ArrowDown` move the selected row, and `Enter` runs the selected command. `Escape` closes the palette.

The palette keeps the last five commands launched through the palette at the top of the empty command list, matching standard command palette behavior in tools like VS Code and Obsidian.

- **Destinations**: jump to All Images, smart collections, manual collections, folders, sessions, the active session's canvases, and detected object classes. Selecting a destination clears conflicting scopes and reloads images, exactly like the sidebar.
- **Ranking**: pinned (favorited) results come first, then the best query match, then your most recent and most frequently used commands. Frequent commands rise over time.
- **Result actions**: right-click a row (or press `Shift+F10`) for Run, Favorite/Unfavorite, Set Hotkey…, Add Alias…, Remove from Recents, Copy Command ID, and Open in Settings. Aliases add your own search terms to a command.
- **Keyboard shortcuts**: run **View Keyboard Shortcuts** to browse every command's binding, search them, customize a hotkey (with conflict and reserved-key detection), or reset everything to defaults.
- **Workflows**: run **Save Workflow from Recent Commands** to capture your recent command sequence as a reusable workflow. Saved workflows appear in the palette as runnable, favoritable items; rename or delete them from the row context menu. Workflows validate each step's context before running and confirm destructive steps.

## Preview Display

Use **View > Preview Display** or `Cmd+Shift+D` to open a separate display window. The main Cull window remains the control surface while Preview Display shows the current focused image on another monitor or an iPad used as a macOS Sidecar display.

Use **View > Move Preview Display to Display...** and **View > Fullscreen Preview Display** for external monitor placement. Use **Freeze Preview Display** to hold the current image while navigating privately, and **Blank Preview Display** to hide the image without changing library data.

Use the Preview Display preset, field, and rail controls in the View menu to choose whether the external viewer sees only the image, review status, metadata, prompt/tags, or the RGB/luma histogram. Rail side, width, and text size are bounded so long prompt and tag text truncates instead of overlapping the image.

Use **View > Start Preview Display Web Stream** for a tokenized localhost preview URL. For an iPad or browser on the local network, use **View > Start Preview Display LAN Web Stream**. Cull copies the live URL; stop it with **View > Stop Preview Display Web Stream**. Treat the URL as a secret because anyone on the reachable network with the full URL can view the streamed preview.

More details and limitations are in [Preview Display](preview-display.md).

## Review And Curate

Use the grid or loupe to move through images and apply lightweight curation metadata:

| Shortcut | Action |
|---|---|
| `h`, `j`, `k`, `l` or arrow keys | Move focus |
| `Space` | Toggle selection |
| `s` then `1`-`5` | Set star rating |
| `0` | Clear rating |
| `a` | Accept |
| `x` | Reject |
| `u` | Mark undecided |
| `Backspace` | Move focused image to Trash |
| `Cmd+Z` | Undo supported library actions |
| `Cmd+Shift+Z` | Redo supported library actions |

Selection, ratings, and decisions are stored in the local Cull database. Originals are not rewritten.

Loupe also shows a compact RGB/luma histogram overlay for the focused image. When the luma edge bins indicate likely clipping, Cull labels clipped shadows and highlights directly in the overlay.

### Best Of Group

After generating similarity groups in the Embedding Explorer, run **Best of Group Ranking…** from the palette. Each group shows a suggested winner with an explainable breakdown — rating, decision, focus quality, and representativeness — and you can override the winner per group. Nothing is ever auto-deleted or auto-selected; the suggestion is advisory.

### Client Feedback

Client feedback is stored **separately** from your own curator ratings and decisions, so the two never overwrite each other:

- **Toggle Client Favorite** marks the focused image as a client pick.
- **Add Client Comment…** attaches a client note to the focused image.

Both are surfaced alongside curator data in the delivery CSV export.

## Collections

Collections let you group images without copying source files.

- Use Collect mode (`b`) to add focused images to a target collection.
- Use the sidebar to browse collections.
- Use Export view to prepare a collection for output.

## Clipboard Monitor

Clipboard Monitor is for reference-gathering sessions where images arrive through the macOS clipboard, for example from a browser, generator, or design tool.

- Use the sidebar **Monitor Clipboard** control to start or stop monitoring.
- Starting a monitor session creates a new collection named like `Clipboard 2026.05.30 14:35`, focuses the grid on that collection, and stores each changed clipboard image as a real file.
- Captures use the default `Clipboard Captures` folder inside Cull's app data folder unless you choose **Move Folder**.
- The sidebar shows the access status, active capture folder, collection name, and captured image count.
- Use **Publish** to publish the clipboard collection as a static reference set and copy the published URL when available.

Clipboard access is explicit. Stop the monitor when the reference-gathering session is finished.

## Static Publishing

Static publishing turns a collection into a local shareable site without changing the original source images.

- Use the static publishing controls from Export view for regular collections.
- Use **Publish** in Clipboard Monitor to publish the current clipboard collection directly.
- Published sites include a manifest, image assets, and agent instructions so a recipient or agent can inspect the reference set without opening the Cull database.

## Export Images

Use the Export view (`Cmd+7`) to prepare selected images or a collection for output. Three export workflows are also available directly from the command palette:

- **Export to Folder…** — export the current scope (selection, collection, folder, or whole library) to a destination folder with optional format conversion (`original`, `png`, `jpg`, `webp`) and a filename naming template. Templates support `{name}`, `{id}`, `{index}`, and `{index1}` tokens, or the preset keywords `original`, `id`, and `index`.
- **Export Contact Sheet…** — render a configurable grid montage (columns, cell size) of the current images to a PNG, with per-cell captions for filename, star rating, and dimensions.
- **Export Delivery List (CSV)…** — write a CSV of the current scope with separate columns for curator rating/decision and client favorite/comment, suitable for proof and final delivery lists.

The same folder export is available from the CLI slice:

```bash
cull --json export_images --collection_id <id> --output_dir ~/Desktop/export --format original
```

Export operations write new files to the chosen destination and do not rewrite your source images.

## Embeddings And Search

Cull supports local CLIP embeddings through ONNX Runtime. Local embeddings run on your machine and do not send images to an external service.

Run **Find Similar to Focused Image** from the palette (or right-click → Find Similar) to replace the current view with the images most visually similar to the focused one, ranked by embedding similarity.

Optional cloud embedding and generation providers require your own API key. Keys are stored in the OS keychain where supported. See [Privacy & Data Flow](PRIVACY.md) before enabling cloud features for sensitive images.

## Agent And MCP Workflows

Cull exposes an agent-friendly workflow surface through MCP-aligned commands and menu actions.

- The tray menu includes window, Clipboard Monitor, MCP status, and quit actions.
- MCP tools can inspect library state, work with collections, and publish clipboard monitor collections when the server is enabled.
- The MCP HTTP server is disabled by default; enable it only when you need remote agent access.

## CLI

Cull includes an MCP-aligned headless CLI slice for agent workflows:

```bash
cull --json get_library_stats
cull --json list_images --limit 20 --offset 0
cull --json import_folder --folder_path ~/renders
cull --json import_files --file_paths ~/renders/a.png,~/renders/b.png
cull --json export_images --collection_id <id> --output_dir ~/Desktop/export --format original
```

The CLI command names and JSON fields intentionally match MCP tool naming where possible. See [Agent CLI Standards](agent-cli-standards.md).

## Native Menus

The complete menu inventory is maintained in [System Menu Audit](SYSTEM_MENU_AUDIT.md). If a shortcut and menu item disagree, treat that as a bug. Use **Help > Cull User Guide** to reopen this guide from the app.

## Privacy Defaults

By default, Cull is local-first:

- Image viewing, thumbnails, ratings, collections, and SQLite metadata are local.
- Local CLIP embeddings use ONNX Runtime on your machine.
- Clipboard Monitor writes captured clipboard images to your selected local capture folder.
- Cloud providers are opt-in and require your own API key.
- The MCP HTTP server is disabled unless explicitly enabled.

See [Privacy & Data Flow](PRIVACY.md) for provider-specific details.
