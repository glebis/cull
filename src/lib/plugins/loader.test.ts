import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { pluginsEnabled } from '../stores';
import {
    clearPluginRegistrations,
    createPluginHost,
    getPluginPaletteCommands,
    getRegisteredPluginViews,
    grantPromptModel,
    loadInstalledPlugins,
    shouldLoadPlugins,
    verifyBundleChecksum,
} from './loader';
import type { LoadedPlugin } from './host';

function loadedPlugin(overrides: Partial<LoadedPlugin['manifest']> = {}, source = 'export default {};'): LoadedPlugin {
    return {
        manifest: {
            id: 'cull-publish',
            name: 'Publish View (Static Site)',
            version: '1.0.0',
            description: 'Build a static site package.',
            entry: 'dist/plugin.js',
            permissions: ['library:read', 'export:read'],
            minAppVersion: '0.2.1',
            checksum: 'sha256:deadbeef',
            repo: 'https://example.com',
            ...overrides,
        },
        source,
    };
}

// SHA-256("abc")
const ABC_SHA256 = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';

describe('plugin loader decision logic', () => {
    beforeEach(() => {
        pluginsEnabled.set(false);
        clearPluginRegistrations();
    });

    it('only loads when the module_plugins flag is exactly "true"', () => {
        expect(shouldLoadPlugins('true')).toBe(true);
        expect(shouldLoadPlugins('false')).toBe(false);
        expect(shouldLoadPlugins(null)).toBe(false);
        expect(shouldLoadPlugins('')).toBe(false);
        expect(shouldLoadPlugins('1')).toBe(false);
    });

    it('flag off: never fetches installed plugins and loads nothing', async () => {
        const fetchInstalled = vi.fn();
        const importModule = vi.fn();

        const loaded = await loadInstalledPlugins({
            getFlag: async () => 'false',
            fetchInstalled,
            importModule,
        });

        expect(loaded).toEqual([]);
        expect(fetchInstalled).not.toHaveBeenCalled();
        expect(importModule).not.toHaveBeenCalled();
        expect(get(pluginsEnabled)).toBe(false);
    });

    it('flag on: verifies the bundle hash and activates the plugin with a host', async () => {
        const plugin = loadedPlugin({ checksum: `sha256:${ABC_SHA256}` }, 'abc');
        const activate = vi.fn();
        const importModule = vi.fn(async () => ({ default: { activate } }));

        const loaded = await loadInstalledPlugins({
            getFlag: async () => 'true',
            fetchInstalled: async () => [plugin],
            importModule,
        });

        expect(loaded).toHaveLength(1);
        expect(importModule).toHaveBeenCalledWith('abc');
        expect(activate).toHaveBeenCalledTimes(1);
        const host = activate.mock.calls[0][0];
        expect(typeof host.mountView).toBe('function');
        expect(typeof host.registerPaletteCommands).toBe('function');
        expect(typeof host.invoke).toBe('function');
    });

    it('frontend re-hash: a checksum mismatch never reaches import()', async () => {
        const tampered = loadedPlugin({ checksum: `sha256:${ABC_SHA256}` }, 'tampered source');
        const importModule = vi.fn();

        const loaded = await loadInstalledPlugins({
            getFlag: async () => 'true',
            fetchInstalled: async () => [tampered],
            importModule,
        });

        expect(loaded).toEqual([]);
        expect(importModule).not.toHaveBeenCalled();
    });

    it('verifyBundleChecksum computes sha256 over the source', async () => {
        expect(await verifyBundleChecksum('abc', `sha256:${ABC_SHA256}`)).toBe(true);
        expect(await verifyBundleChecksum('abc', `sha256:${ABC_SHA256.toUpperCase()}`)).toBe(true);
        expect(await verifyBundleChecksum('abd', `sha256:${ABC_SHA256}`)).toBe(false);
    });
});

describe('grant prompt model', () => {
    it('surfaces manifest permissions with human-readable descriptions', () => {
        const model = grantPromptModel(
            loadedPlugin({ permissions: ['library:read', 'export:read', 'module:static-publishing'] }).manifest
        );

        expect(model.pluginId).toBe('cull-publish');
        expect(model.name).toBe('Publish View (Static Site)');
        expect(model.permissions.map(p => p.capability)).toEqual([
            'library:read',
            'export:read',
            'module:static-publishing',
        ]);
        for (const permission of model.permissions) {
            expect(permission.description.length).toBeGreaterThan(0);
            expect(permission.description).not.toBe(permission.capability);
        }
    });
});

describe('plugin host registrations', () => {
    beforeEach(() => {
        pluginsEnabled.set(false);
        clearPluginRegistrations();
    });

    it('palette commands registered by a plugin are hidden while the flag is off', () => {
        const host = createPluginHost('cull-publish');
        host.registerPaletteCommands([
            { id: 'open-publish', title: 'Open Publish View', run: () => {} },
        ]);

        expect(getPluginPaletteCommands()).toEqual([]);

        pluginsEnabled.set(true);
        const commands = getPluginPaletteCommands();
        expect(commands).toHaveLength(1);
        expect(commands[0].id).toBe('plugin.cull-publish.open-publish');
        expect(commands[0].category).toBe('Plugins');
    });

    it('mountView registrations are tracked per plugin', () => {
        const host = createPluginHost('cull-publish');
        const el = { tagName: 'DIV' } as unknown as HTMLElement;
        host.mountView(el);

        pluginsEnabled.set(true);
        expect(getRegisteredPluginViews().get('cull-publish')).toBe(el);
    });

    it('clearPluginRegistrations empties both registries', () => {
        const host = createPluginHost('p1');
        host.registerPaletteCommands([{ id: 'c', title: 'C', run: () => {} }]);
        host.mountView({} as HTMLElement);
        clearPluginRegistrations();
        pluginsEnabled.set(true);
        expect(getPluginPaletteCommands()).toEqual([]);
        expect(getRegisteredPluginViews().size).toBe(0);
    });
});
