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

Run `npm run ci` before pushing to run the same checks as GitHub Actions.

**Rust:** `npm run ci:rust` runs `cargo fmt --all -- --check`, `cargo clippy --all-targets`, and `cargo test --all-targets`.

**Svelte/TypeScript:** `npm run ci:frontend` runs `npm ci`, `npm run check`, and `npm test`.

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
4. Run `npm run ci`
5. Commit with a descriptive message
6. Open a PR against `main`
