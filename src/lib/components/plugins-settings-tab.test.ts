import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';

describe('plugins settings tab', () => {
    const mcp = readFileSync('src/lib/components/McpSettings.svelte', 'utf8');
    const plugins = readFileSync('src/lib/components/PluginsSettings.svelte', 'utf8');

    it('McpSettings has a plugins tab in the tab union and a tab button', () => {
        expect(mcp).toMatch(/activeSettingsTab[^\n]*'plugins'/);
        expect(mcp).toMatch(/activeSettingsTab\s*===\s*'plugins'/);
        expect(mcp).toMatch(/=>\s*activeSettingsTab\s*=\s*'plugins'/);
    });

    it('the module_plugins toggle lives in PluginsSettings, not McpSettings General', () => {
        expect(plugins).toMatch(/module_plugins/);
    });
});
