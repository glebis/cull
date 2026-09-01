# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.6.1] - 2026-09-01

### Changed

- **Recovery release for the burned v0.6.0 tag.** The v0.6.0 annotated tag was pushed minutes before two dependabot commits landed on origin/main, so the immutable release gate correctly refused to build it (STALE_RELEASE_SOURCE) and the tag cannot be moved. Same content as the prepared v0.6.0 — ~26 MB smaller release bundle, pruned Claude Agent SDK resources, compressed bundled art, sidebar work below — plus the release-gate fix for closed-bead lookups and today\u2019s dev-dependency bumps. No behavior changes.

## [0.6.0] - 2026-09-01

### Changed

- **~26 MB smaller release bundle.** Release binaries are now stripped and link-time optimized; the bundled Claude Agent SDK is pruned to its runtime entry (sdk.mjs + package.json); bundled art is properly sized and quantized (favicon 810 KB → 2.4 KB, clipboard empty-state 550 KB → 35 KB, DMG background 306 KB → 68 KB). Unused framework placeholder SVGs and two unused EB Garamond faces were removed, and OS junk files no longer ship in the frontend build. Cull.app 71 MB → 45 MB, DMG 26.8 MB → 20.4 MB. No behavior changes; runtime import resolution verified in the built app.

### Added

- **Sidebar: recency rail above the folder tree.** Persistent `Just imported:` chip and last-N scope list above LIBRARY; auto-reveal + highlight until visited. Replaces the 8-second toast. (`imageview-1i2k.1`)
- **Sidebar: one adaptive search.** The sidebar scope filter now narrows detected classes and the active session's canvases (previously silently excluded). Enter promotes the query to the grid CommandBar. (`imageview-1i2k.2`)
- **Sidebar: hide zero counts + hide-empty option.** `formatSidebarCount` omits null/zero counts; new persisted option hides folders/collections with empty subtrees. (`imageview-1i2k.3`)
- **Sidebar: clipboard monitor promotion.** Persistent status chip in the footer strip opens a popover with start/stop, captured count, and a Details link that scrolls to the full clipboard section. (`imageview-1i2k.6`)
- **Sidebar: empty states teach the next action.** Empty Collections and Smart sections now name the action (`+` creates one; apply a grid filter then Save Collection). Smart section no longer vanishes when empty. (`imageview-1i2k.9`)
- **Sidebar: alt-hover folder preview (prototype).** Hold Option while hovering a folder row to open a thumbnail grid of that folder's images; the popover closes on Alt release. Plain hovers stay calm. Folder rows no longer show the redundant `…` menu — right-click and the section-item context menu expose folder actions. (`imageview-1i2k.10`)

### Changed

- **Sidebar: one geometric icon language.** Row-icon glyph dialects (◼/⏰/◇/★) removed in favor of words + counts + right-edge kind labels. Pin is a single rotated CSS square; running indicator is a CSS dot. The ⏰ emoji in Recent Imports is gone. (`imageview-1i2k.4`)
- **Sidebar: color semantics pinned.** Blue = interactive (active/focus/link/primary); green = positive state (success + live running dot); orange = active mode (collect-indicator moved off green); purple = detected-class tag; red = error. One meaning per accent color. (`imageview-1i2k.5`)

### Fixed

- **Grid overview: tiny cells now show real image colors.** At extreme zoom-out the overview canvas painted each cell with a color hashed from the image ID (random-looking, unrelated to content). Cells now paint with the image's stored dominant color (`get_dominant_colors`), falling back to a neutral surface when metrics are missing. (`imageview-lr9q`)
- **Concurrent external-device reads.** Opening another folder now cancels and supersedes the previous read, and duplicate-content registration resolves the canonical image identity instead of surfacing a foreign-key error.
- **External-drive detection on macOS.** Removable or ejectable SD cards remain visible even when macOS also reports the volume as internal.
- **Release regression coverage for sidebar feature retention.** The release gate now blocks if Recent Imports regains a decorative clock glyph or if connected-device visibility regresses.
- **Sidebar: 24px hit-area floor for twisty and preset chips.** Negative-inset pseudo-element on the button, not the input. (`imageview-1i2k.8`)
- **Sidebar: caption-size secondary text contrast.** `--text-secondary` raised to `#c5cbef` (OKLCH lightness-only) to clear APCA Lc at 9–10px caps. (`imageview-1i2k.7`)
- **Sidebar: folder row polish.** Folder counts align to a right-side column; twisties/chevrons bumped to 11px (with the negative-inset hit-area preserved); folder labels left-aligned; per-row `…` menu removed in favor of right-click + section-item context. (`imageview-1i2k.10`)

### Added

