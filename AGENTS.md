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

Tests run against `localhost:1420` in Chrome Beta via CDP. E2E tests use `tauri-mock.ts` directly (not via api.ts) for browser-only testing.

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

This project uses **bd** (beads) for issue tracking. Run `npm run bd -- onboard` to get started; the wrapper in `scripts/bd.sh` resolves multiple `bd` binaries on PATH deterministically.

## Quick Reference

```bash
npm run bd -- ready              # Find available work
npm run bd -- show <id>          # View issue details
npm run bd -- update <id> --status in_progress  # Claim work
npm run bd -- close <id>         # Complete work
npm run bd -- vc status          # Check beads DB version-control state
npm run bd -- vc commit -m "..." # Commit beads DB changes when bd sync is unavailable
```

Note: some installed `bd` versions do not expose `bd sync`. If `npm run bd -- sync` is
unavailable, use `npm run bd -- vc status` and `npm run bd -- vc commit -m "..."` for beads DB
changes, and mention the fallback in the handoff. If bd commands fail with
schema errors, run `npm run bd -- doctor` and `npm run bd -- migrate --yes` before trying any manual
SQL repair.

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

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   npm run bd -- sync  # If unavailable: npm run bd -- vc status && npm run bd -- vc commit -m "..."
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
