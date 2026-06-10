# Publish Plugin-Only Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Publish a plugin-only feature: a `tabRegistry` becomes the single source of truth for top-level tabs, the host API gains `registerTab`, `cull-publish` ships as a bundled first-party Svelte plugin that auto-activates, and the core publish routing/gates are deleted.

**Architecture:** A new `tabRegistry` store replaces the hardcoded view lists in `keys.ts` and `command-palette.ts`. Core views self-register at startup; plugins register tabs via a new `registerTab` host method. Bundled first-party plugins are compiled into the app and activated unconditionally at startup (no blob:/checksum); third-party registry plugins keep their existing gated path. `StaticPublishingSettings.svelte` moves verbatim into the bundled `cull-publish` plugin, routing backend calls through `host.invoke`.

**Tech Stack:** Svelte 5 runes (`mount()` from `svelte` for plugin views), TypeScript, vitest, existing Rust `plugin_invoke` bridge (untouched).

**Spec:** `docs/superpowers/specs/2026-06-10-publish-plugin-only-design.md`

**Conventions for every task:** bd CLI is `/usr/local/bin/bd` (Homebrew bd is broken). Svelte 5 runes (`$state`/`$derived`/`onclick`), design tokens only (never hardcode colors). Never touch `cull.db`. After frontend changes run `npm run check && npm test`; Rust is untouched by this plan (run `cd src-tauri && cargo test --lib` only if you touch Rust). Commit per task; hooks must pass (never `--no-verify` unless the concurrent-worktree hazard forces it, then run `npm run preflight:quick` manually).

---

### Task 1: Tab registry store

**Files:**
- Create: `src/lib/plugins/tab-registry.ts`
- Test: `src/lib/plugins/tab-registry.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
    tabRegistry, registerCoreTabs, registerPluginTab, clearPluginTabs, tabCycleOrder,
} from './tab-registry';

describe('tabRegistry', () => {
    beforeEach(() => { clearPluginTabs(); registerCoreTabs(); });

    it('registers the core tabs in canonical order', () => {
        expect(tabCycleOrder()).toEqual([
            'grid', 'loupe', 'compare', 'canvas', 'lineage', 'embeddings', 'export', 'tinder',
        ]);
    });

    it('appends a plugin tab after core tabs', () => {
        registerPluginTab({ id: 'publish', label: 'Publish View', subtitle: 'Build a static site package', mountView: () => {} });
        expect(tabCycleOrder()).toEqual([
            'grid', 'loupe', 'compare', 'canvas', 'lineage', 'embeddings', 'export', 'tinder', 'publish',
        ]);
        const publish = get(tabRegistry).find(t => t.id === 'publish');
        expect(publish?.source).toBe('plugin');
        expect(typeof publish?.mountView).toBe('function');
    });

    it('clearPluginTabs removes only plugin tabs', () => {
        registerPluginTab({ id: 'publish', label: 'Publish View', mountView: () => {} });
        clearPluginTabs();
        expect(tabCycleOrder()).toEqual([
            'grid', 'loupe', 'compare', 'canvas', 'lineage', 'embeddings', 'export', 'tinder',
        ]);
    });

    it('a plugin tab cannot shadow a core tab id', () => {
        registerPluginTab({ id: 'grid', label: 'Fake Grid', mountView: () => {} });
        expect(get(tabRegistry).filter(t => t.id === 'grid')).toHaveLength(1);
        expect(get(tabRegistry).find(t => t.id === 'grid')?.source).toBe('core');
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/plugins/tab-registry.test.ts`
Expected: FAIL — cannot resolve `./tab-registry`.

- [ ] **Step 3: Write the implementation**

