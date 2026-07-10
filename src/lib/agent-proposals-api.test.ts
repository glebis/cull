import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import {
    applyActionProposal,
    cancelClaudeAgentChatTurn,
    createActionProposal,
    listAgentSelectionPresets,
    listActionProposals,
    runClaudeAgentChatTurn,
    trashImagesDetailed,
    upsertAgentSelectionPreset,
} from './api';

describe('agent proposal API wrappers', () => {
    it('invokes trash_images_detailed with imageIds', async () => {
        vi.mocked(invoke).mockResolvedValueOnce({
            requested: 1,
            succeeded: 1,
            failed: 0,
            results: [{ image_id: 'img_1', path: '/tmp/a.png', status: 'trashed', error: null }],
        });

        await expect(trashImagesDetailed(['img_1'])).resolves.toMatchObject({
            requested: 1,
            succeeded: 1,
            failed: 0,
        });
        expect(invoke).toHaveBeenCalledWith('trash_images_detailed', { imageIds: ['img_1'] });
    });

    it('invokes action proposal commands with typed payloads', async () => {
        vi.mocked(invoke).mockResolvedValueOnce([]);
        await listActionProposals('pending', 12);
        expect(invoke).toHaveBeenCalledWith('list_action_proposals', {
            status: 'pending',
            limit: 12,
        });

        vi.mocked(invoke).mockResolvedValueOnce({ id: 'proposal_1' });
        const request = {
            kind: 'select_images',
            persona: 'copilot' as const,
            lens: 'portfolio',
            criteria: 'Select portfolio images',
            visual_level: 'tiny' as const,
            selection_preset_id: 'selpreset_portfolio',
            estimated_input_tokens: 2100,
            estimated_output_tokens: 420,
            estimated_cost_eur: 0.014,
            source_context_json: '{}',
            items_json: '[{"image_id":"img_1"}]',
            guard_results_json: '{}',
        };
        await createActionProposal(request);
        expect(invoke).toHaveBeenCalledWith('create_action_proposal', { request });

        vi.mocked(invoke).mockResolvedValueOnce({ status: 'applied' });
        await applyActionProposal('proposal_1', ['img_1'], '{"applied":1}');
        expect(invoke).toHaveBeenCalledWith('apply_action_proposal', {
            proposalId: 'proposal_1',
            approvedImageIds: ['img_1'],
            resultJson: '{"applied":1}',
        });
    });

    it('invokes editable selection preset commands', async () => {
        vi.mocked(invoke).mockResolvedValueOnce([]);
        await listAgentSelectionPresets();
        expect(invoke).toHaveBeenCalledWith('list_agent_selection_presets');

        const request = {
            id: 'selpreset_portfolio',
            name: 'Portfolio edit',
            purpose: 'portfolio',
            prompt: 'Select portfolio candidates',
            criteria_json: '{}',
            sort_order: 10,
        };
        vi.mocked(invoke).mockResolvedValueOnce({ ...request, created_at: '', updated_at: '' });
        await upsertAgentSelectionPreset(request);
        expect(invoke).toHaveBeenCalledWith('upsert_agent_selection_preset', { request });
    });

    it('invokes Claude agent chat turn with candidate context', async () => {
        const request = {
            request_id: 'agent-request-1',
            instruction: 'select portfolio candidates',
            visual_level: 'tiny' as const,
            preset: null,
            candidate_images: [{
                image_id: 'img_1',
                filename: 'one.png',
                width: 1024,
                height: 768,
                format: 'png',
                star_rating: 4,
                color_label: null,
                decision: 'undecided',
                source_label: 'midjourney',
                thumbnail_path: '/tmp/thumbs/one.png',
            }],
            selected_count: 1,
            visible_count: 12,
            view_context_json: '{"kind":"folder","label":"Portfolio","path":"/art/portfolio"}',
            model: null,
            max_budget_usd: null,
        };
        vi.mocked(invoke).mockResolvedValueOnce({
            operation: 'create_proposal',
            message: 'Created',
            proposal: { id: 'proposal_1' },
            updated_preset: null,
            usage_json: '{}',
            raw_result_json: '{}',
        });

        await runClaudeAgentChatTurn(request);
        expect(invoke).toHaveBeenCalledWith('run_claude_agent_chat_turn', { request });
    });

    it('invokes Claude agent chat turn cancellation by request id', async () => {
        vi.mocked(invoke).mockResolvedValueOnce(true);

        await expect(cancelClaudeAgentChatTurn('agent-request-1')).resolves.toBe(true);
        expect(invoke).toHaveBeenCalledWith('cancel_claude_agent_chat_turn', {
            requestId: 'agent-request-1',
        });
    });
});
