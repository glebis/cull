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

    it('uses live candidate estimates and active proposal selection for the context chip', () => {
        expect(source).toContain("from '$lib/agent-token-estimate'");
        expect(source).toContain('activeProposalId ? proposals.find');
        expect(source).toContain('candidateCount');
        expect(source).toContain('estimateAgentBudget');
    });

    it('supports editable selection presets through the chat panel', () => {
        expect(source).toContain('Selection presets');
        expect(source).toContain('onclick={() => selectPreset(preset.id)}');
        expect(source).toContain('startEditActivePreset');
        expect(source).toContain('onupdatepreset(editingPresetId, presetPromptDraft.trim())');
        expect(source).toContain('aria-label="Preset prompt"');
        expect(source).toContain('Save preset');
    });

    it('creates and reviews proposals through callback props', () => {
        expect(source).toContain('oncreateproposal(activePreset?.id ?? null, message)');
        expect(source).toContain('onreviewproposal(activeProposal.id)');
        expect(source).toContain('ondismissproposal(activeProposal.id)');
        expect(source).not.toContain('dispatchEvent');
    });

    it('renders agent chat as a compact conversation rather than an SDK event rail', () => {
        expect(source).toContain('ClaudeAgentStreamEvent');
        expect(source).toContain('streamEvents = []');
        expect(source).toContain('lastInstruction = null');
        expect(source).toContain('latestRunEvent = $derived');
        expect(source).toContain('statusForEvent(latestRunEvent, busy)');
        expect(source).toContain('class="chat-thread"');
        expect(source).toContain('class="chat-message user-message"');
        expect(source).toContain('class="chat-message assistant-message"');
        expect(source).toContain('aria-live="polite"');
        expect(source).not.toContain('Agent run events');
        expect(source).not.toContain('{event.phase.replace');
    });
});
