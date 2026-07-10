import { describe, expect, it } from 'vitest';
import {
    parseAgentProposalSourceContext,
    proposalActorLabel,
    proposalViewContextKey,
    sourceContextIsStale,
    sourceContextScopeLabel,
} from './agent-proposal-context';

describe('agent proposal source context', () => {
    it('derives proposal actor, source scope, and staleness from source context', () => {
        const sourceContext = parseAgentProposalSourceContext(JSON.stringify({
            source: 'claude_agent_sdk',
            actor: { type: 'agent', name: 'Claude', role: 'copilot' },
            view_context: {
                kind: 'folder',
                id: null,
                label: 'Portfolio',
                path: '/art/portfolio',
                view_mode: 'grid',
            },
        }));

        expect(proposalActorLabel(sourceContext, 'copilot')).toBe('Claude (copilot)');
        expect(sourceContextScopeLabel(sourceContext)).toBe('Portfolio (grid)');
        expect(proposalViewContextKey(sourceContext.view_context)).toBe('folder:/art/portfolio@grid');
        expect(sourceContextIsStale(sourceContext, {
            kind: 'collection',
            id: 'col_best',
            label: 'Best',
            path: null,
            view_mode: 'grid',
        })).toBe(true);
    });
});
