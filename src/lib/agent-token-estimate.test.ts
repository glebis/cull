import { describe, expect, it } from 'vitest';
import { estimateAgentBudget } from './agent-token-estimate';

describe('agent token budget estimate', () => {
    it('returns zero when no images can be sent to the agent', () => {
        expect(estimateAgentBudget({
            candidateCount: 0,
            instruction: 'select the best images',
            visualLevel: 'preview',
        })).toEqual({ inputTokens: 0, outputTokens: 0, costEur: 0 });
    });

    it('produces non-zero preview estimates for visible candidate context', () => {
        const estimate = estimateAgentBudget({
            candidateCount: 12,
            instruction: 'select the strongest portfolio images',
            visualLevel: 'preview',
        });

        expect(estimate.inputTokens).toBeGreaterThan(0);
        expect(estimate.outputTokens).toBe(420);
        expect(estimate.costEur).toBeGreaterThan(0);
    });

    it('scales visual estimates by selected context level', () => {
        const text = estimateAgentBudget({
            candidateCount: 4,
            instruction: '',
            visualLevel: 'text',
        });
        const preview = estimateAgentBudget({
            candidateCount: 4,
            instruction: '',
            visualLevel: 'preview',
        });
        const full = estimateAgentBudget({
            candidateCount: 4,
            instruction: '',
            visualLevel: 'full',
        });

        expect(text.inputTokens).toBeLessThan(preview.inputTokens);
        expect(preview.inputTokens).toBeLessThan(full.inputTokens);
    });
});
