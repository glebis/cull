// Frontend plugin loader (runtime v1).
//
// Flow: Rust reads + hash-verifies installed bundles (load_installed_plugins)
// -> the webview re-hashes each source against the manifest checksum ->
// only matching code reaches a blob: dynamic import (CSP allows script-src
// blob: for exactly this path) -> the module's activate(host) runs with a
// narrow host API. Everything is behind the module_plugins flag, default OFF.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { getAppSetting } from '../api';
import { pluginsEnabled } from '../stores';
import type {
    GrantPromptModel,
    LoadedPlugin,
    PluginHost,
    PluginManifest,
    PluginModule,
    PluginPaletteCommand,
} from './host';

export function shouldLoadPlugins(flagValue: string | null): boolean {
    return flagValue === 'true';
}

export async function sha256Hex(source: string): Promise<string> {
    const bytes = new TextEncoder().encode(source);
    const digest = await crypto.subtle.digest('SHA-256', bytes);
    return [...new Uint8Array(digest)].map(b => b.toString(16).padStart(2, '0')).join('');
}

/** Webview-side re-hash of the bundle against the manifest checksum. */
export async function verifyBundleChecksum(source: string, checksum: string): Promise<boolean> {
    const expected = checksum.replace(/^sha256:/, '').toLowerCase();
    return (await sha256Hex(source)) === expected;
}

/** Import verified ESM source via a blob: URL (the only widened CSP source). */
export async function importPluginModule(source: string): Promise<PluginModule> {
    const blob = new Blob([source], { type: 'text/javascript' });
    const url = URL.createObjectURL(blob);
    try {
        return (await import(/* @vite-ignore */ url)) as PluginModule;
    } finally {
        URL.revokeObjectURL(url);
    }
}

// --- Host registries -------------------------------------------------------

interface RegisteredPaletteCommand extends PluginPaletteCommand {
    pluginId: string;
}

const paletteCommands: RegisteredPaletteCommand[] = [];
const pluginViews = new Map<string, HTMLElement>();

export function clearPluginRegistrations(): void {
    paletteCommands.length = 0;
    pluginViews.clear();
}

export function getRegisteredPluginViews(): Map<string, HTMLElement> {
    return pluginViews;
}

/** Palette items contributed by plugins, in the shape the command palette
 * consumes. Empty whenever the runtime flag is off — the gate lives here so
 * no caller can surface plugin commands ungated. */
export function getPluginPaletteCommands(): Array<{
    id: string;
    title: string;
    subtitle?: string;
    category: string;
    kind: 'command';
    keywords?: string[];
    run: () => void | Promise<void>;
}> {
    if (!get(pluginsEnabled)) return [];
    return paletteCommands.map(cmd => ({
        id: `plugin.${cmd.pluginId}.${cmd.id}`,
        title: cmd.title,
        subtitle: cmd.subtitle,
        category: 'Plugins',
        kind: 'command' as const,
        keywords: cmd.keywords,
        run: cmd.run,
    }));
}

export function createPluginHost(pluginId: string): PluginHost {
    return {
        mountView(el: HTMLElement) {
            pluginViews.set(pluginId, el);
        },
        registerPaletteCommands(commands: PluginPaletteCommand[]) {
            for (const command of commands) {
                paletteCommands.push({ ...command, pluginId });
            }
        },
        invoke(tool: string, args?: Record<string, unknown>) {
            return invoke('plugin_invoke', { pluginId, tool, args: args ?? null });
        },
    };
}

// --- Grant prompt model ----------------------------------------------------

const PERMISSION_DESCRIPTIONS: Record<string, string> = {
    'library:read': 'Read your image library (images, folders, collections, stats)',
    'library:search': 'Search your library by similarity and detected objects',
    'curation:write': 'Change ratings, decisions, and collections',
    'export:read': 'Export images and build publish packages',
    'import:write': 'Import new images into your library',
    'ai:run': 'Run AI analysis (embeddings, quality, object detection)',
    'display:navigate': 'Control what the app is currently showing',
    'tokens:manage': 'Manage MCP access tokens and the audit log',
    'settings:manage': 'Change app settings and background jobs',
};

export function describePermission(capability: string): string {
    const moduleKey = capability.startsWith('module:') ? capability.slice('module:'.length) : null;
    if (moduleKey) {
        return `Use the ${moduleKey.replace(/-/g, ' ')} module`;
    }
    return PERMISSION_DESCRIPTIONS[capability] ?? `Use the '${capability}' capability`;
}

/** Install-time consent data: surfaced BEFORE anything is granted or run. */
export function grantPromptModel(manifest: PluginManifest): GrantPromptModel {
    return {
        pluginId: manifest.id,
        name: manifest.name,
        permissions: manifest.permissions.map(capability => ({
            capability,
            description: describePermission(capability),
        })),
    };
}

// --- Loading ---------------------------------------------------------------

export interface PluginLoaderDeps {
    /** Reads the module_plugins app setting. */
    getFlag?: () => Promise<string | null>;
    /** Fetches hash-verified bundles from the Rust loader. */
    fetchInstalled?: () => Promise<LoadedPlugin[]>;
    /** Injectable for tests; defaults to the blob: dynamic import. */
    importModule?: (source: string) => Promise<PluginModule>;
}

/** Load and activate every installed plugin. No-op (and no backend traffic
 * beyond the flag read) unless module_plugins is "true". */
export async function loadInstalledPlugins(deps: PluginLoaderDeps = {}): Promise<LoadedPlugin[]> {
    const getFlag = deps.getFlag ?? (() => getAppSetting('module_plugins'));
    if (!shouldLoadPlugins(await getFlag())) {
        return [];
    }
    const importModule = deps.importModule ?? importPluginModule;
    const fetchInstalled =
        deps.fetchInstalled ?? (() => invoke<LoadedPlugin[]>('load_installed_plugins'));
    const installed = await fetchInstalled();
    const activated: LoadedPlugin[] = [];
    for (const plugin of installed) {
        const { manifest, source } = plugin;
        try {
            if (!(await verifyBundleChecksum(source, manifest.checksum))) {
                console.warn(`[plugins] checksum mismatch for '${manifest.id}', refusing to load`);
                continue;
            }
            const mod = await importModule(source);
            const activate = mod.default?.activate;
            if (typeof activate !== 'function') {
                console.warn(`[plugins] '${manifest.id}' has no default activate(host) export`);
                continue;
            }
            await activate(createPluginHost(manifest.id));
            activated.push(plugin);
        } catch (e) {
            console.error(`[plugins] failed to activate '${manifest.id}':`, e);
        }
    }
    return activated;
}
