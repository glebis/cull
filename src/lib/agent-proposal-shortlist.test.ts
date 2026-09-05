import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

import type { AgentActionProposal, SelectionState } from './api';

const apiMocks = vi.hoisted(() => ({
    applyActionProposal: vi.fn(),
    getSelectionRun: vi.fn(),
    addToShortlist: vi.fn(),
    removeFromShortlist: vi.fn(),
}));

vi.mock('./api', () => ({
    ...apiMocks,
    undo: vi.fn().mockResolvedValue(null),
    redo: vi.fn().mockResolvedValue(null),
    listImages: vi.fn().mockResolvedValue([]),
    getImageCount: vi.fn().mockResolvedValue(0),
    listImagesByFolder: vi.fn().mockResolvedValue([]),
    listImagesFiltered: vi.fn().mockResolvedValue([]),
    listCollectionImages: vi.fn().mockResolvedValue([]),
    listImagesByDetectedClass: vi.fn().mockResolvedValue([]),
    listImagesInReferencedFolder: vi.fn().mockResolvedValue([]),
    evaluateSmartCollection: vi.fn().mockResolvedValue([]),
    getBatchImages: vi.fn().mockResolvedValue([]),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockResolvedValue(() => {}),
}));

import {
    activeAgentProposalId,
    selectionRun,
    selectedIds,
    shortlistIds,
    toasts,
} from './stores';
import {
    isShortlistProposalKind,
    shortlistProposalActionLabel,
    sourceContextSelectionId,
} from './agent-proposal-context';
import { applyShortlistProposal } from './agent-proposal-shortlist';
import type { AgentProposalSourceContext } from './agent-proposal-context';

function makeProposal(kind: string, selectionId: string | null): AgentActionProposal {
    const sourceContext: AgentProposalSourceContext & { selection_id?: string } = {
        source: 'claude',
    };
    if (selectionId !== null) sourceContext.selection_id = selectionId;
    return {
        id: `prop-${kind}`,
        kind,
        status: 'pending',
        persona: 'copilot',
        lens: null,
        criteria: 'test criteria',
        visual_level: 'tiny',
        selection_preset_id: null,
        estimated_input_tokens: null,
        estimated_output_tokens: null,
        estimated_cost_eur: null,
        source_context_json: JSON.stringify(sourceContext),
        items_json: JSON.stringify([
            { image_id: 'off-screen-1', reason: 'strong candidate' },
            { image_id: 'off-screen-2', reason: 'strong candidate' },
        ]),
        guard_results_json: '{}',
        apply_result_json: null,
        undo_journal_json: null,
        created_at: '2026-09-05T00:00:00Z',
        updated_at: '2026-09-05T00:00:00Z',
        applied_at: null,
    };
}

function makeState(runId: string, ids: string[], overrides: Record<string, unknown> = {}): SelectionState {
    return {
        run: {
            id: runId,
            name: 'Client final',
            status: 'active',
            source_count: 10,
            shortlist_count: ids.length,
            target_count: null,
            source_scope: { type: 'all', include_rejected: false },
            created_at: '2026-09-05T00:00:00Z',
            updated_at: '2026-09-05T00:00:00Z',
            finished_at: null,
            rejected_shortlist_count: 0,
            ...overrides,
        },
        shortlist_ids: ids,
    };
}

function resetStores() {
    selectionRun.set(null);
    selectedIds.set(new Set());
    shortlistIds.set(new Set());
    toasts.set([]);
    activeAgentProposalId.set(null);
    vi.clearAllMocks();
}

beforeEach(resetStores);

describe('shortlist proposal kinds', () => {
    it('recognizes only the two shortlist kinds', () => {
        expect(isShortlistProposalKind('shortlist_add')).toBe(true);
        expect(isShortlistProposalKind('shortlist_remove')).toBe(true);
        expect(isShortlistProposalKind('select_images')).toBe(false);
        expect(isShortlistProposalKind('trash_images')).toBe(false);
    });

    it('labels the group action distinctly from highlight and trash actions', () => {
        expect(shortlistProposalActionLabel('shortlist_add')).toBe('Add approved to shortlist');
        expect(shortlistProposalActionLabel('shortlist_remove')).toBe('Remove approved from shortlist');
    });

    it('reads the captured target from the source context', () => {
        expect(sourceContextSelectionId({ selection_id: ' run-1 ' })).toBe('run-1');
        expect(sourceContextSelectionId({})).toBeNull();
        expect(sourceContextSelectionId({ selection_id: '  ' })).toBeNull();
    });
});