- **Local install helper (`scripts/install-local-build.sh`).** Refuses to overwrite `/Applications/Cull.app` while any Cull process is holding the executable open (macOS silently preserves the executable pages of a running Mach-O binary on overwrite). Trashes the old copy before installing so a partial install can never leave a hybrid old+new bundle on disk. Verifies the post-install SHA matches the freshly built artifact. Fixes the silent stale-binary bug where a copied-over Cull ran with the previous session's UI.
- **Release gate: per-version manual-smoke record required.** `release:cull tag` now blocks with `SMOKE_RECORD_MISSING`, `SMOKE_RECORD_STALE`, `SMOKE_BEAD_MISSING`, or `SMOKE_BEAD_OPEN` unless `docs/releases/<version>-smoke.md` exists with today's date and a binary SHA that matches `target/release/bundle/macos/Cull.app/Contents/MacOS/cull`, and a beads issue with `external_ref=cull-release-<version>-smoke` is closed. The smoke record is the human-driven step that prevents an automated session from tagging and shipping without a real install + manual verification.

## [0.5.1] - 2026-08-20

### Fixed

- **Release script: anchor releaseCommit on the merge commit.** scripts/cull-release.mjs previously recorded the version-bump commit as releaseCommit, but the immutable release tag gate requires origin/main to be an ancestor of the tagged SHA. On merge-commit-style release PRs the bump commit is one commit behind the merge commit, so the gate fired with STALE_RELEASE_SOURCE for v0.4.0 and v0.5.0. The script now records the pre-prepare source (= origin/main tip = merge commit) so runTag and the gate anchor on the same SHA. PR #190.

## [0.5.0] - 2026-08-20

- Recovered a v0.4.0 publish race: STALE_RELEASE_SOURCE fired because v0.4.0 was tagged on the release commit before its PR landed on main.
- The repository ruleset `Protect immutable release tags` (id 18866636) forbids tag recreation, so v0.4.0 stays in place and v0.5.0 ships with the same content.
- Same binaries, same changelog snapshot, deeper version number to keep the immutable-tag invariant.

## [0.4.0] - 2026-08-19

