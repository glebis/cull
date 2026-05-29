# Cull

Local-first, keyboard-first review and curation for AI-generated image workflows on macOS.

Official site: [cull.company](https://cull.company/)

Terminal-inspired image viewer for batch review, comparison, curation, iteration, and export of hundreds to tens of thousands of images. Agent integration is built in through MCP, an MCP-aligned headless CLI slice, and the `cull://` URL scheme.

## What's Different

- **Keyboard-first** — vim-style navigation, every action reachable without mouse
- **AI-native** — CLIP/DINOv2 embeddings, UMAP visualization, visual similarity, and user-supplied local detection models
- **Agent-friendly** — local MCP server, headless CLI tools, and deep links share the same command model
- **macOS-integrated** — Open With, drag-and-drop, native menus, outbound Share sheet, Reveal in Finder
- **Non-destructive** — SQLite library with SHA-256 dedup; originals are never modified

## Tech Stack

Tauri 2 + Rust + Svelte 5 + SQLite + ONNX Runtime

## Documentation

- [User Guide](docs/USER_GUIDE.md) — installation, import, viewing, curation, collections, embeddings, export, CLI, and privacy basics
- [System Menu Audit](docs/SYSTEM_MENU_AUDIT.md) — complete native menu inventory and implementation status
- [Privacy & Data Flow](docs/PRIVACY.md) — local-first behavior, opt-in cloud features, and data destinations
- [Contributing](CONTRIBUTING.md) — setup, checks, and pull request process
- [Distribution](docs/cross-platform-distribution.md) — release builds and platform planning

## Current Status (v0.1.0)

### Implemented

| Feature | Status |
|---------|--------|
| Grid view with 4 thumbnail presets (80-400px) | Done |
| Loupe view with zoom/pan (up to 20x) | Done |
| Compare view (side-by-side pairs) | Done |
| Canvas view with freeform placement, resize, crop, rotate, and notes | Done |
| Lineage view with timeline and comparison layouts | Done |
| Embedding Explorer (UMAP 2D scatter + k-means clustering) | Done |
| Zen mode (fullscreen, hides all chrome) | Done |
| Vim-style keyboard navigation (hjkl, arrows, Home/End, PgUp/PgDn) | Done |
| Star ratings (1-5), accept/reject/undecide curation | Done |
| Color labels (schema ready, UI pending) | Partial |
| SQLite library with SHA-256 dedup | Done |
| Recursive folder import | Done |
| Thumbnail generation (Lanczos3) | Done |
| Folder and size filtering | Done |
| Collections (create, add, list, delete, collect-mode) | Done |
| Local embedding models (CLIP ViT-B/32, DINOv2 ViT-S/14) | Done |
| Cloud/local text and multimodal embeddings (Gemini, Cohere, OpenAI, Ollama) | Done |
| Model download UX with progress, pause/resume, cancel, and manual commands | Done |
| Cosine similarity search | Done |
| UMAP visualization with auto-clustering | Done |
| User-supplied YOLO/NudeNet-compatible detection model controls | Partial |
| Image quality metrics, color metrics, and perceptual hashing | Done |
| Deep link URL scheme (`cull://`) | Done |
| MCP server over local stdio/socket plus optional token-authenticated HTTP | Done |
| MCP-aligned headless CLI slice | Done |
| Native macOS menu bar (File/Edit/View/Window/Help) | Done |
| Drag-and-drop from Finder | Done |
| File type associations (Open With in Finder) | Done |
| Share sheet integration | Done |
| Reveal in Finder | Done |
| Single-instance with deep link forwarding | Done |
| Multi-window support | Done |
| Static Canvas publishing package and local static server | Done |
| Dark terminal aesthetic | Done |

### Supported Formats

Currently: **JPEG, PNG, WebP, GIF, BMP, TIFF, ICO**, plus **HEIC/HEIF, SVG, AVIF, JPEG XL, and PSD on macOS via ImageIO**. RAW preview import is available for CR2, CR3, NEF, ARW, DNG, ORF, RAF, and RW2 when the RAW module is enabled.

Remaining RAW work: full metadata extraction and non-preview decode for camera formats.

### Agent CLI

Cull has an initial MCP-aligned headless CLI slice. Commands use MCP tool names and JSON parameter field names so agents can reuse the same mental model across CLI and MCP:

```bash
cull --json get_library_stats
cull --json import_folder --folder_path ~/renders
cull --json import_files --file_paths ~/renders/a.png,~/renders/b.png
cull --json export_images --collection_id <id> --output_dir ~/Desktop/export --format original
cull --json call_tool import_folder --params_json '{"folder_path":"/Users/me/renders"}'
```

Implemented headless tools: `get_library_stats`, `list_images`, `list_folders`, `list_collections`, `import_folder`, `import_files`, `get_embedding_model_download_info`, `download_embedding_model`, `generate_embeddings`, `analyze_image_quality`, `get_image_quality`, `get_quality_count`, `list_export_presets`, `export_images`.

CLI module and output standards live in [docs/agent-cli-standards.md](docs/agent-cli-standards.md).

## Roadmap

`bd` is the tracker of record. This README lists only the major remaining product gaps.

### P1 — Power user features

**OS Integration:**
- Quick Look extension (Spacebar preview in Finder)
- URL scheme expansion beyond current open/import/view parameters: export, search, contact-sheet, resize

**Viewing & Metadata:**
- EXIF/IPTC/XMP metadata display panel
- Histogram with per-channel RGB and clipping warnings
- CLIP text-to-image search
- Semantic similarity search UI for visually similar images
- Expanded EXIF/XMP generation metadata coverage beyond current C2PA, PNG text, sidecar, and source-detection coverage

**Automation & CLI:**
- Full CLI parity for search, detection, rating, conversion, and batch operations
- Batch operations pipeline: resize, convert, rename, watermark, export
- Contact sheet export with labels, ratings, and metadata

**AI & Detection:**
- Automatic tagging on import when a compatible local detection model is installed
- Florence-2 integration for zero-shot detection and captioning

**Embedding Explorer:**
- Scope projection to current filter/folder/collection
- Nearest-neighbor panel with similarity scores
- Lasso/rectangle selection to create collections
- Clickable clusters with representative thumbnails
- Cluster auto-naming

**Export:**
- Collection export with format conversion and naming templates
- WYSIWYG export for arbitrary view state as PNG/PDF

### P2 — Full platform

- Services menu integration ("Open in Cull" system-wide)
- AppleScript / Apple Events support
- Color management (ICC profiles, monitor matching)
- Print support with layout options
- Global keyboard shortcuts (system-wide hotkeys)
- InsightFace face detection and grouping by person
- PaddleOCR text extraction for search
- In-app inpainting (select region, type prompt, call API)

## CLI Full-Parity Target

Every GUI operation will have a CLI equivalent:

```bash
cull ~/photos                              # open folder in GUI
cull contact-sheet ./shoot --columns 6     # generate contact sheet
cull search "sunset landscape" --top 20    # semantic search
cull similar photo.jpg --top 10            # find visually similar
cull export favorites --format webp        # batch export collection
cull metadata photo.jpg                    # dump EXIF as JSON
find . -name "*.png" | cull pipe --resize 800x0 --format webp
```

See [docs/cli-and-url-scheme.md](docs/cli-and-url-scheme.md) for the full specification.

## Current URL Scheme

```
cull://open?path=/path/to/image.jpg&view=loupe
cull://import?folder=/path/to/photos
cull://grid?folder=/path/to/photos&size=240
cull://loupe?image_id=<image-id>&zoom=200
```

Search, contact-sheet, and export deep-link verbs are roadmap items.

## Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `h j k l` | Move in grid |
| `Arrow keys` | Move in grid |
| `Home` / `End` | First / last image |
| `PageUp` / `PageDown` | Jump by visible rows |
| `Space` | Toggle select |
| `+` / `-` | Adjust thumbnail size |
| `g` | Cycle grid presets |
| `f` | Fullscreen (Loupe) |
| `\` or `Cmd+B` | Toggle sidebar |

### View Modes

| Key | View |
|-----|------|
| `Cmd+1` | Grid |
| `Cmd+2` | Loupe |
| `Cmd+3` | Compare |
| `Cmd+4` | Canvas |
| `Cmd+5` | Lineage |
| `Cmd+6` | Embedding Explorer |
| `Cmd+0` | Export |

### Curation

| Key | Action |
|-----|--------|
| `s` then `1-5` | Star rating |
| `0` | Clear rating |
| `a` / `x` / `u` | Accept / reject / undecide |
| `b` | Collect mode (add to collection) |

## Quick Start

```bash
git clone https://github.com/glebis/cull.git
cd cull
npm install
npm run tauri dev
```

Prerequisites: Rust 1.78+, Node.js 20+

## Distribution

See [docs/cross-platform-distribution.md](docs/cross-platform-distribution.md) for platform support, release builds, CI matrix planning, and smoke-test checklists.

## Design

Dark terminal aesthetic. Monospace typography. 8px spacing grid. No decorative elements.

See `docs/design-system.md` for the full design spec.

## License

Apache License 2.0. See [LICENSE.md](LICENSE.md).

Cull is open source. Third-party dependencies, optional model weights, and example
assets keep their own licenses; see [NOTICE](NOTICE) and
[docs/OPEN_SOURCE_AUDIT.md](docs/OPEN_SOURCE_AUDIT.md) for the current transition
audit.
