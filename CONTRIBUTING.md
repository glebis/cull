# Contributing to Cull

## Prerequisites

- Rust 1.78+
- Node.js 20+
- Tauri CLI: `npm install -g @tauri-apps/cli`

## Development Setup

```bash
git clone https://github.com/glebis/cull.git
cd cull
npm install
npm run tauri dev
```

## Licensing Of Contributions

Cull is licensed under the Apache License 2.0. Unless you explicitly say
otherwise in writing, contributions intentionally submitted to this repository
are accepted under the same Apache-2.0 terms.

By contributing, you certify that you have the right to submit the work and that
it does not include code copied from unlicensed, source-available,
non-commercial, GPL, AGPL, LGPL, or otherwise incompatible sources.

AI-assisted contributions are allowed, but generated output must be reviewed as
source code, not pasted blindly. Do not submit generated code that matches public
code unless that public code has a compatible license and the required notices
are preserved. When a contribution is substantially AI-assisted, mention the
tool and the human review performed in the PR description.

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
