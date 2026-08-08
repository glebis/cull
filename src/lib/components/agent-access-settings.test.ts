import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(fileURLToPath(new URL('./AgentAccessSettings.svelte', import.meta.url)), 'utf8');

describe('Agent Access settings', () => {
    it('leads with Cull skill installation and keeps MCP optional', () => {
        const skill = source.indexOf('Install the Cull Skill');
        const connection = source.indexOf('MCP Connection');
        const tokens = source.indexOf('Access Tokens');
        const config = source.indexOf('Claude Code MCP Config');
        expect(skill).toBeGreaterThan(-1);
        expect(connection).toBeGreaterThan(skill);
        expect(tokens).toBeGreaterThan(connection);
        expect(config).toBeGreaterThan(tokens);
    });

    it('offers copyable commands and prompts without executing installers', () => {
        expect(source).toContain('npx skills add glebis/claude-skills --skill cull');
        expect(source).toContain('claude plugin marketplace add glebis/claude-skills');
        expect(source).toContain('claude plugin install cull@glebis-skills');
        expect(source).toContain('Use $skill-installer to install the Cull skill');
        expect(source).toContain('navigator.clipboard.writeText');
        expect(source).not.toMatch(/invoke\([^)]*(install|skill)/i);
    });
});
