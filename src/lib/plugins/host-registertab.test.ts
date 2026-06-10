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
