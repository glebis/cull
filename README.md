# ImageView

Agent-friendly AI image viewer for macOS.

Terminal-inspired, keyboard-first image viewer built for AI-generated image workflows. Handles batch review, comparison, curation, iteration, and export of hundreds to tens of thousands of images. Full bidirectional agent integration via MCP.

## Features

- **7 view modes** — Grid, Compare, Loupe, Canvas, Lineage, Embedding Explorer, Export
- **Keyboard-first** — vim-style navigation, every action reachable without mouse
- **Agent integration** — MCP server for bidirectional communication with Claude Code and other agents
- **Smart library** — SQLite-backed with SHA-256 dedup, project-scoped curation, iteration tracking
- **Object detection** — YOLO via ONNX Runtime, auto-tagging on import
- **Embedding search** — CLIP embeddings, semantic similarity, UMAP visualization
- **Export anything** — WYSIWYG export of any view state as PNG/PDF/SVG contact sheets, film strips, comparison panels

## Tech Stack

Tauri 2 + Rust + Svelte + SQLite + ONNX Runtime

## Quick Start

```
git clone https://github.com/gastownhall/imageview.git
cd imageview
npm install
npm run tauri dev
```

Prerequisites: Rust 1.78+, Node.js 20+

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

### Actions

| Key | Action |
|-----|--------|
| `s` then `1-5` | Star rating |
| `0` | Clear rating |
| `a` / `x` | Accept / reject |
| `u` | Clear decision (undecide) |
| `Escape` | Cancel pending action |

## Roadmap

- [x] Phase 1 — Grid view, import, keyboard navigation, dark terminal theme
- [ ] Phase 2 — Compare, Loupe, Canvas, Lineage views
- [ ] Phase 3 — ONNX + CLIP + YOLO + Embedding Explorer
- [ ] Phase 4 — MCP server, CLI, AppleScript, global shortcuts
- [ ] Phase 5 — Image generation integration, WYSIWYG export

## Design

Dark terminal aesthetic. Monospace typography. 8px spacing grid. No decorative elements.

See `docs/superpowers/specs/2026-05-06-imageview-design.md` for the full design spec.

## License

Business Source License 1.1. Non-commercial use is permitted. Commercial use requires a separate license. Converts to Apache 2.0 on 2030-05-07. See [LICENSE.md](LICENSE.md).
