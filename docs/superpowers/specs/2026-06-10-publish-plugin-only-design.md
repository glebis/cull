# Publish Plugin-Only — Design

**Date:** 2026-06-10
**Status:** Draft for review
**Owner:** Gleb Kalinin

## Goal

Make Publish a plugin-only feature. Remove the core publish UI and its
ad-hoc gating; extend the plugin host API so a plugin can register a
top-level tab/view-mode; ship `cull-publish` as a bundled first-party
plugin so Publish still works out of the box. Fold in the gate-consistency
bugfix (the Ctrl+Tab view cycle and the command palette must agree on which
tabs exist).

## Background

Track C extracted only the publish *settings UI* into a plugin while the
`'publish'` ViewMode, its tab, palette entry, view-cycle slot, and
`+page.svelte` routing stayed in core (`resolvePublishSurface` swapped
*content* inside a core-owned tab). The host API (`src/lib/plugins/host.ts`)
exposes only `mountView` and `registerPaletteCommands` — a plugin cannot
own a tab. Two gates also disagree: the Ctrl+Tab cycle (`keys.ts:35`) checks
`staticPublishingEnabled`, while the palette checks `currentPublishSurface()
!== 'hidden'`, so a plugin-active/setting-off state shows Publish in the
palette but not the cycle.

The backend (`export_static_publish_package`, the Rust static-publishing
service, MCP publish tools, the `module_static_publishing` setting) is core
and shared by both the UI and the agent/MCP path. It is out of scope to
move — only the UI and its gating change.

## Locked decisions

| Decision | Choice |
|---|---|
| Default-user access | `cull-publish` is a **bundled** first-party plugin, auto-activated at startup; Publish works with no setup |
| Core `StaticPublishingSettings.svelte` | Full UI moved into the plugin as its mounted Svelte view; core rendering path deleted |
| Bundled vs third-party loading | Bundled = compiled-in Svelte module, static import, no blob:/checksum; third-party = existing plain-ESM-over-blob: + checksum from the registry |
| Host API extension | `registerTab({id, label, mountView})` — minimal; no raw-key binding |
| Plugin shortcuts | Via the existing command-palette hotkey system (`cull.commandPalette.hotkeys`); optional `suggestedHotkey` applied only if unbound |
| `module_plugins` flag | Now gates **third-party registry plugins only**; bundled plugins ignore it |
| `module_static_publishing` setting | Retained as the **backend** capability gate (MCP/agent path); no longer drives UI |

## Architecture

### Tab registry (new single source of truth)

`src/lib/plugins/tab-registry.ts` — a `writable` of ordered tab entries:

```ts
interface TabEntry {
  id: ViewMode;        // e.g. 'grid', 'publish'
  label: string;       // palette title, e.g. 'Publish View'
  subtitle?: string;
  source: 'core' | 'plugin';
  order: number;       // stable sort key; core tabs keep current order
  mountView?: (el: HTMLElement) => void;   // plugin tabs only
}
```

- Core views register themselves at startup with their existing order and
  Cmd+1–8 bindings preserved (grid, loupe, compare, canvas, lineage,
  embeddings, export, plus tinder/speed-review).
- `keys.ts` derives the Ctrl+Tab cycle list from `tabRegistry` (sorted by
  `order`) instead of the hardcoded array; `VIEW_MODE_KEYS` (Cmd+digit) keeps
  its static map for core views only — plugin tabs have no Cmd+digit by
  default (they get a palette command and an optional `suggestedHotkey`).
- `command-palette.ts` derives `VIEW_COMMANDS` from `tabRegistry`, dropping
  the `requiresStaticPublishing` special case.
- `+page.svelte` renders core views by `id`; for a `source: 'plugin'` tab it
  renders `<PluginViewHost>` and calls the entry's `mountView`.

### Host API extension

`src/lib/plugins/host.ts` — add to `PluginHost`:

```ts
registerTab(tab: {
  id: string;
  label: string;
  subtitle?: string;
  mountView: (el: HTMLElement) => void;
}): void;
```

The host implementation pushes a `source: 'plugin'` entry into `tabRegistry`
with an `order` after all core tabs. `registerPaletteCommands` is unchanged;
its `PluginPaletteCommand` gains an optional `suggestedHotkey?: string`.

### Plugin shortcuts (no raw keys)

- A plugin tab and its palette commands are normal command-palette commands,
  already eligible for user hotkeys via `cull.commandPalette.hotkeys`.
- If a `PluginPaletteCommand` declares `suggestedHotkey`, the host writes it
  into the command-hotkey store **only if that hotkey is currently unbound**
  (checked against `BUILT_IN_SHORTCUT_LABELS` and the user hotkey store).
  Conflicts defer to the existing binding — a plugin can never override a
  user or built-in shortcut, and never registers a raw keydown handler.
- `KeyboardShortcuts.svelte` lists plugin commands like any other (no change
  needed beyond them appearing in the command list).

### Bundled-plugin loader

`src/lib/plugins/bundled.ts` — a static array of first-party plugin modules:

