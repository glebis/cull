# Contributing to Cull

## Prerequisites

- Rust 1.78+
- Node.js 20+
- Tauri CLI: `npm install -g @tauri-apps/cli`

## Development Setup

```bash
git clone https://github.com/glebis/imageview.git
cd imageview
npm install
npm run tauri dev
```

## Code Style

**Rust:** `cd src-tauri && cargo fmt --all`, `cargo clippy --all-targets`, and `cargo test --all-targets` before committing.

**Svelte/TypeScript:** `npm run check` for type checking and `npm test` for unit tests.

## Issue Tracking

This project uses [beads](https://github.com/gastownhall/beads) (`bd`) for issue tracking. Issues live in `.beads/` alongside the code.

```bash
bd list              # see open issues
bd show <id>         # issue details
bd q "title"         # quick-create an issue
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Run `npm run check`, `npm test`, `cd src-tauri && cargo fmt --all`, `cargo clippy --all-targets`, and `cargo test --all-targets`
5. Commit with a descriptive message
6. Open a PR against `main`
