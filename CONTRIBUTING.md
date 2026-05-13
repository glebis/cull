# Contributing to Cull

## Prerequisites

- Rust 1.78+
- Node.js 20+
- Tauri CLI: `npm install -g @tauri-apps/cli`

## Development Setup

```bash
git clone https://github.com/gastownhall/cull.git
cd cull
npm install
npm run tauri dev
```

## Code Style

**Rust:** `cargo fmt` and `cargo clippy` before committing.

**Svelte/TypeScript:** `npm run check` for type checking.

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
4. Run `cargo fmt && cargo clippy` and `npm run check`
5. Commit with a descriptive message
6. Open a PR against `main`
