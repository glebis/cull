# Cull User Guide

Cull is a local-first desktop image viewer for reviewing, comparing, curating, searching, and exporting AI-generated image sets. The app stores its library metadata in SQLite and never modifies original image files during normal viewing or curation.

## Install And Run From Source

Prerequisites:

- macOS
- Node.js 20+
- Rust stable

```bash
git clone https://github.com/glebis/imageview.git
cd imageview
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

Supported viewing formats in the current app are JPEG, PNG, WebP, and GIF. Additional file associations are registered for future format support, but some formats are still under development.

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
| `Cmd+0` | Export | Build exports from selected or collected images |

The same view shortcuts are used by the tab bar, keyboard handler, and native View menu.

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

## Collections

Collections let you group images without copying source files.

- Use Collect mode (`b`) to add focused images to a target collection.
- Use the sidebar to browse collections.
- Use Export view to prepare a collection for output.

## Embeddings And Search

Cull supports local CLIP embeddings through ONNX Runtime. Local embeddings run on your machine and do not send images to an external service.

Optional cloud embedding and generation providers require your own API key. Keys are stored in the OS keychain where supported. See [Privacy & Data Flow](PRIVACY.md) before enabling cloud features for sensitive images.

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

The complete menu inventory is maintained in [System Menu Audit](SYSTEM_MENU_AUDIT.md). If a shortcut and menu item disagree, treat that as a bug.

## Privacy Defaults

By default, Cull is local-first:

- Image viewing, thumbnails, ratings, collections, and SQLite metadata are local.
- Local CLIP embeddings use ONNX Runtime on your machine.
- Cloud providers are opt-in and require your own API key.
- The MCP HTTP server is disabled unless explicitly enabled.

See [Privacy & Data Flow](PRIVACY.md) for provider-specific details.
