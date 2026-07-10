import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { MCP_CONFIG_SNIPPET } from './mcp-config';

const componentsDir = join(process.cwd(), 'src/lib/components');

describe('MCP config snippet', () => {
    it('is valid JSON describing the cull stdio server', () => {
        const parsed = JSON.parse(MCP_CONFIG_SNIPPET);
        expect(parsed.mcpServers.cull.command).toBe('cull');
        expect(parsed.mcpServers.cull.args).toEqual(['--mcp-stdio']);
    });

    it('is the single source for both Agent Access display and copy', () => {
        const src = readFileSync(join(componentsDir, 'AgentAccessSettings.svelte'), 'utf8');
        expect(src).toContain('MCP_CONFIG_SNIPPET');
        // The old bug: copyConfig() copied a hardcoded app-bundle path that
        // differed from the rendered snippet.
        expect(src).not.toContain('/Applications/Cull.app');
        expect(src).not.toContain('mcpServers: {');
    });

    it('is the single source for AgentSkillsDialog', () => {
        const src = readFileSync(join(componentsDir, 'AgentSkillsDialog.svelte'), 'utf8');
        expect(src).toContain('MCP_CONFIG_SNIPPET');
        expect(src).not.toContain('"mcpServers": {');
    });
});
