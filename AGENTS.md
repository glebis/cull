# Agent Instructions

## Project Overview

Cull is a Tauri 2 + SvelteKit 5 + Rust desktop image viewer focused on AI-generated art. Uses SQLite (rusqlite), CLIP embeddings (ONNX), and Svelte 5 runes.

## Architecture

- **Rust backend**: `src-tauri/src/db_core/` — models, DB, smart collections, NL parser, source detection
- **Frontend**: `src/lib/` — Svelte 5 components, API layer, stores
- **Commands**: `src-tauri/src/commands/` — Tauri IPC commands
- **Tests**: `src-tauri/` (Rust unit tests), `tests/e2e/` (browser E2E)

## Codebase Patterns

### Rust
- `Database` struct with `Mutex<Connection>`, accessed via `self.conn.lock().unwrap()`
- `Database::open()` → `run_migrations()` — not `new()`
- `ImageWithFile` is nested: `{ image: Image, path, thumbnail_path, selection: Option<Selection> }`
- All queries LEFT JOIN selections with `s.project_id = '__global__'`
- `rows.collect::<Result<Vec<_>>>()` — one generic param (rusqlite)
- Tauri commands: `pub async fn`, `State<'_, AppState>`, `.map_err(|e| e.to_string())`

### Svelte 5
- Uses runes: `$state()`, `$props()`, `$derived()`
- Event handlers: `onclick`, `onkeydown` (not `on:click`)
- CSS classes: `.section`, `.section-header`, `.section-item`, `.count`, `.active`
- Stores: `writable<T>` from `svelte/store`, accessed with `$storeName`

### CSS Design System (app.css)
```
--bg: #08080c          --surface: #0c0c12      --border: #1a1a2e
--text: #e0e0e0        --text-secondary: #7a7fa0
--blue: #7aa2f7        --green: #9ece6a        --orange: #e0af68
--purple: #bb9af7      --red: #f7768e
--spacing: 8px         --radius: 4px
--font: JetBrains Mono (monospace)
```
Tokyo Night dark theme. All components MUST use these tokens, never hardcode colors.

### API Layer
- `src/lib/api.ts` imports `invoke` directly from `@tauri-apps/api/core`
- **NO MOCK LAYER.** Never add a mock/fallback invoke. The app is a Tauri desktop app — it always runs with the real Rust backend. A previous mock layer (`tauri-mock.ts`) caused persistent bugs where the UI showed fake test data instead of the real database. It was removed 2026-05-09.
- `src/lib/tauri-mock.ts` exists for E2E browser testing ONLY — it must never be imported from `api.ts` or any component

### Data Safety
- **NEVER delete, trash, or reset `cull.db`** — it contains real user data (ratings, selections, collections) accumulated over many sessions
- When the UI shows wrong data, the bug is in the code, not the database
- The database path: `~/Library/Application Support/com.glebkalinin.cull/cull.db`

### License And Open Source Release
- Cull is true open source under Apache-2.0, not source-available. Do not
  reintroduce BSL/BUSL/source-available positioning in active product docs, app
  metadata, or UI.
- Keep license metadata aligned across `LICENSE.md`, `NOTICE`, `package.json`,
  `package-lock.json`, `src-tauri/Cargo.toml`, README, and the About dialog.
- Run `npm run audit:licenses` before publishing, before changing
  dependency/model download policy, and after adding dependencies.
- AI-assisted code is allowed, but do not paste generated output that appears
  copied from public code unless the upstream license is compatible and notices
  are preserved. Keep `AUTHORSHIP.md`, `CONTRIBUTING.md`, and
  `docs/OPEN_SOURCE_AUDIT.md` current when provenance assumptions change.

### Model And Asset Licensing
- Apache-2.0 covers Cull source code, not third-party model weights, fonts,
  artwork, or example assets.
- CLIP/DINOv2 embedding downloads must stay tied to compatible model licenses
  recorded in `docs/OPEN_SOURCE_AUDIT.md`.
