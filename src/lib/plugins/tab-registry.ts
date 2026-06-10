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