- Ship cli_tool install toggle (PR #184)
- Fix CI flakes (RUSTSEC-2026-0258 / h2 bump, deeplink tests on read-only HOME)

## [0.3.3] - 2026-08-08

### Fixed

- Restored readable PDF first-page previews on macOS; previews no longer render as solid black.
- Hardened release recovery so an already-verified draft can be published without replacing its assets.

## [0.3.2] - 2026-08-07

### Fixed

- Restored real PDF first-page previews and repaired legacy placeholders.
- Prevented PDF preview preparation from blocking filtering or leaving the interface in Loading.
- Prevented packaged interaction cache contamination and preserved empty Loupe and Recent Imports behavior.

## [0.3.1] - 2026-07-12

### Fixed

- Signed macOS releases now include and verify the Tauri updater archive and
  detached signature alongside the notarized Apple Silicon DMG.

## [0.2.5] - 2026-07-03

### Added

- Canvas edits now persist across scopes, with broader view-model coverage for
  saved canvas state.
- Undo history gained a dedicated panel, expanded event coverage, and a clearer
  empty state.
- The Cull site now presents a fuller screenshot tour with real app imagery,
  social sharing metadata, and latest-release download resolution.

### Changed

- The site download block, Homebrew install row, and hero slideshow were
  tightened for desktop, tablet, and mobile layouts.
- Site interaction polish now covers download buttons, slideshow dots, claim
  cards, footer cards, and footer links.

### Fixed

- Loupe actual-size/full-quality rendering uses originals more reliably.
- Collection counts and pinned collection actions reflect the current library
  state more accurately.
- Grid viewport position is preserved when layout column counts change.

## [0.2.4] - 2026-07-02

### Added

- Claude Agent SDK chat in the agent dock: streamed agent events, chat-driven
  selection proposals, agent profiles, token/cost estimates, and visual context
  for selected images.
- Agent panel commands exposed in the command palette.

### Fixed

- Empty collections, folders, and filters now show scope-specific empty states
  instead of claiming the whole library is empty (with a Clear Filters action
  for filter views).
- MCP settings: the copied Claude Code config now matches the displayed
  snippet exactly, and token create/revoke/rotate failures surface visible
  errors instead of failing silently.
- Toasts always render above dialogs and context menus via a tokenized
  z-index scale.
- Sidebar footer fit and accessibility; DMG installer window scrollbars.
- CI: Rust checks install node_modules (required by bundled agent-SDK
  resources) and the supply-chain audit passes again (anyhow updated for
  RUSTSEC-2026-0190; unresolvable transitive advisories documented in
  deny.toml).

## [0.2.3] - 2026-06-18

### Fixed

- Limited the signed macOS release workflow to Apple Silicon while Intel
  packaging is blocked by the current ONNX Runtime dependency setup.

## [0.2.2] - 2026-06-18

Reconstructed from the largest post-0.2.1 commit and merge chunks.

### Added

- Public `cull.company` landing site under `site/`, including Vercel signup and confirmation endpoints, generated visual assets, and a confirmed opt-in launch list flow.
- Plugin runtime foundation: manifest validation, capability grants, checksum-verified install/uninstall, registry parsing, frontend loader, `plugin_invoke` bridge, and a dedicated Plugins settings tab.
- Bundled `cull-publish` proof plugin that extracts the static publishing UI into a first-party plugin while keeping the backend export tools in core.
- Tab registry as the single source of truth for app views, including plugin-provided tabs and suggested hotkeys.
- Agent surface documentation covering the headless CLI, MCP connection flow, scoped tokens, audit log, approval boundaries, and the agent-snapshot demo loop.
- Export workbench improvements for social/editorial output, PDF rendering, contact sheets, and command-driven export launch.
- Preview Display recovery and refinements for second-screen and tokenized web preview workflows.
- Gesture support across grid, loupe, compare, and canvas interactions.
- Art catalog metadata layer and catalog commands for richer generation/source metadata workflows.
- Scoped PDF/media import work, including preview fallback paths for formats that need generated previews.
- Release readiness artifacts: audit reports, compatibility/release policy work, clean-machine DMG gate, release checksums, build provenance, and supply-chain audit commands.

### Changed

- README now focuses on positioning, use cases, installation, documentation links, development setup, and license/copyright boundaries instead of duplicating the user guide, roadmap, full CLI spec, and shortcut tables.
- Static publishing is treated as a plugin-capable surface with core backend support rather than only a built-in view.
- View commands, keyboard view cycling, and command-palette destinations now derive from the shared tab registry.
- AI/model-heavy first-run UI copy was softened and collapsed so a new empty library does not lead with optional model jargon.
- Delivery CSV and voice dictation are default-off module features; RAW support is visible/enabled by default.
- Dependency/toolchain baseline moved forward, including Tauri 2.11.2, Svelte 5.56.3, SvelteKit 2.65.1, Vite 8 site tooling, reqwest 0.13.3, png 0.18.1, dirs 6.0.0, thiserror 2.0.18, and Vitest 4.1.9.

### Fixed

- Plugin security gates now block ID traversal, unsafe `file://` scope expansion, and denied permission grants.
- Bundled plugin IDs survive third-party registry reloads.
- Token expiry warning threshold was corrected and token expiry/audit information is surfaced in the configuration dashboard.
- Backend initialization failures now show a distinct error state instead of a healthy-looking empty library.
- Global Tab hijacking was removed so native keyboard focus order works again.
- Toasts, clear/close buttons, and pill controls gained stronger accessibility labels and live-region behavior.
- Prompt resubmit cost estimates now guard against stale async responses before paid generation.
- Modal event containment and overlay layering were hardened for nested dialogs.
- Export event failures and Tauri asset rasterization issues were fixed.
- Site CI installs site dependencies and aligns the Rust toolchain before checks.

### Security

- The renderer CSP no longer whitelists unused AI provider hosts.
- MCP `export_images` output directories are confined to approved home/temp policy roots.
- The default asset-protocol scope no longer includes `$HOME/.codex/generated_images`.
- Supply-chain license auditing is wired into release checks.

## [0.2.1] - 2026-06-04

### Changed

- Rotated the Tauri updater signing key before the first public signed release.

### Fixed

- Configured the macOS release workflow to require a Developer ID Application identity for direct-download notarized builds.

## [0.2.0] - 2026-06-03

### Added

- Config-driven release workflow and a tiered compatibility policy (`docs/COMPATIBILITY.md`, `docs/CONTRACTS.md`, the `release` skill).
- MCP tag scopes are now enforced for per-image tools.

### Changed

- MCP scope filtering and pagination are pushed into SQL, so scoped tokens page correctly across large (100k+) libraries.
- Folder counts are computed with SQL grouping instead of a full Rust scan.
- Imports stream SHA-256 hashing and reject oversized files, bounding memory.

### Fixed

- MCP collection scopes are enforced consistently across all per-image tools.
- Smart-collection filters are validated before SQL generation (no invalid `IN ()` / out-of-range date windows).
- Migrations verify schema invariants on open, detecting partially-migrated databases.

### Security

- Google/Gemini API keys are sent via the `x-goog-api-key` header, never in the request URL.
- Pasted clipboard images no longer widen the `asset:` protocol scope to the original file.
- The static-publish server only serves validated Cull packages.
- Filesystem paths from deep links and clipboard paste go through one shared, sensitive-directory-aware policy.

## [0.1.0] - 2026-05-07

### Added

- Grid view with virtualized scrolling for 10K+ images
- Image import pipeline with SHA-256 dedup and recursive folder scanning
- Thumbnail generation (400px, Lanczos3 resampling)
- SQLite library with images, files, projects, selections, metadata tables
- Dark terminal theme with JetBrains Mono typography
- Vim-style keyboard navigation (hjkl, Home/End, PageUp/PageDown)
- Star rating (s+1-5), color labels, accept/reject curation
- Adjustable thumbnail size with slider and +/- keys
- Import progress indication with Tauri events
- ARIA grid semantics and WCAG AA contrast compliance
- Broken image fallback with filename display
