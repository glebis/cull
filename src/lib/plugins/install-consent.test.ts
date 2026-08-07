// Settings -> Plugins install consent contract (Track C2, bd imageview-dkz.24).
//
// Repo pattern: source-contract + decision-logic tests (no component mount
// infra), following client-tools-toggle-contract.test.ts. The load-bearing
// claim: the install dialog surfaces EVERY manifest permission string before
// any download/install command can be invoked, and the Registry/Installed
// groups (where install happens) are gated behind the module_plugins toggle —
// the Plugins tab itself is always reachable, but third-party install is not.

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { grantPromptModel } from './loader';
import type { PluginManifest } from './host';

const root = process.cwd();

function readProjectFile(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

const manifest: PluginManifest = {
    id: 'cull-publish',
    name: 'Publish View (Static Site)',
    version: '1.0.0',
    description: 'Build a static site package from a canvas or selection.',
    entry: 'plugin.js',
    permissions: ['library:read', 'export:read', 'module:static-publishing'],
    minAppVersion: '0.2.1',
    checksum: 'sha256:' + '0'.repeat(64),
    repo: 'https://github.com/glebis/cull-plugins',
};

describe('plugin install consent (decision logic)', () => {
    it('grantPromptModel lists every manifest permission with a description', () => {
        const model = grantPromptModel(manifest);
        expect(model.pluginId).toBe('cull-publish');
        expect(model.permissions.map(p => p.capability)).toEqual(manifest.permissions);
        for (const permission of model.permissions) {
            expect(permission.description.length).toBeGreaterThan(0);
        }
    });
});

describe('Settings -> Plugins section (source contract)', () => {
    const settings = () => readProjectFile('src/lib/components/PluginsSettings.svelte');
    const mcpSettings = () => readProjectFile('src/lib/components/McpSettings.svelte');
    const api = () => readProjectFile('src/lib/api.ts');

    it('exposes registry fetch, install, uninstall, and installed-info commands in the API layer', () => {
        const content = api();
        expect(content).toContain("invoke('fetch_plugin_registry')");
        expect(content).toContain("invoke('install_plugin'");
        expect(content).toContain("invoke('uninstall_plugin'");
        expect(content).toContain("invoke('list_installed_plugin_info')");
    });

    it('renders the consent dialog from grantPromptModel, listing each permission description', () => {
        const content = settings();
        expect(content).toContain('grantPromptModel(');
        // The dialog iterates the consent model's permissions.
        expect(content).toMatch(/\{#each\s+consent\.permissions\s+as\s+permission\}/);
        expect(content).toContain('{permission.capability}');
        expect(content).toContain('{permission.description}');
    });

    it('only invokes the install command after explicit consent confirmation', () => {
        const content = settings();
        // Clicking Install opens the consent dialog and must NOT install.
        const installCallsIn = (s: string) => (s.match(/(?<!un)installPlugin\(/g) ?? []).length;
        const request = content.match(/function requestInstall[\s\S]*?\n    \}/)?.[0] ?? '';
        expect(request).toContain('consent = grantPromptModel(');
        expect(installCallsIn(request)).toBe(0);
        // The API install call lives only in the confirm handler.
        const confirm = content.match(/async function confirmInstall[\s\S]*?\n    \}/)?.[0] ?? '';
        expect(installCallsIn(confirm)).toBeGreaterThan(0);
        expect(installCallsIn(confirm)).toBe(installCallsIn(content));
    });

    it('lists registry plugins with name, description, version, and permissions', () => {
        const content = settings();
        expect(content).toContain('fetchPluginRegistry()');
        expect(content).toContain('{plugin.manifest.name}');
        expect(content).toContain('{plugin.manifest.version}');
        expect(content).toContain('{plugin.manifest.description}');
        expect(content).toMatch(/manifest\.permissions/);
    });

    it('shows granted permissions per installed plugin and wires uninstall', () => {
        const content = settings();
        expect(content).toContain('listInstalledPluginInfo()');
        expect(content).toContain('uninstallPlugin(');
        expect(content).toMatch(/installed\.granted|\{granted\}|granted\.join/);
    });

    it('owns the module_plugins toggle inside the Plugins tab (moved out of McpSettings General)', () => {
        const plugins = settings();
        // The toggle and its persistence now live in PluginsSettings.
        expect(plugins).toContain("getAppSetting('module_plugins')");
        expect(plugins).toContain("setAppSetting('module_plugins'");
        // The store the rest of the app gates on stays in sync from here.
        expect(plugins).toContain('pluginsEnabled.set(modulePlugins)');

        // McpSettings mounts the tab and no longer carries the toggle/state.
        const content = mcpSettings();
        expect(content).toContain("{ id: 'plugins', label: 'Plugins' }");
        expect(content).toContain('<PluginsSettings');
        expect(content).not.toContain('toggleModulePlugins');
        expect(content).not.toContain('{#if modulePlugins}');
    });

    it('gates the Registry/Installed groups behind module_plugins', () => {
        // Install only happens inside the gated block; Core renders before it.
        const plugins = settings();
        const gate = plugins.indexOf('{#if modulePlugins}');
        expect(gate).toBeGreaterThan(-1);
        // requestInstall (third-party install entry point) is inside the gate.
        expect(plugins.indexOf('requestInstall(plugin)')).toBeGreaterThan(gate);
    });
});
