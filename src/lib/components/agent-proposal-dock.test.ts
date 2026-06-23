import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const source = readFileSync(join(process.cwd(), 'src/lib/components/AgentProposalDock.svelte'), 'utf8');

describe('AgentProposalDock source contract', () => {
    it('keeps visual level and cost visible as secondary context', () => {
        expect(source).toContain('Context: {contextLabel}');
        expect(source).toContain("activeProposal?.estimated_cost_eur?.toFixed(3)");
        expect(source).toContain('activeProposal?.estimated_input_tokens');
        expect(source).toContain('context-chip');
    });

    it('supports editable selection presets through the chat panel', () => {
        expect(source).toContain('Selection presets');
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
});
