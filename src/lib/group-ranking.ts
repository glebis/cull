// Explainable best-of-group ranking. Given the members of a similarity group,
// score each on a few transparent components and surface a suggested winner —
// never auto-deleting or auto-selecting. The user can override the winner and
// that choice is honored downstream.

export interface GroupQuality {
    focus_score?: number | null;
    blur_score?: number | null;
    exposure_score?: number | null;
}

export interface GroupMemberInput {
    imageId: string;
    starRating: number; // 0-5
    decision: 'accept' | 'reject' | 'undecided' | null;
    similarityRank: number; // 0-based; 0 = group representative
    quality?: GroupQuality | null;
}

export interface RankWeights {
    rating: number;
    decision: number;
    quality: number;
    representativeness: number;
}

export const DEFAULT_RANK_WEIGHTS: RankWeights = {
    rating: 0.4,
    decision: 0.3,
    quality: 0.2,
    representativeness: 0.1,
};

export interface ScoreComponents {
    rating: number;
    decision: number;
    quality: number;
    representativeness: number;
}

export interface RankedMember {
    imageId: string;
    score: number;
    components: ScoreComponents;
    isSuggestedWinner: boolean;
    isOverriddenWinner: boolean;
}

export interface RankedGroup {
    members: RankedMember[];
    suggestedWinnerId: string | null;
    overriddenWinnerId: string | null;
    // The winner to act on: the override when set, else the suggestion.
    effectiveWinnerId: string | null;
}

function decisionScore(decision: GroupMemberInput['decision']): number {
    switch (decision) {
        case 'accept':
            return 1;
        case 'reject':
            return 0;
        default:
            return 0.5;
    }
}

// Group-relative quality: normalize focus across the group (sharper = better),
// lightly penalize members whose exposure deviates from mid-tone. Absent
// metrics fall back to a neutral 0.5 so they neither help nor hurt.
function qualityScores(members: GroupMemberInput[]): number[] {
    const focus = members.map(m => m.quality?.focus_score ?? null);
    const known = focus.filter((f): f is number => f !== null);
    const min = known.length ? Math.min(...known) : 0;
    const max = known.length ? Math.max(...known) : 0;
    const span = max - min;

    return members.map(m => {
        const f = m.quality?.focus_score ?? null;
        if (f === null) return 0.5;
        const normFocus = span > 0 ? (f - min) / span : 0.5;
        const exposure = m.quality?.exposure_score;
        // exposure_score expected ~0..1 with 0.5 ideal; penalty up to ~0.2.
        const exposurePenalty =
            typeof exposure === 'number' ? Math.min(0.2, Math.abs(exposure - 0.5) * 0.4) : 0;
        return Math.max(0, normFocus - exposurePenalty);
    });
}

function representativeness(rank: number, groupSize: number): number {
    if (groupSize <= 1) return 1;
    return Math.max(0, (groupSize - 1 - rank) / (groupSize - 1));
}

export function rankGroupMembers(
    members: GroupMemberInput[],
    weights: RankWeights = DEFAULT_RANK_WEIGHTS,
    overriddenWinnerId: string | null = null,
): RankedGroup {
    if (members.length === 0) {
        return { members: [], suggestedWinnerId: null, overriddenWinnerId, effectiveWinnerId: overriddenWinnerId };
    }

    const quality = qualityScores(members);
    const size = members.length;

    const scored = members.map((m, i) => {
        const components: ScoreComponents = {
            rating: Math.max(0, Math.min(1, m.starRating / 5)),
            decision: decisionScore(m.decision),
            quality: quality[i],
            representativeness: representativeness(m.similarityRank, size),
        };
        const score =
            components.rating * weights.rating +
            components.decision * weights.decision +
            components.quality * weights.quality +
            components.representativeness * weights.representativeness;
        return { imageId: m.imageId, score, components, similarityRank: m.similarityRank };
    });

    // Sort by score desc; break ties toward the more representative member.
    scored.sort((a, b) => b.score - a.score || a.similarityRank - b.similarityRank);

    const suggestedWinnerId = scored[0]?.imageId ?? null;
    const hasOverride = overriddenWinnerId !== null && scored.some(s => s.imageId === overriddenWinnerId);
    const effectiveWinnerId = hasOverride ? overriddenWinnerId : suggestedWinnerId;

    const rankedMembers: RankedMember[] = scored.map(s => ({
        imageId: s.imageId,
        score: s.score,
        components: s.components,
        isSuggestedWinner: s.imageId === suggestedWinnerId,
        isOverriddenWinner: hasOverride && s.imageId === overriddenWinnerId,
    }));

    return {
        members: rankedMembers,
        suggestedWinnerId,
        overriddenWinnerId: hasOverride ? overriddenWinnerId : null,
        effectiveWinnerId,
    };
}
