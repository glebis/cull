import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { agentFailureCopy } from './agent-error-copy';

describe('agent failure copy', () => {
    it.each([
        ['API key is not configured for anthropic', 'AI access is not configured', 'Sign in to Claude Code or configure the required provider, then try again.'],
        ['maximum budget exceeded', 'Agent budget reached', 'Reduce the selection or shorten the instruction, then try again.'],
        ['request timed out after 120 seconds', 'Agent request timed out', 'Try again with fewer images or a shorter instruction.'],
        ['401 Unauthorized: invalid API key sk-secret', 'Claude authentication failed', 'Sign in to Claude Code again, then retry the request.'],
    ])('maps %s to actionable copy', (raw, title, detail) => {
        const result = agentFailureCopy(raw);

        expect(result).toEqual({ title, detail });
        expect(`${result.title} ${result.detail}`).not.toContain(raw);
    });

    it('uses a safe fallback instead of exposing an unknown raw error', () => {
        const raw = 'SDKError stack trace at internal/runner.ts:42';

        expect(agentFailureCopy(new Error(raw))).toEqual({
            title: 'Agent request failed',
            detail: 'Try again. If the problem continues, check Agent Access Settings.',
        });
    });

    it('routes the toast and activity log through the shared copy mapper', () => {
        const page = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');
        const dock = readFileSync(join(process.cwd(), 'src/lib/components/AgentProposalDock.svelte'), 'utf8');

        expect(page).toContain('agentFailureCopy(e)');
        expect(page).not.toContain("showToast('Claude agent failed', { detail: String(e)");
        expect(dock).toContain('agentActivityMessage(event)');
        expect(dock).not.toContain('<span>{event.message}</span>');
    });
});
