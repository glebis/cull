import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { tabRegistry, clearPluginTabs, registerCoreTabs } from './tab-registry';
import { activateBundledPlugins, loadInstalledPlugins } from './loader';
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

    it('keeps bundled ids in activePluginIds when third-party load is disabled', async () => {
        await activateBundledPlugins(fakeBundled, { pluginsFlagEnabled: false });
        await loadInstalledPlugins({ getFlag: async () => null }); // module_plugins off
        expect(get(activePluginIds).has('cull-publish')).toBe(true);
    });

    it('keeps bundled ids when third-party plugins activate', async () => {
        await activateBundledPlugins(fakeBundled, { pluginsFlagEnabled: true });
        await loadInstalledPlugins({ getFlag: async () => 'true', fetchInstalled: async () => [] });
        expect(get(activePluginIds).has('cull-publish')).toBe(true);
    });
});