```ts
// src/lib/plugins/tab-registry.ts
// Single source of truth for top-level tabs (view modes). Core views register
// at startup; plugins append tabs via registerPluginTab. The Ctrl+Tab cycle
// (keys.ts) and the command palette (command-palette.ts) both derive from this
// — there is no second hardcoded list to drift out of sync.
import { writable, get } from 'svelte/store';
import type { ViewMode } from '../stores';

export interface TabEntry {
    id: ViewMode;
    label: string;
    subtitle?: string;
    source: 'core' | 'plugin';
    order: number;
    mountView?: (el: HTMLElement) => void;
}

// Core tabs in canonical cycle order. Cmd+digit bindings stay in keys.ts.
const CORE_TABS: Array<Omit<TabEntry, 'source'>> = [
    { id: 'grid', label: 'Grid View', subtitle: 'Browse thumbnails', order: 10 },
    { id: 'loupe', label: 'Loupe View', subtitle: 'Inspect the focused image', order: 20 },
    { id: 'compare', label: 'Compare View', subtitle: 'Compare selected or adjacent images', order: 30 },
    { id: 'canvas', label: 'Canvas View', subtitle: 'Arrange selected images spatially', order: 40 },
    { id: 'lineage', label: 'Lineage View', subtitle: 'Review related generations', order: 50 },
    { id: 'embeddings', label: 'Embeddings View', subtitle: 'Explore visual clusters', order: 60 },
    { id: 'export', label: 'Export View', subtitle: 'Prepare images for publishing', order: 70 },
    { id: 'tinder', label: 'Speed Review', subtitle: 'Fast accept or reject triage', order: 80 },
];

const PLUGIN_TAB_BASE_ORDER = 1000;

export const tabRegistry = writable<TabEntry[]>([]);

function sorted(entries: TabEntry[]): TabEntry[] {
    return [...entries].sort((a, b) => a.order - b.order);
}

export function registerCoreTabs(): void {
    tabRegistry.update(entries => {
        const withoutCore = entries.filter(e => e.source !== 'core');
        const core: TabEntry[] = CORE_TABS.map(t => ({ ...t, source: 'core' }));
        return sorted([...core, ...withoutCore]);
    });
}

export function registerPluginTab(tab: {
    id: string; label: string; subtitle?: string; mountView: (el: HTMLElement) => void;
}): void {
    tabRegistry.update(entries => {
        if (entries.some(e => e.id === tab.id)) return entries; // never shadow an existing id
        const order = PLUGIN_TAB_BASE_ORDER + entries.filter(e => e.source === 'plugin').length;
        return sorted([...entries, { ...tab, id: tab.id as ViewMode, source: 'plugin', order }]);
    });
}

export function clearPluginTabs(): void {
    tabRegistry.update(entries => entries.filter(e => e.source !== 'plugin'));
}

export function tabCycleOrder(): ViewMode[] {
    return sorted(get(tabRegistry)).map(e => e.id);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/plugins/tab-registry.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib/plugins/tab-registry.ts src/lib/plugins/tab-registry.test.ts
git commit -m "feat(plugins): tab registry as single source of truth for view tabs"
```

---

### Task 2: Host API — registerTab + suggestedHotkey type

**Files:**
- Modify: `src/lib/plugins/host.ts`
- Test: `src/lib/plugins/host-registertab.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { tabRegistry, clearPluginTabs, registerCoreTabs } from './tab-registry';
import { createPluginHost } from './loader';

describe('host.registerTab', () => {
    beforeEach(() => { clearPluginTabs(); registerCoreTabs(); });

    it('a plugin host can register a top-level tab that lands in the registry', () => {
        const host = createPluginHost('cull-publish');
        host.registerTab({ id: 'publish', label: 'Publish View', subtitle: 'Build a static site package', mountView: () => {} });
        const entry = get(tabRegistry).find(t => t.id === 'publish');
        expect(entry?.source).toBe('plugin');
        expect(entry?.label).toBe('Publish View');
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/plugins/host-registertab.test.ts`
Expected: FAIL — `host.registerTab is not a function`.

- [ ] **Step 3: Add `registerTab` to the `PluginHost` interface and `suggestedHotkey` to `PluginPaletteCommand`**

In `src/lib/plugins/host.ts`, change the `PluginPaletteCommand` interface to add the optional field:

```ts
export interface PluginPaletteCommand {
    id: string;
    title: string;
    subtitle?: string;
    keywords?: string[];
    /** Optional default hotkey, applied ONLY if currently unbound (host-side).
     * Never overrides a user or built-in binding; never a raw keydown grab. */
    suggestedHotkey?: string;
    run: () => void | Promise<void>;
}
```

And add to the `PluginHost` interface (after `registerPaletteCommands`):

