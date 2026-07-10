// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import type { AgentActionProposal } from '$lib/api';
import ActionProposalReviewDialog from './ActionProposalReviewDialog.svelte';

const proposal: AgentActionProposal = {
    id: 'proposal-1',
    kind: 'trash_images',
    status: 'pending',
    persona: 'curator',
    lens: null,
    criteria: 'Remove weak variants',
    visual_level: 'tiny',
    selection_preset_id: null,
    estimated_input_tokens: 100,
    estimated_output_tokens: 20,
    estimated_cost_eur: 0.002,
    source_context_json: '{}',
    items_json: JSON.stringify([
        { image_id: 'image-1', reason: 'soft focus' },
        { image_id: 'image-2', reason: 'duplicate' },
    ]),
    guard_results_json: '{}',
    apply_result_json: null,
    undo_journal_json: null,
    created_at: '2026-07-10T09:00:00Z',
    updated_at: '2026-07-10T09:00:00Z',
    applied_at: null,
};

const selectionProposal: AgentActionProposal = {
    ...proposal,
    id: 'proposal-2',
    kind: 'select_images',
};

afterEach(() => cleanup());

describe('ActionProposalReviewDialog rendered behavior', () => {
    it('renders the selection-proposal title and apply action label', () => {
        render(ActionProposalReviewDialog, {
            proposal: selectionProposal,
            visible: true,
            visibleImages: [],
            onapplyproposal: vi.fn(),
            oncancelreview: vi.fn(),
        });

        expect(screen.getByRole('dialog', { name: 'Review selection proposal' })).toBeVisible();
        expect(screen.getByRole('button', { name: 'Select approved' })).toBeEnabled();
    });

    it('submits only checked proposal candidates', async () => {
        const user = userEvent.setup();
        const onapplyproposal = vi.fn();
        render(ActionProposalReviewDialog, {
            proposal,
            visible: true,
            visibleImages: [],
            onapplyproposal,
            oncancelreview: vi.fn(),
        });

        expect(screen.getByRole('dialog', { name: 'Review Trash proposal' })).toBeVisible();
        expect(screen.getByRole('checkbox', { name: 'Include image-1' })).toBeChecked();
        await user.click(screen.getByRole('checkbox', { name: 'Include image-2' }));
        await user.click(screen.getByRole('button', { name: 'Move approved to Trash' }));

        expect(onapplyproposal).toHaveBeenCalledWith('proposal-1', ['image-1']);
    });

    it('cancels on Escape without applying the proposal', async () => {
        const user = userEvent.setup();
        const onapplyproposal = vi.fn();
        const oncancelreview = vi.fn();
        render(ActionProposalReviewDialog, {
            proposal,
            visible: true,
            visibleImages: [],
            onapplyproposal,
            oncancelreview,
        });

        await user.keyboard('{Escape}');

        expect(oncancelreview).toHaveBeenCalledOnce();
        expect(onapplyproposal).not.toHaveBeenCalled();
    });
});
