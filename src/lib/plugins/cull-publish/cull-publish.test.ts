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
