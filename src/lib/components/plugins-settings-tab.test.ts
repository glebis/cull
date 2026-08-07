import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();
const read = (p: string) => readFileSync(join(root, p), 'utf8');

describe('plugins settings tab', () => {
    const mcp = read('src/lib/components/McpSettings.svelte');
    const plugins = read('src/lib/components/PluginsSettings.svelte');

    it('McpSettings has a plugins tab in the tab union and a tab button', () => {
        expect(mcp).toContain("{ id: 'plugins', label: 'Plugins' }");
        expect(mcp).toContain("$settingsTab === 'privacy'");
        expect(mcp).toContain('<PluginsSettings />');
    });

    it('the module_plugins toggle lives in PluginsSettings, not McpSettings General', () => {
        expect(plugins).toMatch(/module_plugins/);
    });

    it('renders PluginsSettings inside the shared settings section spacing', () => {
        expect(mcp).toContain('<section class="wrapped"><PluginsSettings /></section>');
    });

    it('lists Core bundled plugins with a Core badge', () => {
        expect(plugins).toMatch(/BUNDLED_PLUGINS/);
        expect(plugins).toMatch(/class="core-badge"/);
    });

    it('the Core group has no install/uninstall controls', () => {
        // Anchor on the template #each (not the <script> reference) so the
        // slice is the rendered Core block, not function definitions.
        const start = plugins.indexOf('{#each coreManifests');
        const gate = plugins.indexOf('{#if modulePlugins}');
        expect(start).toBeGreaterThan(-1);
        expect(gate).toBeGreaterThan(start);
        const coreBlock = plugins.slice(start, gate);
        expect(coreBlock).not.toContain('requestInstall');
        expect(coreBlock).not.toContain('handleUninstall');
        expect(coreBlock).not.toMatch(/>\s*Install\s*</);
        expect(coreBlock).not.toMatch(/>\s*Uninstall\s*</);
    });

    it('hides the Installed and Registry groups behind the module_plugins toggle', () => {
        const gate = plugins.indexOf('{#if modulePlugins}');
        const coreEach = plugins.indexOf('{#each coreManifests');
        expect(gate).toBeGreaterThan(coreEach); // Core is rendered outside the gate
        const gated = plugins.slice(gate);
        expect(gated).toContain('Registry');
        expect(gated).toContain('Installed');
    });

    it('has a registry Refresh button and a search input', () => {
        expect(plugins).toMatch(/Refresh/);
        expect(plugins).toMatch(/fetchPluginRegistry/);
        expect(plugins).toMatch(/filterPlugins/);
        expect(plugins).toMatch(/class="plugin-search"/);
    });
});
