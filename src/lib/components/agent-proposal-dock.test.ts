import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const source = readFileSync(join(process.cwd(), 'src/lib/components/AgentProposalDock.svelte'), 'utf8');

describe('AgentProposalDock source contract', () => {
    it('keeps visual level and cost visible as secondary context', () => {
        expect(source).toContain('Context {contextLabel}');
        expect(source).toContain('displayCostEur.toFixed(3)');
        expect(source).toContain('displayInputTokens');
        expect(source).toContain('context-chip');
    });

    it('keeps header controls minimal', () => {
        expect(source).toContain('aria-label="Close agent panel"');
        expect(source).toContain('>×</button>');
        expect(source).toContain('close-button');
        expect(source).not.toContain('onpintoggle');
        expect(source).not.toContain('icon-button');
    });

    it('uses live candidate estimates and active proposal selection for the context chip', () => {
        expect(source).toContain("from '$lib/agent-token-estimate'");
        expect(source).toContain('activeProposalId ? proposals.find');
        expect(source).toContain('candidateCount');
        expect(source).toContain('estimateAgentBudget');
    });

    it('supports editable selection profiles through a compact selector', () => {
        expect(source).toContain('Agent profile');
        expect(source).toContain('Selection criteria');
        expect(source).toContain('onchange={handleProfileChange}');
        expect(source).toContain('profileSummary(activePreset)');
        expect(source).toContain('preset.prompt.replace');
        expect(source).toContain('startEditActivePreset');
        expect(source).toContain('onupdatepreset(editingPresetId, presetPromptDraft.trim())');
        expect(source).toContain('aria-label="Preset prompt"');
        expect(source).toContain('Save preset');
    });

    it('creates and reviews proposals through callback props', () => {
        expect(source).toContain('oncreateproposal(activePreset?.id ?? null, message)');
        expect(source).toContain('onreviewproposal(activeProposal.id)');
        expect(source).toContain('ondismissproposal(activeProposal.id)');
        expect(source).toContain('onselectproposal?: (proposalId: string) => void');
        expect(source).not.toContain('dispatchEvent');
    });

    it('keeps pending proposal state visible above the chat transcript', () => {
        const proposalIndex = source.indexOf('aria-label="Pending agent proposal"');
        const profileIndex = source.indexOf('aria-label="Agent profile"');
        const chatIndex = source.indexOf('aria-label="Agent chat"');
        expect(proposalIndex).toBeGreaterThan(-1);
        expect(proposalIndex).toBeLessThan(profileIndex);
        expect(proposalIndex).toBeLessThan(chatIndex);
        expect(source).toContain('Needs approval');
        expect(source).toContain('Review and apply');
        expect(source).toContain('candidateCountLabel');
        expect(source).toContain('visibleImages');
        expect(source).toContain('candidate-preview');
        expect(source).toContain('safeAssetPreviewPath');
        expect(source).toContain('proposal-switcher');
        expect(source).toContain('pendingProposals.length');
        expect(source).not.toContain('proposal-criteria');
        expect(source).toContain('flex: 1 1 auto;');
        expect(source).toContain('min-height: 220px');
    });

    it('renders agent chat as a compact live conversation with optional activity', () => {
        expect(source).toContain('ClaudeAgentStreamEvent');
        expect(source).toContain('streamEvents = []');
        expect(source).toContain('lastInstruction = null');
        expect(source).toContain('latestRunEvent = $derived');
        expect(source).toContain('latestAssistantEvent = $derived');
        expect(source).toContain('statusForEvent(latestRunEvent, busy)');
        expect(source).toContain('event.message && !');
        expect(source).toContain('class="chat-thread"');
        expect(source).toContain('class="chat-message user-message"');
        expect(source).toContain('class="chat-message assistant-message"');
        expect(source).toContain('class="activity-log"');
        expect(source).toContain('handleInstructionKeydown');
        expect(source).toContain('aria-live="polite"');
        expect(source).not.toContain('Streaming content_block_delta');
    });
});
