import { describe, expect, it } from 'vitest';
import { nextThreeStagePresentationState } from './presentation-utils';

describe('nextThreeStagePresentationState', () => {
    it('cycles normal to zen, then image-only zen, then back to normal', () => {
        expect(nextThreeStagePresentationState({ zen: false, imageOnly: false })).toEqual({
            zen: true,
            imageOnly: false,
        });
        expect(nextThreeStagePresentationState({ zen: true, imageOnly: false })).toEqual({
            zen: true,
            imageOnly: true,
        });
        expect(nextThreeStagePresentationState({ zen: true, imageOnly: true })).toEqual({
            zen: false,
            imageOnly: false,
        });
    });

    it('normalizes stale image-only state when zen is already off', () => {
        expect(nextThreeStagePresentationState({ zen: false, imageOnly: true })).toEqual({
            zen: true,
            imageOnly: false,
        });
    });
});