```ts
    /** Register a top-level tab/view-mode. The tab joins the Ctrl+Tab cycle
     * and gets a command-palette entry; its mountView fills the view body. */
    registerTab(tab: { id: string; label: string; subtitle?: string; mountView: (el: HTMLElement) => void }): void;
```

- [ ] **Step 4: Implement `registerTab` in `createPluginHost`**

In `src/lib/plugins/loader.ts`, add the import at the top:

```ts
import { registerPluginTab } from './tab-registry';
```

In `createPluginHost(pluginId)`, add the method to the returned host object (alongside `mountView`/`registerPaletteCommands`):

```ts
        registerTab(tab) {
            registerPluginTab(tab);
        },
```

- [ ] **Step 5: Run test to verify it passes**

Run: `npx vitest run src/lib/plugins/host-registertab.test.ts`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/lib/plugins/host.ts src/lib/plugins/loader.ts src/lib/plugins/host-registertab.test.ts
git commit -m "feat(plugins): add registerTab host API + suggestedHotkey field"
```

---

### Task 3: suggestedHotkey — apply only if unbound

**Files:**
- Modify: `src/lib/plugins/loader.ts` (in `registerPaletteCommands`)
- Test: `src/lib/plugins/suggested-hotkey.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { applySuggestedHotkey } from './loader';

