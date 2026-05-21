# Cull

Local-first, agent-friendly AI image viewer for macOS. Built to replace Preview.app for power users.

Terminal-inspired, keyboard-first image viewer for AI-generated image workflows. Handles batch review, comparison, curation, iteration, and export of hundreds to tens of thousands of images. Full bidirectional agent integration via MCP, CLI, and URL scheme.

## What's Different

- **Keyboard-first** — vim-style navigation, every action reachable without mouse
- **AI-native** — CLIP embeddings, UMAP visualization, semantic search, YOLO detection
- **Agent-friendly** — every GUI operation has a CLI equivalent; composable pipelines via stdin/stdout
- **macOS-integrated** — Open With, drag-and-drop, native menus, Share sheet, Reveal in Finder
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
| CLIP ViT-B/32 embeddings via ONNX Runtime | Done |
| Gemini Embedding 2 via API | Done |
| Cosine similarity search | Done |
| UMAP visualization with auto-clustering | Done |
| Deep link URL scheme (`cull://`) | Done |
| Native macOS menu bar (File/Edit/View/Window/Help) | Done |
| Drag-and-drop from Finder | Done |
| File type associations (Open With in Finder) | Done |
| Share sheet integration | Done |
| Reveal in Finder | Done |
| Single-instance with deep link forwarding | Done |
| Multi-window support | Done |
| Dark terminal aesthetic | Done |

### Supported Formats

Currently: **JPEG, PNG, WebP, GIF, BMP, TIFF, ICO**, plus **HEIC/HEIF, SVG, AVIF, JPEG XL, and PSD on macOS via ImageIO**. RAW preview import is available for CR2, CR3, NEF, ARW, DNG, ORF, RAF, and RW2 when the RAW module is enabled.

Still planned: full metadata extraction and non-preview RAW decode for camera formats.

### Agent CLI

Cull has an initial MCP-aligned headless CLI slice. Commands use MCP tool names and JSON parameter field names so agents can reuse the same mental model across CLI and MCP:

```bash
cull --json get_library_stats
cull --json import_folder --folder_path ~/renders
cull --json import_files --file_paths ~/renders/a.png,~/renders/b.png
cull --json export_images --collection_id <id> --output_dir ~/Desktop/export --format original
cull --json call_tool import_folder --params_json '{"folder_path":"/Users/me/renders"}'
```

Implemented headless tools: `get_library_stats`, `list_images`, `list_folders`, `list_collections`, `import_folder`, `import_files`, `list_export_presets`, `export_images`.

CLI module and output standards live in [docs/agent-cli-standards.md](docs/agent-cli-standards.md).

## Roadmap

### P0 — Must-have for daily driver

- [x] **File type associations** — CFBundleDocumentTypes so app appears in Finder "Open With"
- [x] **Drag and drop from Finder** — drop files/folders onto the window to import
- [x] **Native macOS menu bar** — File > Open, Open Folder, Edit, View, Window, Help
- [x] **Broad format support** — HEIC, TIFF, BMP, SVG, AVIF, JPEG XL, RAW preview formats
- [ ] **Model download UX** — progress bar, pause/resume, manual download option

### P1 — Power user features

**OS Integration:**
- [ ] Quick Look extension (Spacebar preview in Finder)
- [ ] URL scheme expansion (action verbs: import, export, search, contact-sheet)

**Viewing & Metadata:**
- [ ] EXIF/IPTC/XMP metadata display panel
- [ ] Histogram with per-channel RGB and clipping warnings
- [ ] CLIP text-to-image search
- [ ] Semantic similarity search (find visually similar images)
- [ ] AI generation metadata parsing (prompt, seed, model from PNG/EXIF)

**Automation & CLI:**
- [ ] CLI tool — headless access to import, search, export, detect, rate, convert. Initial MCP-aligned import/export/listing slice is implemented.
- [ ] MCP server (stdio) — expose all functionality to agents
- [ ] Batch operations pipeline — composable resize, convert, rename, watermark, export
- [ ] Contact sheet export — configurable grid with labels, ratings, metadata

**AI & Detection:**
- [ ] YOLO object detection via ONNX Runtime — auto-tagging on import
- [ ] Florence-2 integration — zero-shot detection + captioning
- [ ] Multi-model embedding support (Google, Ollama, OpenAI, local ONNX)

**Embedding Explorer:**
- [ ] Scope to current filter/folder/collection
- [ ] UMAP in web worker (avoid UI freeze at 10K+ images)
- [ ] Nearest-neighbor panel with similarity scores
- [ ] Lasso/rectangle selection to create collections
- [ ] Clickable clusters with representative thumbnails
- [ ] Cluster auto-naming

**Export:**
- [ ] Collection export with format conversion and naming templates
- [ ] WYSIWYG export — capture any view state as PNG/PDF

### P2 — Full platform

- [ ] Services menu integration ("Open in Cull" system-wide)
- [ ] AppleScript / Apple Events support
- [ ] Color management (ICC profiles, monitor matching)
- [ ] Print support with layout options
- [ ] Global keyboard shortcuts (system-wide hotkeys)
- [ ] Canvas view — infinite spatial canvas with freeform placement
- [ ] Lineage view — iteration tree with prompt diffs
- [ ] InsightFace — face detection and grouping by person
- [ ] PaddleOCR — text extraction for search
- [ ] DINOv2 embeddings (alternative to CLIP)
- [ ] Color palette extraction (k-means dominant colors)
- [ ] In-app inpainting (select region, type prompt, call API)
- [ ] Perceptual hashing for near-duplicate detection
- [ ] Basic image stats on import (aspect ratio, transparency, bit depth)

## CLI (Planned Full Parity)

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

## URL Scheme

```
cull://open?path=/path/to/image.jpg&view=loupe
cull://import?folder=/path/to/photos&recursive=true
cull://search?q=sunset+landscape&view=grid
cull://contact-sheet?folder=./photos&columns=4&output=/tmp/sheet.png
```

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
git clone https://github.com/glebis/imageview.git
cd imageview
npm install
npm run tauri dev
```

Prerequisites: Rust 1.78+, Node.js 20+

## Distribution

See [docs/cross-platform-distribution.md](docs/cross-platform-distribution.md) for platform support, release builds, CI matrix planning, and smoke-test checklists.

## Design

Dark terminal aesthetic. Monospace typography. 8px spacing grid. No decorative elements.

See `docs/design-system.md` for the full design spec.

## Source License

Business Source License 1.1. Non-commercial use is permitted. Commercial use requires a separate license. Converts to Apache 2.0 on 2030-05-07. See [LICENSE.md](LICENSE.md).

This is a source-available license, not an OSI-approved open-source license. If the release goal is strict open source, choose an OSI-approved license before publishing.
