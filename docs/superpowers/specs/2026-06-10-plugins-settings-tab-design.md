# Plugins Settings Tab — Design

**Date:** 2026-06-10
**Status:** Approved — ready to plan
**Owner:** Gleb Kalinin

## Goal

Improve the plugin settings UI: a dedicated **Plugins** tab in Settings, a
searchable plugin list, built-in (Core) plugins listed with a badge/icon, a
manual **Refresh** button for the registry, and a cleaned-up Publish View
plugin description (no internal "Track C3" reference).

## Background

Plugins currently render inside `PluginsSettings.svelte`, mounted within the
General settings tab and gated by `module_plugins`. The registry is fetched
only on mount (`fetchPluginRegistry()` in `onMount`), so after the earlier
404 (registry repo did not exist yet) there was no way to retry without
reopening settings. Bundled first-party plugins (`cull-publish`) are not
shown in the list at all, so a user cannot see that Publish is a built-in
plugin. The bundled manifest description still carries an internal note:
"Extracted from the core publish view (Track C3 proof plugin)".

Settings is already a tabbed overlay (`McpSettings.svelte`,
`activeSettingsTab: 'general' | 'appearance' | 'privacy'`).

## Locked decisions

| Decision | Choice |
|---|---|
| Plugins location | New dedicated `'plugins'` settings tab; `module_plugins` toggle moves here |
| Core plugins | Listed in a **Core** group from `BUNDLED_PLUGINS`, with a Core badge + icon, no Install/Uninstall, shown regardless of the toggle |
| Registry update | A **Refresh** button that re-calls `fetchPluginRegistry()` on demand, with loading/error state |
| Search | Client-side `filterPlugins(list, query)` over name/description/permissions, filtering all groups |
| Description cleanup | Bundled manifest + published registry description drop the "Extracted from the core publish view (Track C3 proof plugin)" sentence |

## Architecture

### Settings tab

`McpSettings.svelte`:
- Extend `activeSettingsTab` union with `'plugins'`.
- Add a `Plugins` button to `.settings-tabs`.
- Render `<PluginsSettings />` only in the `plugins` tab (remove its current
  mount point in General).
- Move the `module_plugins` toggle markup into `PluginsSettings.svelte` (its
  natural home) — `PluginsSettings` owns the toggle, reads/writes the
  `module_plugins` setting, and syncs `pluginsEnabled`. (Keep the existing
  `toggleModulePlugins` behavior/toast.)

### PluginsSettings layout

Three groups, top-to-bottom, all filtered by one search input:

1. **Core** — from `BUNDLED_PLUGINS` (manifests). Each row: name, version,
   permissions, a **Core badge** (icon + "Core" label, design-token colors).
   No Install/Uninstall. Always shown (independent of the toggle).
2. **Registry** — header row with the `REGISTRY` label and a **Refresh**
   button calling `fetchPluginRegistry()`; `registryLoading`/`registryError`
   `$state`. Rows show Install + permissions. Hidden when the toggle is off.
   A registry entry whose id matches a bundled Core plugin is suppressed
   here (it already appears in Core) to avoid a confusing duplicate.
3. **Installed** — third-party installed plugins with Uninstall. Hidden when
   the toggle is off.

Search: a `query` `$state` bound to a text input; `filterPlugins(items,
query)` (pure, exported, in `src/lib/plugins/plugin-search.ts`) matches
case-insensitively against `name`, `description`, and `permissions`. Applied
to each group's list before render. An empty query returns all.

### Core badge

A small inline component/markup: an icon (e.g. a shield or lock glyph
consistent with the app's monospace/token aesthetic — reuse an existing
icon pattern; no new asset) + the text "Core", using `var(--purple)` or
`var(--blue)` on `var(--surface)`, matching the existing `.permission-tag`
styling family. Tooltip/title: "Built-in plugin — always available".

### Description cleanup

- `src/lib/plugins/cull-publish/manifest.ts`: description becomes
  `"Build a read-only static site package from a collection."`
- Published registry repo (`glebis/cull-plugins`): `registry.json` and
  `cull-publish/manifest.json` descriptions updated to match. (Separate repo;
  updated via a commit/push to it, not the app repo.)
- The bundled tab subtitle ("Build a static site package") is unchanged.

## Testing

- **Unit — `filterPlugins`:** matches on name/description/permission
  case-insensitively; empty query returns all; no match returns empty.
- **Contract/behavioral — Core listing:** `BUNDLED_PLUGINS` renders in a Core
  group with the badge and without Install/Uninstall controls; a registry
  entry duplicating a bundled id is suppressed from the Registry group.
- **Behavioral — Refresh:** the button calls `fetchPluginRegistry` and sets
  `registryError` on failure (inject the fetcher per the existing
  source-contract test style).
- **Contract — settings tab:** `McpSettings` renders a `plugins` tab that
  mounts `PluginsSettings`; `module_plugins` toggle lives in that tab.
- **Contract — description hygiene:** the bundled manifest description
  contains no "Track C", "C3", or "Extracted from" substrings.
- Backend untouched — Rust suite is a regression guard only.

## Out of scope

- Editable registry URL field (refresh-only this iteration; the
  `plugin_registry_url` setting still exists and can be exposed later).
- Plugin update/version-bump flow (install/uninstall only).
- Any backend/Rust change.

## Review

- Codex-reviewed (or `feature-dev:code-reviewer` substitute if codex hangs)
  before the change lands, per the session's gate convention.
