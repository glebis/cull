// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/svelte';
import '@testing-library/jest-dom/vitest';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import type { ClaudeAgentStreamEvent } from '$lib/api';
import AgentProposalDock from './AgentProposalDock.svelte';

afterEach(cleanup);

function errorEvent(sequence: number, message: string): ClaudeAgentStreamEvent {
    return {
        request_id: 'request-1',
        sequence,
        phase: 'sdk_error',
        message,
        details: null,
        is_final: sequence === 4,
        is_error: true,
    };
}

describe('AgentProposalDock error copy', () => {
    it('shows actionable failure messages without raw SDK details', async () => {
        const user = userEvent.setup();
        const rawMessages = [
            'API key is not configured for anthropic',
            'maximum budget exceeded',
            'request timed out after 120 seconds',
            '401 Unauthorized: invalid API key sk-secret',
        ];

        render(AgentProposalDock, {
            proposals: [],
            presets: [],
            selectedCount: 0,
            pinned: false,
            visible: true,
            busy: false,
            streamEvents: rawMessages.map((message, index) => errorEvent(index + 1, message)),
            visualLevel: 'text',
            activePresetId: null,
        });
        await user.click(screen.getByText('Activity'));

        expect(screen.getByText('Sign in to Claude Code or configure the required provider, then try again.')).toBeVisible();
        expect(screen.getByText('Reduce the selection or shorten the instruction, then try again.')).toBeVisible();
        expect(screen.getByText('Try again with fewer images or a shorter instruction.')).toBeVisible();
        expect(screen.getAllByText('Sign in to Claude Code again, then retry the request.')).not.toHaveLength(0);
        for (const raw of rawMessages) expect(screen.queryByText(raw)).not.toBeInTheDocument();
    });

    it('does not expose a final raw error while the request is still busy', () => {
        const raw = '401 Unauthorized: invalid API key sk-secret';

        render(AgentProposalDock, {
            proposals: [],
            presets: [],
            selectedCount: 0,
            pinned: false,
            visible: true,
            busy: true,
            streamEvents: [errorEvent(1, raw)],
            visualLevel: 'text',
            activePresetId: null,
        });

        expect(screen.getAllByText('Sign in to Claude Code again, then retry the request.')[0]).toBeVisible();
        expect(screen.queryByText(raw)).not.toBeInTheDocument();
    });
});