- Do not add built-in downloads for YOLO, NudeNet, or other third-party ONNX
  weights unless source, license, attribution, checksum, and commercial-use terms
  are documented first.
- User-supplied local ONNX files are allowed, but the app must not imply Cull
  grants rights to those weights.

## E2E Testing with agent-browser

Tests run against `localhost:1420` in Chrome Beta via CDP. E2E tests use `tauri-mock.ts` directly (not via api.ts) for browser-only testing. The browser smoke suite is classified as a manual pre-push gate for covered UI/browser changes; see `docs/e2e-testing-policy.md` for the required file areas and non-CI status.

### Prerequisites
```bash
# Chrome Beta with debug port
"/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta" \
  --remote-debugging-port=9222 --user-data-dir="$HOME/.chrome-beta-profile" &

# Vite dev server
npx vite dev --port 1420 &
```

### Running tests
```bash
bash tests/e2e/run-e2e.sh
```

### agent-browser + Svelte 5 patterns
- Use `tab new` not `open` (prevents session switching)
- **Input values**: Svelte's `bind:value` doesn't detect DOM-level `.value` changes. Use native setter + event dispatch:
  ```bash
  agent-browser eval "const el = document.querySelector('.command-input'); \
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; \
    set.call(el, 'query text'); \
    el.dispatchEvent(new Event('input', {bubbles:true}));"
  ```
- **Keyboard events**: Use `dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', bubbles:true}))`
- **Clicks on dynamic elements**: Refs go stale between snapshot and click. Use CSS selectors:
  ```bash
  agent-browser eval "document.querySelector('.save-btn').click();"
  ```
- **Ref-based clicks** only work for stable elements (sidebar items, toolbar buttons)
- Generate unique session IDs: `SESSION="e2e$(tr -dc 'a-z0-9' < /dev/urandom | head -c 4)"`

## Smart Collections

### FilterNode schema (nested-capable)
```typescript
type FilterNode =
  | { type: 'group'; op: 'and' | 'or'; children: FilterNode[] }
  | { type: 'not'; child: FilterNode }
  | { type: 'rule'; field: string; op: string; value: any };
```

### Source detection
- Runs at import time, stores evidence JSON per image
- Layers: metadata/C2PA → filename patterns → PNG text chunks
- CLIP similarity is visual only, not source attribution

### NL parser
- Deterministic regex-based (`nl_parser.rs`), not ML
- Covers: ratings, sources (midjourney/sd/dalle/comfyui), orientation, format, recency, decisions, color labels

### Generation Runs
- `generation_runs` table is canonical source of truth for AI generation metadata (prompt, model, provider, seed, settings)
- Created from sidecar JSON files (`.json` adjacent to images) during import, or manually via MCP `set_generation_metadata`
- Images link via `generation_run_id` FK
- Sidecar parser handles both OpenAI (`provider, quality, thinking, estimated_cost`) and Gemini (`model, duration_s, edit_source`) schemas
- Raw sidecar JSON preserved in `raw_metadata_json` for forward-compatibility
- Key files: `db_core/sidecar.rs` (parser), `db_core/import.rs` (integration), `db_core/db.rs` (queries)
- MCP tools: `get_generation_run`, `set_generation_metadata`
- Loupe displays prompt + provider/model/seed tags when available

## bd Issue Tracking

This project uses **bd** (beads) for issue tracking. Run `npm run bd -- onboard`
to get started; the wrapper in `scripts/bd.sh` resolves multiple `bd` binaries
on PATH deterministically. Run `npm run bd -- prime` for current workflow
context, and run `npm run hooks:install` to install bd hooks plus Cull's tiered
hook chain.

## Reference Paths

- Beads local reference notes → `.beads/README.md`
- Beads tracked issue export → `.beads/issues.jsonl`

## Quick Reference