```ts
import cullPublish from './cull-publish';   // { manifest, activate }
export const BUNDLED_PLUGINS = [cullPublish];
```

Startup sequence (in the existing plugin-init path in `+page.svelte`):
1. Activate every `BUNDLED_PLUGINS` entry — call `activate(host)` directly
   with a host bound to the real bridge. No flag check, no checksum, no
   consent (trusted, in the signed build). Grants for a bundled plugin are
   its manifest permissions, recorded so `plugin_invoke` audit/enforcement
   works identically.
2. If `module_plugins` is on, run the existing registry-plugin load path for
   third-party plugins (unchanged).

Bundled plugins are deduped by id against installed third-party plugins;
a bundled id always wins (a registry plugin cannot shadow `cull-publish`).

### cull-publish plugin (the migration)

`src/lib/plugins/cull-publish/`:
- `index.ts` — exports `{ manifest, activate }`. `activate(host)` calls
  `registerTab({ id: 'publish', label: 'Publish View', subtitle: 'Build a
  static site package', mountView })` and mounts the Svelte view.
- `PublishView.svelte` — the full former `StaticPublishingSettings.svelte`
  UI, moved verbatim, with backend calls routed through `host.invoke(...)`
  where they previously called the Tauri commands directly (the host bridge
  already supports `export_static_publish_package`, `get_library_stats`,
  `list_collections`, `list_collection_images`).
- `manifest.ts` — same id/permissions as today (`library:read`,
  `export:read`, `module:static-publishing`).

The registry-distributed `entry.mjs` (the leaner plain-DOM reimplementation)
and the published `cull-plugins` repo remain as the *third-party* example /
proof path; the bundled in-app plugin is the compiled Svelte module. They
share the manifest id and permissions. (The bundled module is what users
actually run; the registry bundle stays the worked example of the
third-party contract.)

### Deletions

- `+page.svelte`: the `$viewMode === 'publish'` branch and its
  `resolvePublishSurface`/`StaticPublishingSettings` wiring (replaced by the
  generic plugin-tab render path).
- `keys.ts:35`: the `staticPublishingEnabled` view-cycle gate (cycle now
  derives from `tabRegistry`).
- `src/lib/plugins/publish-surface.ts`: obsolete — no core fallback exists,
  so `resolvePublishSurface`/`currentPublishSurface` are removed and their
  callers updated to read `tabRegistry`.
- `command-palette.ts`: the `requiresStaticPublishing` field and its filter.
- `src/lib/components/StaticPublishingSettings.svelte`: removed from core
  after its UI is moved into the plugin.

The `staticPublishingEnabled` store stays only if still read by the backend
gate path; if its sole consumers were the deleted UI gates, remove it too
and have the plugin rely on its `module:static-publishing` grant (which is
how `plugin_invoke` already gates the backend op).

## Testing

- **Unit — `tab-registry`:** core tabs register in the expected order with
  Cmd+1–8 preserved; a plugin `registerTab` appends after core tabs; the
  Ctrl+Tab cycle list and palette `VIEW_COMMANDS` both derive identically
  from the registry (the old gate-disagreement cannot recur because there is
  one source).
- **Unit — `suggestedHotkey`:** applied when the hotkey is unbound; ignored
  (existing binding wins) when it collides with a built-in or user hotkey.
- **Behavioral/contract:** bundled `cull-publish` auto-activates at startup
  and registers the `publish` tab; Publish is reachable with `module_plugins`
  OFF; the third-party registry path stays gated by `module_plugins`; a
  registry plugin cannot shadow the bundled `cull-publish` id.
- **Contract (open-source-release):** `StaticPublishingSettings.svelte` no
  longer imported by core; `publish-surface.ts` removed; no
  `requiresStaticPublishing` references remain.
- **Rust:** unchanged — backend untouched; full `cargo test --lib` must stay
  green (regression guard).
- **E2E smoke:** the existing browser suite, plus a check that the Publish
  tab renders via the plugin host.

## Risks

- **Moving 1122 LOC of Svelte:** the view is moved, not rewritten, so risk is
  mechanical (import paths, `host.invoke` substitution). Mitigated by moving
  verbatim and substituting only the backend call sites.
- **Startup ordering:** bundled plugins must register tabs before the view
  router and palette first read the registry. Mitigated by activating
  bundled plugins synchronously in the existing init path before first
  render of the view chrome.
- **Two cull-publish sources drifting** (bundled Svelte vs registry
  entry.mjs): accepted — the bundled module is canonical for users; the
  registry bundle is the third-party example. A test pins their shared
  manifest id/permissions so they cannot diverge silently on the contract.

## Out of scope

- Changing the backend publish capability, MCP publish tools, or
  `export_static_publish_package`.
- A generic VS Code-style `contributes:{}` manifest model (YAGNI — one tab
  API suffices).
- Letting plugins bind raw keydown handlers (explicitly rejected for
  security/conflict reasons).
- Removing the published `cull-plugins` registry repo or the third-party
  install flow.

## Review

- To be codex-reviewed (or `feature-dev:code-reviewer` substitute if codex
  is unavailable) before implementation, per the session's stage-gate
  convention.
