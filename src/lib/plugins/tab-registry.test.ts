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
