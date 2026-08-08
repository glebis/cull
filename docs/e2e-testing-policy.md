# Browser E2E Testing Policy

Cull has one browser E2E smoke suite, run by `npm run test:e2e` / `bash tests/e2e/run-e2e.sh`.
The runner starts Vite with `CULL_E2E_MOCK=1` and executes `tests/e2e/smoke.py` against the browser-only Tauri mock.

## Packaged production interaction gate

Every pull request and push to `main` runs a separate macOS gate against a
packaged `Cull.app` and its production WKWebView. It does not use Vite,
Playwright, or `tauri-mock.ts` at runtime. The gate seeds two images into a
dedicated smoke database and verifies pointer hit-testing plus observable UI
outcomes for a folder click, Recent Imports, the sidebar filter, image
selection, double-click Loupe navigation, and the image context menu.

```bash
bash tests/native/run-packaged-interaction-smoke.sh
```

The GitHub `CI` job is named `Packaged production interaction smoke`. A failed
assertion exits non-zero, fails the workflow before merge, and uploads the app
log, result manifest, and PNG failure capture from the smoke artifact directory. Its bundle
identifier is `com.glebkalinin.cull.interaction-smoke`, so it cannot open or
mutate the user's `com.glebkalinin.cull` database.

Vite's optimized dependencies use checkout-local, build-variant directories
under `.vite-cache`, not `node_modules/.vite`. This prevents a shared dependency
directory from hydrating one worktree with paths pre-bundled in another
worktree, and prevents a running dev server from sharing optimized modules with
the native-smoke build.

## Browser smoke classification

**Current browser-suite classification: pre-push manual gate plus machine-classified release CI.**

Run the browser E2E smoke suite before pushing a branch or opening a PR when the
change touches one of the required file areas below. The ordinary `CI` workflow
does not run the browser/mock suite; it runs the packaged production gate above.
The signed canary and production release workflows classify the
exact changed paths with `release.config.json`; when a covered path matches, the
release gate runs the browser suite on the GitHub macOS runner and records the
classification and result in immutable gate evidence.

| Option | Status | Meaning for Cull |
| --- | --- | --- |
| Local-only | No | Useful for debugging, but not the policy for covered UI changes. |
| Pre-push | **Yes** | Required before push/PR for the file areas listed below. |
| Nightly | No | A good future automation target, but not currently configured. |
| CI-on-change | Release only | Canary and production release gates run it for machine-classified covered paths; ordinary PR/main CI does not. |

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

## Blocking release regression contracts

`npm run test:release-regressions` is a required release gate, not an optional
test recommendation. It runs automatically from both `release:cull check` and
`release:cull prepare`, from the local `release` preflight tier, and from the
signed canary and production Release workflows. A missing test file, unknown
contract, non-zero test result, or stale release source blocks preparation and
packaging. The CI workflows refresh `origin/main` before evaluating ancestry;
the candidate must contain every commit on that verified ref and must itself be
reachable from it.

The gate runs these behavior-level contracts separately so the failing name is
actionable in terminal and GitHub Actions output:

| Contract | Automated behavior evidence |
| --- | --- |
| `sidebar-search-filter` | Sidebar tree search/filter rules and selecting a detected-object filter updates active scope and reloads images. |
| `settings-ai-reachable` | Settings opens an explicit tab, the AI tab is mounted/reachable, and AI settings render and react to backend readiness. |
| `grid-deep-zoom-presets` | Minimum-to-maximum grid zoom mapping, presets, gesture anchoring, and zoom bounds. |
| `grid-hover-preview` | Hover delay/eligibility, preview placement, and cancellation behavior. |
| `history-palette-deeplink` | History filtering, palette destination execution, and deep-link routing/integration. |
| `thumbnail-prefetch` | Bounded prefetch cache scheduling, deduplication, eviction, and memory accounting. |

These focused tests avoid screenshot-only assertions. The classified browser
suite remains complementary and verifies integrated UI flows such as search,
command palette execution, and Settings → AI when covered paths changed.

Residual manual validation before publishing remains: install the signed DMG on
a clean macOS account; use a real large image library to inspect hover-preview
positioning while scrolling and at viewport edges; exercise minimum/maximum
deep zoom with trackpad and wheel input; confirm selection remains stable while
virtualized rows recycle; and watch memory/network/decode activity during rapid
scroll reversals. Native Finder drag/drop, real filesystem permissions, signing,
notarization, updater, and Homebrew installation also remain manual because the
browser/mock behavior tests cannot prove them.