```bash
npm run bd -- ready              # Find available work
npm run bd -- show <id>          # View issue details
npm run bd -- update <id> --status in_progress  # Claim work
npm run bd -- close <id>         # Complete work
npm run bd -- vc status          # Check beads DB version-control state
npm run bd -- vc commit -m "..." # Commit beads DB changes when bd sync is unavailable
npm run hooks:install # Install bd hooks plus Cull pre-commit/pre-push checks
npm run preflight:hook    # Fast commit-time checks
npm run preflight:quick   # Frontend check and tests
npm run preflight:full    # Full pre-push checks
npm run preflight:release # Full + license audit + production build
npm run land              # Verify, sync/rebase/push, and print final status
```

Note: some installed `bd` versions do not expose `bd sync`. If `npm run bd -- sync` is
unavailable, use `npm run bd -- vc status` and `npm run bd -- vc commit -m "..."` for beads DB
changes, and mention the fallback in the handoff. If bd commands fail with
schema errors, run `npm run bd -- doctor` and `npm run bd -- migrate --yes` before trying any manual
SQL repair.

### bd binary version mismatch (important)

Two `bd` binaries can be on PATH with **incompatible Dolt schemas**:
`/usr/local/bin/bd` (e.g. 1.0.4) and `/opt/homebrew/bin/bd` (e.g. 0.59.0). The
`scripts/bd.sh` wrapper prefers the Homebrew one, which can fail every write with
`Error 1054 ... Unknown column 'crystallizes' in 'issues'` against a DB created
by 1.0.4. If you hit that, force the matching binary explicitly:

```bash
export BD_BIN=/usr/local/bin/bd   # the binary that owns the .beads Dolt DB
$BD_BIN create ... / $BD_BIN close ...
```

`bd create` supports `--type epic|task`, `-p P0..P4`, `--parent <id>`,
`--acceptance "..."`, `-d/--description`, and `--silent` (prints only the new ID).

bd state lives in the `.beads` Dolt DB (gitignored) and is exported to the tracked
`.beads/issues.jsonl`. Switching git branches reverts that jsonl, and bd may
re-sync from it — so a `bd close` done on one branch can appear reverted after a
checkout. When closing issues across per-issue branches, reconcile bd state on
`main` at the end (re-run the closes there) so the committed jsonl reflects reality.

## Agent Workflow Gotchas

Hard-won pitfalls when doing TDD + per-issue-branch + merge work in this repo:

- **Run `cargo fmt` inside `src-tauri/`, not the repo root.** There is no root
  `Cargo.toml`, so `cargo fmt` at the root is a silent no-op. The pre-push `full`
  preflight runs `cargo fmt --all -- --check`; a formatting drift there *fails the
  push* even though the pre-commit `quick` tier does not run fmt. Always
  `cd src-tauri && cargo fmt` before committing Rust.
- **Run the FULL `cargo test --lib` before merging, not just the changed module.**
  A scoped run like `cargo test --lib db_core::db::tests` will miss regressions in
  sibling modules (e.g. a new schema-invariant check breaking
  `db_core::db::migration_safety_tests`). The pre-push full tier catches it, but
  only after a slow round-trip — cheaper to run the whole lib suite locally first.
- **Pre-push `full` runs fmt + clippy + the whole test suite** (`scripts/preflight.sh full`);
  clippy is `cargo clippy --all-targets` *without* `-D warnings`, so pre-existing
  warnings don't fail the push, but a new compile/test/fmt failure does. Use
  `CULL_PREFLIGHT_SKIP_E2E=1 git push` to skip only the manual browser E2E gate.
- **macOS path-canonicalization in tests:** `tempfile::tempdir()` returns
  `/var/folders/...` which `std::fs::canonicalize` resolves to `/private/var/...`.
  Any test that compares a canonicalized path against a tempdir prefix must
  canonicalize the tempdir too, or `starts_with` fails. Production `dirs::home_dir()`
  is already canonical, so this bites tests only.
- **Cross-module test interactions:** changing shared DB/open behavior can break
  fixtures elsewhere that encoded the *old* behavior (e.g. minimal `user_version`
  fixtures). Prefer realistic fixtures (full-migrate, then mutate) over hand-rolled
  partial DBs.

