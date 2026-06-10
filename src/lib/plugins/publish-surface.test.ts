// Track C3 defer/fallback decision logic: who renders the publish view.
//
// - plugin runtime ON + cull-publish active  -> the plugin renders the view
//   (plugin presence substitutes for the raw module_static_publishing
//   setting), and core StaticPublishingSettings defers.
// - plugin absent or module_plugins OFF      -> exactly today's behavior:
//   the core view, gated by module_static_publishing (the Day-4 fallback).

import { describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import {
    CULL_PUBLISH_PLUGIN_ID,
    currentPublishSurface,
    resolvePublishSurface,
} from './publish-surface';
import { activePluginIds, pluginsEnabled, staticPublishingEnabled } from '../stores';

describe('publish surface decision logic', () => {
    it('plugin runtime on + cull-publish active: the plugin renders the view', () => {
        // Regardless of the raw module_static_publishing setting — the
        // installed plugin (with its consented module:static-publishing
        // grant) IS the gate.
        for (const staticPublishing of [false, true]) {
            expect(
                resolvePublishSurface({
                    pluginsEnabled: true,
                    cullPublishActive: true,
                    staticPublishingEnabled: staticPublishing,
                })
            ).toBe('plugin');
        }
    });

    it('plugin absent: core view behaves exactly as today (module-gated)', () => {
        expect(
            resolvePublishSurface({
                pluginsEnabled: true,
                cullPublishActive: false,
                staticPublishingEnabled: true,
            })
        ).toBe('core');
        expect(
            resolvePublishSurface({
                pluginsEnabled: true,
                cullPublishActive: false,
                staticPublishingEnabled: false,
            })
        ).toBe('hidden');
    });

    it('plugin runtime off: Day-4 fallback, plugin presence is ignored', () => {
        expect(
            resolvePublishSurface({
                pluginsEnabled: false,
                cullPublishActive: true,
                staticPublishingEnabled: true,
            })
        ).toBe('core');
        expect(
            resolvePublishSurface({
                pluginsEnabled: false,
                cullPublishActive: true,
                staticPublishingEnabled: false,
            })
        ).toBe('hidden');
    });

    it('currentPublishSurface reads the live stores', () => {
        const restore = {
            plugins: get(pluginsEnabled),
            active: get(activePluginIds),
            staticPublishing: get(staticPublishingEnabled),
        };
        try {
            pluginsEnabled.set(true);
            activePluginIds.set(new Set([CULL_PUBLISH_PLUGIN_ID]));
            staticPublishingEnabled.set(false);
            expect(currentPublishSurface()).toBe('plugin');

            activePluginIds.set(new Set());
            expect(currentPublishSurface()).toBe('hidden');

            staticPublishingEnabled.set(true);
            expect(currentPublishSurface()).toBe('core');
        } finally {
            pluginsEnabled.set(restore.plugins);
            activePluginIds.set(restore.active);
            staticPublishingEnabled.set(restore.staticPublishing);
        }
    });
});
