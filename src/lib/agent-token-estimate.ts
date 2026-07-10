import type { AgentVisualLevel } from '$lib/api';

const ESTIMATED_EUR_PER_INPUT_TOKEN = 0.000006;

const VISUAL_TOKEN_WEIGHTS: Record<AgentVisualLevel, number> = {
    text: 70,
    tiny: 160,
    preview: 260,
    full: 650,
};

export interface AgentBudgetEstimate {
    inputTokens: number;
    outputTokens: number;
    costEur: number;
}

export function estimateAgentBudget({
    candidateCount,
    instruction,
    visualLevel,
}: {
    candidateCount: number;
    instruction: string;
    visualLevel: AgentVisualLevel;
}): AgentBudgetEstimate {
    if (candidateCount <= 0) {
        return { inputTokens: 0, outputTokens: 0, costEur: 0 };
    }

    const promptTokens = instruction.trim().length / 4;
    const contextOverhead = 550;
    const imageTokens = candidateCount * VISUAL_TOKEN_WEIGHTS[visualLevel];
    const inputTokens = Math.round(Math.max(300, contextOverhead + imageTokens + promptTokens));
    const outputTokens = 420;
    const costEur = Number((inputTokens * ESTIMATED_EUR_PER_INPUT_TOKEN).toFixed(3));

    return { inputTokens, outputTokens, costEur };
}
