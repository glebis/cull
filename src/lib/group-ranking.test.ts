import { describe, expect, it } from 'vitest';
import { rankGroupMembers, type GroupMemberInput } from './group-ranking';

function member(partial: Partial<GroupMemberInput> & { imageId: string }): GroupMemberInput {
    return {
        starRating: 0,
        decision: null,
        similarityRank: 0,
        quality: null,
        ...partial,
    };
}

describe('best-of-group ranking', () => {
    it('suggests the highest-rated member as winner', () => {
        const ranked = rankGroupMembers([
            member({ imageId: 'a', starRating: 2, similarityRank: 0 }),
            member({ imageId: 'b', starRating: 5, similarityRank: 1 }),
            member({ imageId: 'c', starRating: 1, similarityRank: 2 }),
        ]);
        expect(ranked.suggestedWinnerId).toBe('b');
        expect(ranked.members[0].imageId).toBe('b');
        expect(ranked.members[0].isSuggestedWinner).toBe(true);
    });

    it('exposes explainable score components for each member', () => {
        const ranked = rankGroupMembers([
            member({ imageId: 'a', starRating: 5, decision: 'accept', similarityRank: 0 }),
        ]);
        const c = ranked.members[0].components;
        expect(c.rating).toBe(1);
        expect(c.decision).toBe(1);
        expect(c.representativeness).toBe(1);
        expect(c).toHaveProperty('quality');
    });

    it('penalizes rejected members below undecided ones at equal rating', () => {
        const ranked = rankGroupMembers([
            member({ imageId: 'rejected', starRating: 3, decision: 'reject', similarityRank: 0 }),
            member({ imageId: 'undecided', starRating: 3, decision: 'undecided', similarityRank: 1 }),
        ]);
        expect(ranked.suggestedWinnerId).toBe('undecided');
    });

    it('uses focus quality to break a rating/decision tie', () => {
        const ranked = rankGroupMembers([
            member({ imageId: 'soft', starRating: 4, similarityRank: 0, quality: { focus_score: 0.1 } }),
            member({ imageId: 'sharp', starRating: 4, similarityRank: 1, quality: { focus_score: 0.9 } }),
        ]);
        expect(ranked.suggestedWinnerId).toBe('sharp');
    });

    it('honors an explicit user override without changing the suggestion', () => {
        const ranked = rankGroupMembers(
            [
                member({ imageId: 'a', starRating: 5, similarityRank: 0 }),
                member({ imageId: 'b', starRating: 2, similarityRank: 1 }),
            ],
            undefined,
            'b',
        );
        expect(ranked.suggestedWinnerId).toBe('a');
        expect(ranked.overriddenWinnerId).toBe('b');
        expect(ranked.effectiveWinnerId).toBe('b');
        expect(ranked.members.find(m => m.imageId === 'b')?.isOverriddenWinner).toBe(true);
    });

    it('ignores an override that does not match any member', () => {
        const ranked = rankGroupMembers(
            [member({ imageId: 'a', starRating: 5 })],
            undefined,
            'ghost',
        );
        expect(ranked.overriddenWinnerId).toBeNull();
        expect(ranked.effectiveWinnerId).toBe('a');
    });

    it('handles an empty group safely', () => {
        const ranked = rankGroupMembers([]);
        expect(ranked.members).toHaveLength(0);
        expect(ranked.suggestedWinnerId).toBeNull();
    });
});