describe('applyShortlistProposal', () => {
    it('applies exact approved IDs — including off-screen ones — to the captured run', async () => {
        // Run A is captured in the proposal; run B happens to be open.
        selectionRun.set(makeState('run-b', []).run);
        apiMocks.applyActionProposal.mockResolvedValue({
            proposal_id: 'prop-shortlist_add',
            status: 'applied',
            applied_count: 2,
            failed_count: 0,
            result_json: '{}',
        });
        apiMocks.getSelectionRun.mockResolvedValue(makeState('run-a', ['off-screen-1', 'off-screen-2']));

        let applied = false;
        await applyShortlistProposal(makeProposal('shortlist_add', 'run-a'), ['off-screen-1', 'off-screen-2'], {
            onApplied: () => { applied = true; },
        });

        // Approved IDs are used exactly as reviewed, unfiltered by visibility.
        expect(apiMocks.applyActionProposal).toHaveBeenCalledWith(
            'prop-shortlist_add',
            ['off-screen-1', 'off-screen-2'],
            JSON.stringify({ approved_count: 2, selection_id: 'run-a' }),
        );
        // The captured target run is refreshed separately after the commit.
        expect(apiMocks.getSelectionRun).toHaveBeenCalledWith('run-a');
        expect(applied).toBe(true);
        // The proposal never writes transient highlights.
        expect(get(selectedIds)).toEqual(new Set());
        // The currently open other run is not replaced.
        expect(get(selectionRun)?.id).toBe('run-b');
        expect(get(shortlistIds)).toEqual(new Set());
    });

    it('feeds the refreshed captured run through the update path when it is open', async () => {
        selectionRun.set(makeState('run-a', ['img-1']).run);
        shortlistIds.set(new Set(['img-1']));
        apiMocks.applyActionProposal.mockResolvedValue({
            proposal_id: 'prop-shortlist_remove',
            status: 'applied',
            applied_count: 1,
            failed_count: 0,
            result_json: '{}',
        });
        apiMocks.getSelectionRun.mockResolvedValue(makeState('run-a', []));

        await applyShortlistProposal(makeProposal('shortlist_remove', 'run-a'), ['img-1']);

        expect(get(shortlistIds)).toEqual(new Set());
        expect(get(selectionRun)?.shortlist_count).toBe(0);
    });

    it('keeps the proposal retryable when the apply fails, reusing the same approved IDs', async () => {
        selectionRun.set(makeState('run-a', []).run);
        apiMocks.applyActionProposal
            .mockRejectedValueOnce(new Error('database is locked'))
            .mockResolvedValueOnce({
                proposal_id: 'prop-shortlist_add',
                status: 'applied',
                applied_count: 2,
                failed_count: 0,
                result_json: '{}',
            });
        apiMocks.getSelectionRun.mockResolvedValue(makeState('run-a', ['off-screen-1', 'off-screen-2']));

        let applied = false;
        await applyShortlistProposal(makeProposal('shortlist_add', 'run-a'), ['off-screen-1', 'off-screen-2'], {
            onApplied: () => { applied = true; },
        });

        // Nothing was applied, nothing refreshed, proposal stays pending.
        expect(apiMocks.getSelectionRun).not.toHaveBeenCalled();
        expect(applied).toBe(false);
        const toast = get(toasts).find(t => t.type === 'error');
        expect(toast?.actions?.[0]?.label).toBe('Retry');

        // Retry reuses the exact approved IDs and completes the flow.
        await toast!.actions![0].onclick();
        await new Promise(resolve => setTimeout(resolve, 0));
        expect(apiMocks.applyActionProposal).toHaveBeenLastCalledWith(
            'prop-shortlist_add',
            ['off-screen-1', 'off-screen-2'],
            expect.any(String),
        );
        expect(apiMocks.getSelectionRun).toHaveBeenCalledWith('run-a');
        expect(applied).toBe(true);
    });

    it('never reapplies the mutation when only the post-commit refresh fails', async () => {
        selectionRun.set(makeState('run-a', []).run);
        apiMocks.applyActionProposal.mockResolvedValue({
            proposal_id: 'prop-shortlist_add',
            status: 'applied',
            applied_count: 1,
            failed_count: 0,
            result_json: '{}',
        });
        apiMocks.getSelectionRun.mockRejectedValue(new Error('read failed'));

        let applied = false;
        await applyShortlistProposal(makeProposal('shortlist_add', 'run-a'), ['off-screen-1'], {
            onApplied: () => { applied = true; },
        });

        expect(apiMocks.applyActionProposal).toHaveBeenCalledTimes(1);
        expect(apiMocks.getSelectionRun).toHaveBeenCalledTimes(1);
        // The commit is truthful: success surfaces even though the refresh failed.
        expect(applied).toBe(true);
        const warning = get(toasts).find(t => t.type === 'warning');
        expect(warning?.message).toContain('could not be refreshed');
    });

    it('refuses to apply without a captured target instead of guessing the open run', async () => {
        selectionRun.set(makeState('run-open', []).run);
        apiMocks.applyActionProposal.mockResolvedValue({});

        await applyShortlistProposal(makeProposal('shortlist_add', null), ['off-screen-1']);

        expect(apiMocks.applyActionProposal).not.toHaveBeenCalled();
        const warning = get(toasts).find(t => t.type === 'warning');
        expect(warning?.message).toContain('no captured selection target');
        expect(get(selectionRun)?.id).toBe('run-open');
    });

    it('does nothing when no images were approved', async () => {
        await applyShortlistProposal(makeProposal('shortlist_add', 'run-a'), []);
        expect(apiMocks.applyActionProposal).not.toHaveBeenCalled();
    });
});