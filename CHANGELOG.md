# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

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