### External audit + codex review

A full external audit (ChatGPT 5.5 Pro, security/UX/a11y/logic/scalability/
best-practices) was run on 2026-06-03; the raw audit doc is archived locally in
the gitignored `docs/internal/`. Its recommendations are fully tracked as bd
epics `imageview-hqf` (P0), `imageview-dtj` (P1), `imageview-9fz` (P2), each
child with Jobs-To-Be-Done + acceptance criteria — the public issue export in
`.beads/issues.jsonl` is the canonical record.

Note on review tooling: the `codex` CLI hung indefinitely in the headless/sandbox
environment (no output even for trivial prompts — likely a ChatGPT-account
auth/network issue). When codex is unavailable, the `feature-dev:code-reviewer`
agent is an effective substitute review gate; mark such reviews as
"codex-substitute" in the issue/commit so provenance is clear.

## Cull Preflight

Use `npm run preflight -- <hook|quick|full|release>` for project readiness checks:

```bash
npm run preflight -- hook     # shell syntax; no app tauri-mock imports
npm run preflight -- quick    # hook + npm run check; npm test
npm run preflight -- full     # quick + cargo fmt/clippy/tests
npm run preflight -- release  # full + npm run audit:licenses; npm run build
```

Do not use `bd preflight --check` for Cull readiness. In the installed bd
version, embedded bd preflight cannot be configured for this repo and runs a
generic Go/Nix checklist (`go test`, `golangci-lint`, `gofmt`, `go.sum`,
`default.nix`) that does not apply to Cull.

## Feature Landing Flow

Use the feature landing flow when a feature branch is complete and the user asks
to merge, build, land it on `main`, or make it part of the latest main builds.
Do not use it for WIP branches, dirty worktrees, PR-only review, or signed
release packaging.

```bash
npm run land:feature -- <feature-branch>
```

The flow merges the feature branch into `main`, runs `npm run check`,
`npm test`, and `npm run build`, falls back from unavailable `npm run bd -- sync` to
`npm run bd -- vc status`, pushes `main`, and watches main CI. Signed app artifacts are a
separate tag/manual Release workflow step.

Hook behavior is documented in `docs/dev-workflow-hooks.md`. The hook installer
uses bd chaining and Cull-managed markers so existing user hook content outside
managed sections is preserved.

### Hook Behavior

- Pre-commit runs `scripts/preflight.sh quick` by default.
- Pre-push runs `scripts/preflight.sh full` by default.
- Use `CULL_PRE_COMMIT_TIER=<tier>` or `CULL_PRE_PUSH_TIER=<tier>` only for
  deliberate overrides.
- Use `CULL_HOOK_SKIP_CHECKS=1` sparingly, and mention it in handoff when used.
- Use `CULL_PREFLIGHT_DRY_RUN=1` to print planned preflight commands without
  executing them.
- Use `CULL_PREFLIGHT_SKIP_E2E=1` to skip browser E2E inside the release tier.

## Branch And Merge Safety

- Inspect `git log main..<branch>` and `git diff --stat main...<branch>` before
  merging any branch.
- If a branch contains unrelated work, do not direct-merge it. Use a temporary
  worktree from `origin/main` and cherry-pick only the scoped commits.
- Never resolve conflicts by discarding user changes. If unrelated dirty work is
  present, preserve it on a separate branch/worktree and keep the merge scoped.
- Delete remote branches after successful merge, but preserve local dirty work
  under a clearly named branch rather than forcing deletion.

## Landing A Session

Use `npm run land` after changes are committed. It requires a clean worktree,
runs the full preflight tier unless `--skip-checks` is used, reports bd
version-control state, falls back when `bd sync` is unavailable, pulls with
rebase, pushes, and prints final git/bd status.

Useful variants:

```bash
npm run land -- --release
npm run land -- --skip-checks
npm run land -- --dry-run
npm run land -- --no-push
```

The landing script does not delete, reset, or clean user files.

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   npm run land
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