describe('applySuggestedHotkey', () => {
    beforeEach(() => localStorage.clear());

    it('binds the suggested hotkey when it is free', () => {
        const set = applySuggestedHotkey('cull-publish.publish', 'Cmd+9', { 'Cmd+9': undefined });
        expect(set).toBe(true);
    });

    it('does not bind when the hotkey collides with a built-in', () => {
        const set = applySuggestedHotkey('cull-publish.publish', 'Cmd+1', { 'Cmd+1': 'Grid view' });
        expect(set).toBe(false);
    });

    it('does not bind when the hotkey is already user-assigned to another command', () => {
        // simulate an existing user hotkey
        const set = applySuggestedHotkey('cull-publish.publish', 'Cmd+9', { 'Cmd+9': 'Some command' });
        expect(set).toBe(false);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/plugins/suggested-hotkey.test.ts`
Expected: FAIL — `applySuggestedHotkey` not exported.

- [ ] **Step 3: Implement `applySuggestedHotkey` and call it from `registerPaletteCommands`**

In `src/lib/plugins/loader.ts`, add imports:

```ts
import { setCommandHotkey, readCommandHotkeys, BUILT_IN_SHORTCUT_LABELS } from '../command-palette';
```

(If `BUILT_IN_SHORTCUT_LABELS` is not exported from `command-palette.ts`, export it there: change `const BUILT_IN_SHORTCUT_LABELS` to `export const BUILT_IN_SHORTCUT_LABELS`.)

Add the function:

```ts
/** Bind a plugin command's suggested hotkey only if the chord is unbound by a
 * built-in or an existing user hotkey. Returns whether it bound. `bound` maps
 * chord -> label/command for collision detection (injectable for tests). */
export function applySuggestedHotkey(
    commandId: string,
    hotkey: string,
    bound: Record<string, string | undefined>,
): boolean {
    if (bound[hotkey]) return false;        // collision with built-in or user binding
    setCommandHotkey(commandId, hotkey);
    return true;
}
```

In `registerPaletteCommands(commands)` inside `createPluginHost`, after recording the commands, apply suggested hotkeys:

```ts
        registerPaletteCommands(commands: PluginPaletteCommand[]) {
            // (existing registration code that stores `commands`) ...
            const userHotkeys = readCommandHotkeys();
            const bound: Record<string, string | undefined> = { ...BUILT_IN_SHORTCUT_LABELS };
            for (const [id, chord] of Object.entries(userHotkeys)) bound[chord] = id;
            for (const cmd of commands) {
                if (cmd.suggestedHotkey) applySuggestedHotkey(`${pluginId}.${cmd.id}`, cmd.suggestedHotkey, bound);
            }
        },
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/plugins/suggested-hotkey.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib/plugins/loader.ts src/lib/command-palette.ts src/lib/plugins/suggested-hotkey.test.ts
git commit -m "feat(plugins): apply plugin suggestedHotkey only when unbound"
```

---

### Task 4: keys.ts view cycle derives from the tab registry

**Files:**
- Modify: `src/lib/keys.ts` (`viewModeCycle`, imports)
- Test: `src/lib/keys-tab-registry.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect, beforeEach } from 'vitest';
import { clearPluginTabs, registerCoreTabs, registerPluginTab } from './plugins/tab-registry';
import { viewModeCycleForTest } from './keys';

describe('keys view cycle derives from tabRegistry', () => {
    beforeEach(() => { clearPluginTabs(); registerCoreTabs(); });

    it('includes a plugin-registered tab in the cycle', () => {
        registerPluginTab({ id: 'publish', label: 'Publish View', mountView: () => {} });
        expect(viewModeCycleForTest()).toContain('publish');
    });

    it('excludes publish when no plugin registered it', () => {
        expect(viewModeCycleForTest()).not.toContain('publish');
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/keys-tab-registry.test.ts`
Expected: FAIL — `viewModeCycleForTest` not exported.

- [ ] **Step 3: Rewrite `viewModeCycle` to use the registry**

In `src/lib/keys.ts`: remove `staticPublishingEnabled` from the `./stores` import (it is no longer used here). Add:

```ts
import { tabCycleOrder } from './plugins/tab-registry';
```

Replace the whole `viewModeCycle` function:

```ts
function viewModeCycle(): ViewMode[] {
    return tabCycleOrder();
}

/** Test seam: exposes the derived cycle without dispatching key events. */
export function viewModeCycleForTest(): ViewMode[] {
    return viewModeCycle();
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/keys-tab-registry.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/keys.ts src/lib/keys-tab-registry.test.ts
git commit -m "refactor(keys): derive view cycle from tab registry (kills gate drift)"
```

---

### Task 5: command-palette VIEW_COMMANDS derives from the tab registry

**Files:**
- Modify: `src/lib/command-palette.ts` (`VIEW_COMMANDS` → derived; remove `requiresStaticPublishing`; remove `currentPublishSurface` import)
- Test: `src/lib/command-palette-tabs.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect, beforeEach } from 'vitest';
import { clearPluginTabs, registerCoreTabs, registerPluginTab } from './plugins/tab-registry';
import { viewCommandsForTest } from './command-palette';

describe('palette view commands derive from tabRegistry', () => {
    beforeEach(() => { clearPluginTabs(); registerCoreTabs(); });

    it('lists a plugin tab as a view command with its label', () => {
        registerPluginTab({ id: 'publish', label: 'Publish View', subtitle: 'Build a static site package', mountView: () => {} });
        const cmds = viewCommandsForTest();
        expect(cmds.find(c => c.mode === 'publish')?.title).toBe('Publish View');
    });

    it('omits publish when no plugin registered it', () => {
        expect(viewCommandsForTest().find(c => c.mode === 'publish')).toBeUndefined();
    });

    it('preserves core Cmd+digit shortcuts', () => {
        const cmds = viewCommandsForTest();
        expect(cmds.find(c => c.mode === 'grid')?.shortcut).toBe('Cmd+1');
        expect(cmds.find(c => c.mode === 'export')?.shortcut).toBe('Cmd+7');
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/command-palette-tabs.test.ts`
Expected: FAIL — `viewCommandsForTest` not exported.

- [ ] **Step 3: Replace the hardcoded `VIEW_COMMANDS` with a registry-derived builder**

In `src/lib/command-palette.ts`: remove the `currentPublishSurface` import (line ~51) and the static `VIEW_COMMANDS` array (lines ~121–131). Add a Cmd+digit map for core tabs and a builder:

```ts
import { get } from 'svelte/store';
import { tabRegistry } from './plugins/tab-registry';

const CORE_VIEW_SHORTCUTS: Partial<Record<string, string>> = {
    grid: 'Cmd+1', loupe: 'Cmd+2', compare: 'Cmd+3', canvas: 'Cmd+4',
    lineage: 'Cmd+5', embeddings: 'Cmd+6', export: 'Cmd+7', tinder: 'Cmd+8',
};

function buildViewCommands(): Array<{ mode: ViewMode; title: string; subtitle: string; shortcut?: string }> {
    return get(tabRegistry).map(t => ({
        mode: t.id,
        title: t.label,
        subtitle: t.subtitle ?? '',
        shortcut: CORE_VIEW_SHORTCUTS[t.id],
    }));
}

/** Test seam. */
export function viewCommandsForTest() { return buildViewCommands(); }
```

Find where `VIEW_COMMANDS` was consumed (the `.filter(({ requiresStaticPublishing }) ...)` block around line ~722–731). Replace that block to map over `buildViewCommands()` directly with no `requiresStaticPublishing` filter:

```ts
            .map(({ mode, title, subtitle, shortcut }) => ({
                // ...existing command-item shape...
                keywords: [mode, 'tab', 'mode', ...(mode === 'publish' ? ['static', 'site', 'publishing'] : [])],
                // preserve the rest of the existing object construction (id, run: navigateTo(mode), shortcut, etc.)
            }))
```

(Keep the existing item-construction fields; only the source array and the removed filter change.)

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/command-palette-tabs.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Run the full palette + keys suites for regressions**

Run: `npx vitest run src/lib/command-palette.test.ts src/lib/keys-tab-focus.test.ts`
Expected: PASS (fix any test that referenced `requiresStaticPublishing` or the old static array).

- [ ] **Step 6: Commit**

```bash
git add src/lib/command-palette.ts src/lib/command-palette-tabs.test.ts
git commit -m "refactor(palette): derive view commands from tab registry"
```

---

### Task 6: Bundled-plugin loader

**Files:**
- Create: `src/lib/plugins/bundled.ts`
- Modify: `src/lib/plugins/loader.ts` (add `activateBundledPlugins`)
- Test: `src/lib/plugins/bundled.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { tabRegistry, clearPluginTabs, registerCoreTabs } from './tab-registry';
import { activateBundledPlugins } from './loader';
import { activePluginIds } from '../stores';

const fakeBundled = [{
    manifest: { id: 'cull-publish', name: 'Publish', version: '1.0.0', description: '', entry: '', permissions: ['library:read'], minAppVersion: '0.2.1', checksum: '', repo: '' },
    activate: (host: any) => host.registerTab({ id: 'publish', label: 'Publish View', mountView: () => {} }),
}];

describe('activateBundledPlugins', () => {
    beforeEach(() => { clearPluginTabs(); registerCoreTabs(); activePluginIds.set(new Set()); });

    it('activates bundled plugins regardless of the module_plugins flag', async () => {
        await activateBundledPlugins(fakeBundled, { pluginsFlagEnabled: false });
        expect(get(tabRegistry).find(t => t.id === 'publish')?.source).toBe('plugin');
        expect(get(activePluginIds).has('cull-publish')).toBe(true);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/plugins/bundled.test.ts`
Expected: FAIL — `activateBundledPlugins` not exported.

- [ ] **Step 3: Implement `activateBundledPlugins` + the bundled registry**

In `src/lib/plugins/loader.ts` add:

```ts
import { activePluginIds } from '../stores';
import type { PluginManifest } from './host';

export interface BundledPlugin {
    manifest: PluginManifest;
    activate: (host: PluginHost) => void | Promise<void>;
}

/** Activate first-party bundled plugins. They ship in the signed app, so no
 * blob:, checksum, consent, or module_plugins gate — they are trusted code. */
export async function activateBundledPlugins(
    plugins: BundledPlugin[],
    _opts: { pluginsFlagEnabled: boolean } = { pluginsFlagEnabled: false },
): Promise<void> {
    for (const plugin of plugins) {
        const host = createPluginHost(plugin.manifest.id);
        await plugin.activate(host);
        activePluginIds.update(ids => new Set(ids).add(plugin.manifest.id));
    }
}
```

Create `src/lib/plugins/bundled.ts`:

```ts
// First-party plugins compiled into the app build. Activated at startup,
// before registry plugins, regardless of the module_plugins flag.
import cullPublish from './cull-publish';
import type { BundledPlugin } from './loader';

export const BUNDLED_PLUGINS: BundledPlugin[] = [cullPublish];
```

(`./cull-publish` is created in Task 7; this import will not resolve until then — that is expected and Task 7 closes it. Do not wire `bundled.ts` into `+page.svelte` until Task 8.)

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/plugins/bundled.test.ts`
Expected: PASS (the test uses `fakeBundled`, not `BUNDLED_PLUGINS`, so the unresolved `./cull-publish` import in `bundled.ts` does not affect it).

- [ ] **Step 5: Commit**

```bash
git add src/lib/plugins/loader.ts src/lib/plugins/bundled.ts src/lib/plugins/bundled.test.ts
git commit -m "feat(plugins): bundled first-party plugin activation path"
```

---

### Task 7: cull-publish bundled plugin (move the publish UI)

**Files:**
- Create: `src/lib/plugins/cull-publish/index.ts`
- Create: `src/lib/plugins/cull-publish/manifest.ts`
- Create: `src/lib/plugins/cull-publish/PublishView.svelte` (moved from `StaticPublishingSettings.svelte`)
- Test: `src/lib/plugins/cull-publish/cull-publish.test.ts`

- [ ] **Step 1: Move the component**

```bash
git mv src/lib/components/StaticPublishingSettings.svelte src/lib/plugins/cull-publish/PublishView.svelte
```

In `PublishView.svelte`, replace direct Tauri/api backend calls that the plugin bridge supports with `host.invoke(...)`. The component receives `host` as a prop:

```svelte
<script lang="ts">
    import type { PluginHost } from '../host';
    let { host }: { host: PluginHost } = $props();
    // Replace prior direct calls:
    //   getLibraryStats()            -> host.invoke('get_library_stats')
    //   listCollections()            -> host.invoke('list_collections')
    //   listCollectionImages(args)   -> host.invoke('list_collection_images', args)
    //   exportStaticPublishPackage(a)-> host.invoke('export_static_publish_package', a)
    // Keep all other UI/state exactly as it was.
</script>
```

Audit the moved file for every `import ... from '$lib/api'` publish/export/library call and swap to `host.invoke`; leave non-backend imports (stores, components, utils) as-is, fixing their relative paths (now `../../` instead of `../`).

- [ ] **Step 2: Write the manifest**

```ts
// src/lib/plugins/cull-publish/manifest.ts
import type { PluginManifest } from '../host';

export const manifest: PluginManifest = {
    id: 'cull-publish',
    name: 'Publish View (Static Site)',
    version: '1.0.0',
    description: 'Build a read-only static site package from a collection.',
    entry: 'bundled',
    permissions: ['library:read', 'export:read', 'module:static-publishing'],
    minAppVersion: '0.2.1',
    checksum: 'bundled',
    repo: 'https://github.com/glebis/cull-plugins/tree/main/cull-publish',
};
```

- [ ] **Step 3: Write the failing test**

```ts
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { tabRegistry, clearPluginTabs, registerCoreTabs } from '../tab-registry';
import cullPublish from './index';
import { createPluginHost } from '../loader';

describe('cull-publish bundled plugin', () => {
    beforeEach(() => { clearPluginTabs(); registerCoreTabs(); });

    it('exposes a manifest with exactly the publish permissions', () => {
        expect(cullPublish.manifest.id).toBe('cull-publish');
        expect(cullPublish.manifest.permissions.sort()).toEqual(['export:read', 'library:read', 'module:static-publishing']);
    });

    it('registers the publish tab on activate', async () => {
        await cullPublish.activate(createPluginHost('cull-publish'));
        const tab = get(tabRegistry).find(t => t.id === 'publish');
        expect(tab?.source).toBe('plugin');
        expect(tab?.label).toBe('Publish View');
        expect(typeof tab?.mountView).toBe('function');
    });
});
```

- [ ] **Step 4: Run test to verify it fails**

Run: `npx vitest run src/lib/plugins/cull-publish/cull-publish.test.ts`
Expected: FAIL — cannot resolve `./index`.

- [ ] **Step 5: Write `index.ts` (the activate that mounts the Svelte view)**

```ts
// src/lib/plugins/cull-publish/index.ts
import { mount } from 'svelte';
import type { BundledPlugin } from '../loader';
import type { PluginHost } from '../host';
import { manifest } from './manifest';
import PublishView from './PublishView.svelte';

const cullPublish: BundledPlugin = {
    manifest,
    activate(host: PluginHost) {
        host.registerTab({
            id: 'publish',
            label: 'Publish View',
            subtitle: 'Build a static site package',
            mountView: (el: HTMLElement) => { mount(PublishView, { target: el, props: { host } }); },
        });
    },
};

export default cullPublish;
```

- [ ] **Step 6: Run test + typecheck**

Run: `npx vitest run src/lib/plugins/cull-publish/cull-publish.test.ts && npm run check`
Expected: PASS, 0 type errors. Fix any relative-import or `host.invoke` typing fallout in `PublishView.svelte`.

- [ ] **Step 7: Commit**

```bash
git add src/lib/plugins/cull-publish/ && git add -A
git commit -m "feat(plugins): cull-publish bundled plugin (publish UI moved from core)"
```

---

### Task 8: Wire startup + generic plugin-tab rendering; delete core publish path

**Files:**
- Modify: `src/routes/+page.svelte`
- Delete: `src/lib/plugins/publish-surface.ts`

- [ ] **Step 1: Register core tabs + activate bundled plugins at startup**

In `src/routes/+page.svelte`:
- Add imports:
  ```ts
  import { registerCoreTabs, tabRegistry } from '$lib/plugins/tab-registry';
  import { activateBundledPlugins } from '$lib/plugins/loader';
  import { BUNDLED_PLUGINS } from '$lib/plugins/bundled';
  import PluginViewHost from '$lib/components/PluginViewHost.svelte';
  ```
- Remove imports: `StaticPublishingSettings`, `CULL_PUBLISH_PLUGIN_ID`, `resolvePublishSurface`, and `staticPublishingEnabled` from the stores import if unused elsewhere in the file.
- At the top of `onMount`, before any view renders:
  ```ts
  registerCoreTabs();
  await activateBundledPlugins(BUNDLED_PLUGINS, { pluginsFlagEnabled: false });
  ```
- Delete the `publishSurface = $derived(resolvePublishSurface({...}))` block (lines ~74–80).

- [ ] **Step 2: Replace the publish render branch with generic plugin-tab rendering**

Remove the `{:else if $viewMode === 'publish' && publishSurface !== 'hidden'}` block (lines ~507–517). After the last core-view `{:else if}` branch, add a generic fallthrough for plugin tabs:

```svelte
        {:else if $tabRegistry.find(t => t.id === $viewMode && t.source === 'plugin')}
            <div class="plugin-view">
                <PluginViewHost pluginId={$viewMode} />
            </div>
```

`PluginViewHost` currently looks up a pre-stored element via
`getRegisteredPluginViews().get(pluginId)` and does
`container.replaceChildren(view)`. The new tab model passes a `mountView`
**callback** on the registry entry instead, so rewrite the `$effect` to call
it with the host's own container:

```svelte
<script lang="ts">
    import { get } from 'svelte/store';
    import { tabRegistry } from '$lib/plugins/tab-registry';

    let { pluginId, note = '' }: { pluginId: string; note?: string } = $props();
    let container = $state<HTMLElement | null>(null);
    let mounted = $state(false);

    $effect(() => {
        if (!container) return;
        const entry = get(tabRegistry).find(t => t.id === pluginId && t.source === 'plugin');
        if (entry?.mountView) {
            container.replaceChildren();
            entry.mountView(container);   // plugin mounts its Svelte view into the container
            mounted = true;
        } else {
            container.replaceChildren();
            mounted = false;
        }
    });
</script>
```

Keep the existing template/styles. The `pluginId` prop is now the tab id
(`$viewMode`), and the `note` prop is unused for the publish tab (omit it).
Since `cull-publish` no longer calls `host.mountView`, the old
`getRegisteredPluginViews` path is dead for tabs — leave it for any
non-tab plugin views, or remove it if grep shows no remaining callers.

- [ ] **Step 3: Delete the obsolete publish-surface module**

```bash
git rm src/lib/plugins/publish-surface.ts
```

Grep for stragglers and fix them:
```bash
grep -rnE "resolvePublishSurface|currentPublishSurface|publish-surface|StaticPublishingSettings|requiresStaticPublishing" src/ ; echo "should be empty"
```

- [ ] **Step 4: Typecheck + full frontend suite**

Run: `npm run check && npm test`
Expected: 0 type errors; all suites pass. Update any test that imported the deleted `publish-surface` or `StaticPublishingSettings` from core.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(plugins): publish is plugin-only — generic tab render, core path deleted"
```

---

### Task 9: Update the open-source release contract + integration check

**Files:**
- Modify: `src/lib/open-source-release-contract.test.ts`

- [ ] **Step 1: Write the failing assertions**

Add a describe block asserting the migration is complete:

```ts
import { existsSync } from 'node:fs';

describe('publish is plugin-only', () => {
    it('core no longer ships StaticPublishingSettings or publish-surface', () => {
        expect(existsSync('src/lib/components/StaticPublishingSettings.svelte')).toBe(false);
        expect(existsSync('src/lib/plugins/publish-surface.ts')).toBe(false);
    });

    it('the bundled cull-publish plugin exists', () => {
        expect(existsSync('src/lib/plugins/cull-publish/index.ts')).toBe(true);
        expect(existsSync('src/lib/plugins/cull-publish/PublishView.svelte')).toBe(true);
    });
});
```

- [ ] **Step 2: Run to verify it passes (the migration already made these true)**

Run: `npx vitest run src/lib/open-source-release-contract.test.ts`
Expected: PASS. (If FAIL, a prior task left a straggler — fix it.)

- [ ] **Step 3: Commit**

```bash
git add src/lib/open-source-release-contract.test.ts
git commit -m "test: pin publish-plugin-only migration in release contract"
```

---

### Task 10: Full verification + E2E + CODEX GATE

- [ ] **Step 1: Full suites**

Run: `npm run check && npm test && cd src-tauri && cargo test --lib && cd ..`
Expected: all green (Rust unchanged, so its suite is a regression guard).

- [ ] **Step 2: Browser E2E smoke (publish tab renders via plugin)**

Ensure dev server + Chrome Beta CDP are up (Task-0 conventions from the release plan), then:
Run: `bash tests/e2e/run-e2e.sh`
Expected: PASS. Manually confirm in the running dev app: Publish appears in the Ctrl+Tab cycle and command palette with `module_plugins` OFF, and the publish view renders.

- [ ] **Step 3: CODEX GATE**

```bash
codex exec --skip-git-repo-check --sandbox read-only "Audit the publish-plugin-only change (git diff against the branch point; the spec is docs/superpowers/specs/2026-06-10-publish-plugin-only-design.md). Check: (1) tabRegistry is the sole source for both the Ctrl+Tab cycle and the palette — the old gate disagreement cannot recur; (2) a plugin tab cannot shadow a core tab id; (3) bundled cull-publish activates regardless of module_plugins, third-party plugins stay gated; (4) suggestedHotkey never overrides a built-in or user binding; (5) the moved PublishView routes backend calls through host.invoke (no direct privileged calls bypassing the bridge); (6) no dead references to StaticPublishingSettings/publish-surface remain; (7) tests are behavioral. Verdict: APPROVE / APPROVE WITH CHANGES / REWORK." 2>/dev/null
```

If codex hangs >7 min (watch CPU time; it has hung this session), kill the process tree and run the `feature-dev:code-reviewer` agent with the same checklist, marked "codex-substitute". Negotiate findings, apply accepted changes (TDD for code), then commit + push:

```bash
git pull --rebase --autostash --quiet && CULL_PREFLIGHT_SKIP_E2E=1 git push
```

- [ ] **Step 4: Verify in the rebuilt app (optional but recommended)**

`npm run tauri build` → reinstall (`ditto ... /Applications/Cull.app`) → launch → confirm Publish works as a bundled plugin with no settings toggling.

---

**Self-review notes.** Spec coverage: tab registry (Task 1), registerTab (Task 2), suggestedHotkey (Task 3), cycle/palette derivation + gate-bug fix (Tasks 4–5), bundled loader (Task 6), publish migration + full-UI move (Task 7), deletions + startup wiring (Task 8), contract pins (Task 9), verification + gate (Task 10). Backend untouched (spec out-of-scope honored). Type names consistent across tasks: `TabEntry`, `registerPluginTab`, `tabCycleOrder`, `BundledPlugin`, `activateBundledPlugins`, `applySuggestedHotkey`. Known assumption to verify during execution: `PluginViewHost.svelte`'s current mount mechanism (Task 8 Step 2 adapts it to call the registry entry's `mountView`); the executor must read it before editing.
