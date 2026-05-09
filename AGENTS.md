# Agent Instructions

## Project Overview

ImageView is a Tauri 2 + SvelteKit 5 + Rust desktop image viewer focused on AI-generated art. Uses SQLite (rusqlite), CLIP embeddings (ONNX), and Svelte 5 runes.

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
- `src/lib/api.ts` imports `invoke` from `src/lib/tauri-mock.ts`
- `tauri-mock.ts` checks `isTauri()` at call time (not module load):
  - Inside Tauri → uses real `@tauri-apps/api/core` invoke
  - In Chrome browser → returns mock data for testing
- This means the app works in both Tauri and standalone browser

## E2E Testing with agent-browser

Tests run against `localhost:1420` in Chrome Beta via CDP.

### Prerequisites
```bash
# Chrome Beta with debug port
"/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta" \
  --remote-debugging-port=9222 --user-data-dir="$HOME/.chrome-beta-profile" &

# Vite dev server (mock layer activates automatically outside Tauri)
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

## bd Issue Tracking

This project uses **bd** (beads) for issue tracking. Run `bd onboard` to get started.

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
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

