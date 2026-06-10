// Track C3 defer/fallback rule: who renders the publish view.
//
// When the plugin runtime is on AND the cull-publish plugin activated, the
// plugin renders the publish view and core StaticPublishingSettings defers —
// plugin presence substitutes for the raw module_static_publishing setting
// (the plugin's consented module:static-publishing grant gates the backend
// op in Rust). When the plugin is absent or module_plugins is off, core
// behaves exactly as today: the module-gated core view (the Day-4 fallback).

import { get } from 'svelte/store';
import { activePluginIds, pluginsEnabled, staticPublishingEnabled } from '../stores';

export const CULL_PUBLISH_PLUGIN_ID = 'cull-publish';

export type PublishSurface = 'plugin' | 'core' | 'hidden';

export interface PublishSurfaceState {
    pluginsEnabled: boolean;
    cullPublishActive: boolean;
    staticPublishingEnabled: boolean;
}

export function resolvePublishSurface(state: PublishSurfaceState): PublishSurface {
    if (state.pluginsEnabled && state.cullPublishActive) return 'plugin';
    return state.staticPublishingEnabled ? 'core' : 'hidden';
}

/** The current surface, read from the live stores (palette, menu). */
export function currentPublishSurface(): PublishSurface {
    return resolvePublishSurface({
        pluginsEnabled: get(pluginsEnabled),
        cullPublishActive: get(activePluginIds).has(CULL_PUBLISH_PLUGIN_ID),
        staticPublishingEnabled: get(staticPublishingEnabled),
    });
}
