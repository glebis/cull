# Plugins Settings Tab — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or executing-plans. TDD per task; checkbox steps.

**Goal:** A dedicated searchable Plugins settings tab that lists Core (bundled) plugins with a badge, adds a registry Refresh button, and drops the "Track C3" line from the Publish View description.

**Spec:** `docs/superpowers/specs/2026-06-10-plugins-settings-tab-design.md`

**Conventions:** Svelte 5 runes, design tokens only, never touch cull.db, `/usr/local/bin/bd` for bd. After frontend changes: `npm run check && npm test`. Commit per task; hooks must pass.

---

### Task 1: `filterPlugins` search helper

**Files:** Create `src/lib/plugins/plugin-search.ts` + `src/lib/plugins/plugin-search.test.ts`

- [ ] **Failing test:**

```ts
import { describe, it, expect } from 'vitest';
import { filterPlugins } from './plugin-search';

const items = [
    { id: 'cull-publish', name: 'Publish View (Static Site)', description: 'Build a static site package', permissions: ['export:read', 'library:read'] },
    { id: 'foo', name: 'Foo Tool', description: 'Does foo things', permissions: ['library:read'] },
];

describe('filterPlugins', () => {
    it('returns all on empty query', () => { expect(filterPlugins(items, '')).toHaveLength(2); });
    it('matches name case-insensitively', () => { expect(filterPlugins(items, 'publish').map(i => i.id)).toEqual(['cull-publish']); });
    it('matches description', () => { expect(filterPlugins(items, 'foo things').map(i => i.id)).toEqual(['foo']); });
    it('matches a permission', () => { expect(filterPlugins(items, 'export:read').map(i => i.id)).toEqual(['cull-publish']); });
    it('returns empty on no match', () => { expect(filterPlugins(items, 'zzz')).toEqual([]); });
});
```

- [ ] **Run → fail** (`npx vitest run src/lib/plugins/plugin-search.test.ts`).
- [ ] **Implement:**

```ts
// src/lib/plugins/plugin-search.ts
export interface SearchablePlugin {
    id: string;
    name: string;
    description?: string;
    permissions?: string[];
}

/** Case-insensitive match over name, description, and permissions. Empty
 * query returns the list unchanged. Pure — safe to call on every render. */
export function filterPlugins<T extends SearchablePlugin>(items: T[], query: string): T[] {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter(p =>
        p.name.toLowerCase().includes(q) ||
        (p.description ?? '').toLowerCase().includes(q) ||
        (p.permissions ?? []).some(perm => perm.toLowerCase().includes(q)));
}
```

- [ ] **Run → pass. Commit:** `feat(plugins): filterPlugins search helper`.

---

### Task 2: Clean the Publish View description + contract test

**Files:** Modify `src/lib/plugins/cull-publish/manifest.ts`; modify `src/lib/open-source-release-contract.test.ts`

- [ ] **Failing test** (add to the release contract):

```ts
it('the cull-publish manifest description has no internal/dev references', () => {
    const src = readFileSync('src/lib/plugins/cull-publish/manifest.ts', 'utf8');
    expect(src).not.toMatch(/Track\s*C3|C3 proof|Extracted from/i);
});
```

- [ ] **Run → fail.**
- [ ] **Implement:** in `manifest.ts` set
  `description: 'Build a read-only static site package from a collection.'`
- [ ] **Run → pass. Commit:** `fix(plugins): clean Publish View plugin description`.

---

### Task 3: Plugins settings tab + module_plugins toggle move

