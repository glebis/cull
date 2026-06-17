# Cull

Fast local image review for artists, photographers, and people who produce at volume on macOS.

Official site: [cull.company](https://cull.company/)

Cull is a fast image review tool for people who shoot, generate, or produce at volume. Go from hundreds of images, sketches, SD-card dumps, or test shots to a final set without uploading your work or fighting a heavy creative suite.

Most tools are built for editing. Cull is built for the moment before that, when you have too many images and need to look carefully, decide clearly, and turn the pile into something useful: a portfolio edit, a client selection, a post, a book, an exhibition, a reference board, or the next agent-assisted step.

Made by artists for artists, Cull is open, local, and agent-friendly: closer to Obsidian for images than another locked creative suite.

## What It Is For

- **Culling large runs** — go from 500 images to 20 keepers.
- **Visual reference work** — build a local Pinterest-like archive for serious art direction, design research, and image memory.
- **Social and editorial output** — organize batches into Instagram posts, LinkedIn posts, books, magazines, essays, exhibitions, field guides, and client selects.
- **AI-generated image work** — keep prompts, variants, sources, and iterations attached to the images they produced.
- **Agent-assisted workflows** — sort a folder yourself, or hand work to an agent through CLI or MCP when you want help.
- **Second-screen review** — keep the main app private while a client, collaborator, projector, iPad, or browser sees only the current preview.

## What It Does

- Drop in a folder, any size, any structure.
- Move quickly through Grid, Loupe, Compare, Canvas, Lineage, Embedding Explorer, Export, and Speed Review views.
- Rate, accept, reject, compare, collect, and filter without lifting your hands from the keyboard.
- Search by look and feel rather than filename.
- Surface the sharp ones, the warm ones, or the images that match a reference.
- Export picks for social, publishing, clients, contact sheets, PDFs, static reference sets, or the next agent-assisted step.
- Keep originals untouched and private by default.

## Under The Hood

- Local SQLite library with SHA-256 deduplication.
- Local CLIP/DINOv2 embeddings, UMAP visualization, image quality metrics, perceptual hashes, and histograms.
- Generation metadata from sidecar JSON, PNG text chunks, C2PA-style metadata, and filename evidence where available.
- Agent workflows through a local MCP server, MCP-aligned headless CLI commands, and `cull://` deep links.

## Principles

- **Local-first:** the library lives on your machine.
- **Non-destructive:** source files are not rewritten.
- **Keyboard-first:** common review actions are reachable without the mouse.
- **Agent-friendly:** work in the app, or let an agent operate the archive directly when that helps.
- **Open source:** Cull is Apache-2.0 software, not source-available software.
- **European-minded:** cloud/API features are explicit and optional; privacy, autonomy, and auditability matter.

## Download & Install

Quickest path, with [Homebrew](https://brew.sh):

```sh
brew install --cask glebis/tap/cull
```

Prefer a manual download (no developer tools required):

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
