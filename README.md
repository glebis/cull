# Cull

Local-first, keyboard-first review and curation for AI-generated image workflows on macOS.

Official site: [cull.company](https://cull.company/)

Cull is a local-first reference engine for AI creativity. It helps humans and agents turn visual chaos into coherent projects, publications, posts, collections, and reusable creative memory.

It is image-first today: a desktop app for reviewing, comparing, curating, searching, exporting, and publishing large batches of generated images and visual references. Originals stay on your machine and are not modified.

## What It Is For

- **Culling large runs** — go from hundreds of generated images, sketches, photos, or screenshots to a final set of keepers.
- **Visual reference work** — build a local Pinterest-like archive for serious art direction, design research, and image memory.
- **Social and editorial output** — organize batches into Instagram posts, LinkedIn posts, books, magazines, essays, exhibitions, field guides, and client selects.
- **Agent-assisted creative workflows** — let an agent search, inspect, curate, compose exports, or publish reference sets through CLI and MCP surfaces.
- **Second-screen review** — keep the main app private while a client, collaborator, projector, iPad, or browser sees only the current preview.

## What It Does

- Import folders recursively, deduplicate by SHA-256, and keep a local SQLite library.
- Review in Grid, Loupe, Compare, Canvas, Lineage, Embedding Explorer, Export, and Speed Review views.
- Rate, accept, reject, undecide, collect, filter, and export without leaving the keyboard.
- Generate thumbnails, histograms, image quality metrics, perceptual hashes, and local CLIP/DINOv2 embeddings.
- Search by visual similarity and explore clusters with UMAP.
- Read generation metadata from sidecar JSON, PNG text chunks, C2PA-style metadata, and filename evidence where available.
- Export images, contact sheets, CSV delivery lists, social/editorial slides, PDFs, and static reference packages.
- Expose agent workflows through a local MCP server, MCP-aligned headless CLI commands, and `cull://` deep links.

## Principles

- **Local-first:** the library lives on your machine.
- **Non-destructive:** source files are not rewritten.
- **Keyboard-first:** common review actions are reachable without the mouse.
- **Agent-native:** CLI and MCP use the same task model so agents can operate the archive directly.
- **Open source:** Cull is Apache-2.0 software, not source-available software.
- **European-minded:** cloud/API features are explicit and optional; privacy, autonomy, and auditability matter.

## Download & Install

No developer tools required:

1. Download the latest `.dmg` for your Mac from [GitHub Releases](https://github.com/glebis/cull/releases).
2. Open the DMG and drag **Cull** into **Applications**.
3. First launch: current builds are not yet signed/notarized, so macOS Gatekeeper will warn. Right-click the app, choose **Open**, then confirm. You can also allow it under **System Settings -> Privacy & Security**.

Prefer building from source? See [Development](#development).

## Documentation

- [User Guide](docs/USER_GUIDE.md) — installation, import, review, curation, export, publishing, embeddings, preview display, and privacy basics
- [The Agent Surface](docs/agents.md) — headless CLI, MCP, scoped tokens, audit log, and agent-snapshot workflow
- [Preview Display](docs/preview-display.md) — external monitor, Sidecar, and tokenized local web preview
- [Privacy & Data Flow](docs/PRIVACY.md) — local-first behavior, optional cloud features, and provider boundaries
- [Changelog](CHANGELOG.md) — release-by-release notable changes
- [Contributing](CONTRIBUTING.md) — setup, checks, issue workflow, and pull request process

## Current Status

The shipping version is whatever the [latest GitHub Release](https://github.com/glebis/cull/releases/latest) says.

`bd` is the tracker of record for open work. The README intentionally does not duplicate the full roadmap, command list, or keyboard shortcut reference; those live in the docs and changelog.

Current high-level shape:

- macOS desktop app is the primary supported product.
- Windows and Linux distribution are planned, but not the main release target yet.
- MCP and CLI surfaces are usable and expanding, with explicit approval boundaries for destructive or sensitive operations.
- Optional model weights, cloud APIs, and third-party assets keep their own license and privacy boundaries.

## Agent CLI

Cull has an MCP-aligned headless CLI slice. Command names and JSON fields mirror the MCP tool model where possible:

```bash
cull --json get_library_stats
cull --json import_folder --folder_path ~/renders
cull --json import_files --file_paths ~/renders/a.png,~/renders/b.png
cull --json export_images --collection_id <id> --output_dir ~/Desktop/export --format original
cull --json call_tool import_folder --params_json '{"folder_path":"/Users/me/renders"}'
```

See [The Agent Surface](docs/agents.md), [Agent CLI Standards](docs/agent-cli-standards.md), and [CLI and URL Scheme](docs/cli-and-url-scheme.md) for the longer reference.

## Development

```bash
git clone https://github.com/glebis/cull.git
cd cull
npm install
npm run tauri dev
```

Prerequisites: Rust 1.89.0 and Node.js 20.20.2. The repository pins these versions in `rust-toolchain.toml`, `.node-version`, and `.nvmrc`; see [Toolchain](docs/toolchain.md).

Useful checks:

```bash
npm run preflight -- hook
npm run preflight -- quick
npm run preflight -- full
npm run audit:licenses
```

## Tech Stack

Tauri 2, Rust, Svelte 5, SQLite, ONNX Runtime, and a Tokyo Night-inspired interface.

## License And Copyright

Copyright 2026-present Gleb Kalinin.

Cull is licensed under the Apache License 2.0. See [LICENSE.md](LICENSE.md) and [NOTICE](NOTICE).

Implementation was assisted by AI coding tools under human architectural, product, and code-review direction. See [AUTHORSHIP.md](AUTHORSHIP.md) for the current authorship record.

Third-party dependencies, optional model weights, fonts, artwork, and example assets keep their own licenses. The current open-source and model-license record is in [docs/OPEN_SOURCE_AUDIT.md](docs/OPEN_SOURCE_AUDIT.md).