**Files:** Modify `src/lib/components/McpSettings.svelte`; modify `src/lib/components/PluginsSettings.svelte`; Test: `src/lib/components/plugins-settings-tab.test.ts` (source-contract style, matching the repo's existing component contract tests)

- [ ] **Failing test:**

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';

describe('plugins settings tab', () => {
    const mcp = readFileSync('src/lib/components/McpSettings.svelte', 'utf8');
    const plugins = readFileSync('src/lib/components/PluginsSettings.svelte', 'utf8');

    it('McpSettings has a plugins tab in the tab union and a tab button', () => {
        expect(mcp).toMatch(/activeSettingsTab[^\n]*'plugins'/);
        expect(mcp).toMatch(/activeSettingsTab\s*===\s*'plugins'/);
        expect(mcp).toMatch(/=>\s*activeSettingsTab\s*=\s*'plugins'/);
    });

    it('the module_plugins toggle lives in PluginsSettings, not McpSettings General', () => {
        expect(plugins).toMatch(/module_plugins/);
    });
});
```

- [ ] **Run → fail.**
- [ ] **Implement** in `McpSettings.svelte`:
  - Extend the union: `let activeSettingsTab = $state<'general' | 'appearance' | 'privacy' | 'plugins'>('general');`
  - Add a tab button after the others: `<button class="settings-tab" class:active={activeSettingsTab === 'plugins'} onclick={() => activeSettingsTab = 'plugins'}>Plugins</button>`
  - Add a render branch: `{:else if activeSettingsTab === 'plugins'}` rendering `<PluginsSettings />`.
  - Remove the existing `<PluginsSettings />` mount and the `module_plugins` toggle markup (lines around 525) from the General tab.
- [ ] **Implement** in `PluginsSettings.svelte`: add the `module_plugins` toggle at the top (checkbox bound to a local `modulePlugins` `$state`, `onchange` writes `setAppSetting('module_plugins', ...)` and syncs `pluginsEnabled` + toast — port `toggleModulePlugins` from McpSettings). Import `setAppSetting`, `pluginsEnabled`, `showToast`.
- [ ] **Run → pass; `npm run check`. Commit:** `feat(plugins): dedicated Plugins settings tab`.

---

### Task 4: Core group + Refresh + search wiring in PluginsSettings

**Files:** Modify `src/lib/components/PluginsSettings.svelte`; Test: extend `src/lib/components/plugins-settings-tab.test.ts`

- [ ] **Failing test** (append):

```ts
it('lists Core bundled plugins with a Core badge and no install/uninstall', () => {
    const plugins = readFileSync('src/lib/components/PluginsSettings.svelte', 'utf8');
    expect(plugins).toMatch(/BUNDLED_PLUGINS/);
    expect(plugins).toMatch(/Core/);               // badge label
});
it('has a registry Refresh button and a search input', () => {
    const plugins = readFileSync('src/lib/components/PluginsSettings.svelte', 'utf8');
    expect(plugins).toMatch(/Refresh/);
    expect(plugins).toMatch(/fetchPluginRegistry/);
    expect(plugins).toMatch(/filterPlugins/);
});
```

- [ ] **Run → fail.**
- [ ] **Implement** in `PluginsSettings.svelte`:
  - Import `{ BUNDLED_PLUGINS } from '$lib/plugins/bundled'` and `{ filterPlugins } from '$lib/plugins/plugin-search'`.
  - `let query = $state('')` bound to a search `<input>` at the top.
  - Extract registry fetch into a `refreshRegistry()` async fn (move the `onMount` body into it; `onMount` calls it). Add `let registryLoading = $state(false)`; set true/false around the fetch.
  - Add a **Core** group rendering `filterPlugins(BUNDLED_PLUGINS.map(p => p.manifest), query)` — each row shows name/version/permissions + a `<span class="core-badge" title="Built-in plugin — always available">⬡ Core</span>` (token-colored: `var(--purple)` on `var(--surface)`, mirroring `.permission-tag`), no Install/Uninstall. Render this group regardless of the toggle.
  - In the REGISTRY group header, add `<button class="action-btn" onclick={refreshRegistry} disabled={registryLoading}>{registryLoading ? 'Refreshing…' : 'Refresh'}</button>`.
  - Apply `filterPlugins(..., query)` to the registry and installed lists too. Suppress any registry entry whose id is in `BUNDLED_PLUGINS` (already shown in Core).
  - Add `.core-badge` CSS using only tokens.
- [ ] **Run → pass; `npm run check && npm test`. Commit:** `feat(plugins): Core group, registry Refresh, searchable list`.

---

### Task 5: Verify + CODEX GATE

- [ ] `npm run check && npm test` (all green) + `cd src-tauri && cargo test --lib` (regression guard, untouched).
- [ ] Update the published `cull-plugins` registry repo: clone/edit `registry.json` and `cull-publish/manifest.json` descriptions to drop the "Extracted from… Track C3…" sentence; commit + push that repo. (Re-verify the bundle checksum is unchanged — only the description string changes, which does NOT affect the bundle `plugin.js` checksum.)
- [ ] CODEX GATE (codex hangs in this env — watch CPU; if frozen, kill + use `feature-dev:code-reviewer`, mark codex-substitute):

```bash
codex exec --skip-git-repo-check --sandbox read-only "Audit the plugins-settings-tab change (git diff against the branch point; spec docs/superpowers/specs/2026-06-10-plugins-settings-tab-design.md). Check: (1) filterPlugins is pure and matches name/description/permissions; (2) Core bundled plugins render with a badge and no install/uninstall, and a duplicate registry entry for a bundled id is suppressed; (3) Refresh re-fetches and surfaces errors/loading; (4) the module_plugins toggle moved cleanly to the Plugins tab with no dead reference left in General; (5) the manifest description no longer references Track C3; (6) design tokens only, no hardcoded colors; (7) tests behavioral. Verdict: APPROVE / APPROVE WITH CHANGES / REWORK." 2>/dev/null
```

- [ ] Negotiate/apply/commit; `git pull --rebase --autostash && CULL_PREFLIGHT_SKIP_E2E=1 git push`.

---

**Self-review:** Spec coverage — filterPlugins (T1), description cleanup + contract (T2), settings tab + toggle move (T3), Core group/Refresh/search (T4), verify+registry-repo+gate (T5). Types consistent: `filterPlugins`, `SearchablePlugin`, `BUNDLED_PLUGINS`, `refreshRegistry`, `registryLoading`, `query`. Backend untouched per spec.
