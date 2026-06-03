# Browser E2E Testing Policy

Cull has one browser E2E smoke suite, run by `npm run test:e2e` / `bash tests/e2e/run-e2e.sh`.
The runner starts Vite with `CULL_E2E_MOCK=1` and executes `tests/e2e/smoke.py` against the browser-only Tauri mock.

## Classification

**Current classification: pre-push, manual, change-triggered.**

Run the browser E2E smoke suite before pushing a branch or opening a PR when the
change touches one of the required file areas below. The suite is intentionally
not part of `npm run ci` or the current GitHub CI jobs because it depends on a
local browser environment and the E2E Tauri mock, but it is stronger than an
optional local-only check for covered UI behavior.

| Option | Status | Meaning for Cull |
| --- | --- | --- |
| Local-only | No | Useful for debugging, but not the policy for covered UI changes. |
| Pre-push | **Yes** | Required before push/PR for the file areas listed below. |
| Nightly | No | A good future automation target, but not currently configured. |
| CI-on-change | No | Do not claim GitHub CI runs it until a workflow explicitly does so. |

If the suite cannot run because the machine lacks the required browser or
Playwright setup, document the limitation in the PR test plan and include the
closest substitute checks you did run.

## File areas that require browser E2E

Run `npm run test:e2e` when a change affects any of these covered browser flows:

- **UI navigation and view switching:** `src/routes/+page.svelte`,
  `src/lib/keys.ts`, `src/lib/view-tabs.ts`, `src/lib/components/TabBar.svelte`,
  and changes that alter how Grid, Loupe, Compare, Canvas, Lineage, Embeddings,
  Export, or Tinder mount and become active.
- **Keyboard and thumbnail navigation:** `src/lib/components/Grid.svelte`,
  `src/lib/components/Thumbnail.svelte`, `src/lib/components/StatusBar.svelte`,
  selection/rating/decision stores, and shortcut handling in `src/lib/keys.ts`.
- **Command palette and command/search bars:** `src/lib/command-palette.ts`,
  `src/lib/components/CommandPalette.svelte`,
  `src/lib/components/CommandBar.svelte`,
  `src/lib/components/RuleBuilder.svelte`, and keyboard shortcuts that open,
  filter, navigate, or execute commands.
- **Drag/drop import affordances:** `src/routes/+page.svelte`, drop-overlay UX,
  Tauri event listeners that drive `drag-hover`, and mock coverage that simulates
  import/drop state. The browser suite verifies the overlay and front-end flow;
  native Finder/filesystem behavior still needs manual Tauri testing when changed.
- **Preview display and display chrome:** `src/lib/components/Loupe.svelte`,
  `src/lib/components/Compare.svelte`, `src/lib/components/Export.svelte`,
  `src/lib/components/EmbeddingExplorer.svelte`, thumbnail/image path helpers,
  zoom/presentation utilities, status bar/sidebar/zen-mode display, and any
  CSS/layout changes that could hide or resize previews.
- **Tauri mock behavior and E2E harness:** `src/lib/tauri-mock.ts`,
  `vite.config.js` E2E mock wiring, `tests/e2e/**`, `src/lib/e2e-runner.test.ts`,
  and `src/lib/api.ts` changes that add or rename commands consumed by covered UI
  flows. `src/lib/api.ts` must continue importing the real Tauri `invoke`
  directly; only the E2E Vite alias may substitute the mock.

## Running the suite

```bash
npm run test:e2e
```

Equivalent direct runner:

```bash
bash tests/e2e/run-e2e.sh
```

The smoke suite should remain browser/mock-only: it must not touch the real Cull
database, delete files, or invoke native filesystem actions.
