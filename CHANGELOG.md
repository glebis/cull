# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

No changes yet.

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
