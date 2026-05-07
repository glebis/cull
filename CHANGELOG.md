# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

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
