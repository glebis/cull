// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Plugin runtime (Track C1).
//!
//! Plugins are precompiled frontend ESM bundles installed under
//! `$APPDATA/plugins/<id>/`, described by a `manifest.json`, and granted a
//! subset of the existing MCP capability vocabulary. The only privileged path
//! is the `plugin_invoke` Tauri command, enforced in Rust via the same
//! `require_capability` code path MCP tokens use. See `docs/plugins-design.md`.

pub mod install;
pub mod invoke;
pub mod loader;
pub mod manifest;
pub mod registry;
