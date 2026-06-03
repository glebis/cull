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

Run the Cull preflight wrapper before pushing:

```bash
npm run preflight -- quick    # Svelte check + Vitest
npm run preflight -- full     # quick + Rust fmt, clippy, and tests
npm run preflight -- release  # full + license audit and production build
```

Use `full` for normal pull requests and `release` before publishing or changing
license/model download policy. Do not use `bd preflight --check` for Cull
readiness; this bd version's embedded preflight is a generic Go/Nix checklist
and is not configurable for Cull.

**Rust:** `npm run ci:rust` runs `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets`, and `cargo test --locked --all-targets`. Clippy warnings are reported but not denied until `imageview-2w6.11` cleans up the existing warning backlog.

**Svelte/TypeScript:** `npm run ci:frontend` runs `npm ci`, `npm run check`, `npm test`, and `npm run build`.

**Browser E2E:** `npm run test:e2e` is a manual pre-push gate for covered UI/browser changes, including UI navigation, command palette/search flows, drag/drop affordances, preview display, and Tauri mock behavior. It is not part of `npm run ci` or GitHub CI yet; see [Browser E2E Testing Policy](docs/e2e-testing-policy.md).

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
