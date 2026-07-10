import type { AgentVisualLevel } from '$lib/api';

export function effectiveAgentVisualLevel({
    requestedVisualLevel,
    candidateCount,
    thumbnailCount,
}: {
    requestedVisualLevel: AgentVisualLevel;
    candidateCount: number;
    thumbnailCount: number;
}): AgentVisualLevel {
    if (requestedVisualLevel === 'text' && candidateCount > 0 && thumbnailCount > 0) {
        return 'preview';
    }
    return requestedVisualLevel;
}
