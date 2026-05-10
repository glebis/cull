import { describe, it, expect } from 'vitest';
import {
    computeTinderStats,
    totalPairs,
    pairBaseIndex,
    isDone,
    parseVoiceCommand,
    filterByDecision,
} from './tinder-utils';

describe('computeTinderStats', () => {
    it('counts left choice as 1 accepted + 1 rejected', () => {
        const stats = computeTinderStats([{ choice: 'left' }]);
        expect(stats).toEqual({ accepted: 1, rejected: 1, skipped: 0 });
    });

    it('counts right choice as 1 accepted + 1 rejected', () => {
        const stats = computeTinderStats([{ choice: 'right' }]);
        expect(stats).toEqual({ accepted: 1, rejected: 1, skipped: 0 });
    });

    it('counts skip as 2 skipped', () => {
        const stats = computeTinderStats([{ choice: 'skip' }]);
        expect(stats).toEqual({ accepted: 0, rejected: 0, skipped: 2 });
    });

    it('accumulates across multiple entries', () => {
        const stats = computeTinderStats([
            { choice: 'left' },
            { choice: 'right' },
            { choice: 'skip' },
            { choice: 'left' },
        ]);
        expect(stats).toEqual({ accepted: 3, rejected: 3, skipped: 2 });
    });

    it('returns zeros for empty history', () => {
        expect(computeTinderStats([])).toEqual({ accepted: 0, rejected: 0, skipped: 0 });
    });
});

describe('totalPairs', () => {
    it('returns half for even count', () => {
        expect(totalPairs(10)).toBe(5);
    });

    it('rounds up for odd count', () => {
        expect(totalPairs(9)).toBe(5);
    });

    it('returns 0 for 0 images', () => {
        expect(totalPairs(0)).toBe(0);
    });

    it('returns 1 for 1 image', () => {
        expect(totalPairs(1)).toBe(1);
    });
});

describe('pairBaseIndex', () => {
    it('returns pair * 2', () => {
        expect(pairBaseIndex(0)).toBe(0);
        expect(pairBaseIndex(3)).toBe(6);
    });
});

describe('isDone', () => {
    it('returns false when pairs remain', () => {
        expect(isDone(0, 10)).toBe(false);
        expect(isDone(3, 10)).toBe(false);
    });

    it('returns true on last pair', () => {
        expect(isDone(4, 10)).toBe(true);
    });

    it('returns true past last pair', () => {
        expect(isDone(5, 10)).toBe(true);
    });

    it('returns true for 0 images', () => {
        expect(isDone(0, 0)).toBe(true);
    });
});

describe('parseVoiceCommand', () => {
    it('detects left in English', () => {
        expect(parseVoiceCommand('Left')).toBe('left');
        expect(parseVoiceCommand('choose left one')).toBe('left');
    });

    it('detects left in Russian', () => {
        expect(parseVoiceCommand('левый')).toBe('left');
        expect(parseVoiceCommand('Левая картинка')).toBe('left');
    });

    it('detects right in English', () => {
        expect(parseVoiceCommand('Right')).toBe('right');
        expect(parseVoiceCommand('the right image')).toBe('right');
    });

    it('detects right in Russian', () => {
        expect(parseVoiceCommand('правый')).toBe('right');
        expect(parseVoiceCommand('Правая')).toBe('right');
    });

    it('detects skip', () => {
        expect(parseVoiceCommand('skip')).toBe('skip');
        expect(parseVoiceCommand('пропустить')).toBe('skip');
    });

    it('detects undo', () => {
        expect(parseVoiceCommand('undo')).toBe('undo');
        expect(parseVoiceCommand('отменить')).toBe('undo');
    });

    it('returns null for unrecognized text', () => {
        expect(parseVoiceCommand('hello world')).toBeNull();
        expect(parseVoiceCommand('')).toBeNull();
    });
});

describe('filterByDecision', () => {
    const history = [
        { leftId: 'a', rightId: 'b', choice: 'left' },
        { leftId: 'c', rightId: 'd', choice: 'right' },
        { leftId: 'e', rightId: 'f', choice: 'skip' },
    ];

    it('filters accepted images', () => {
        const accepted = filterByDecision(history, 'accepted');
        expect(accepted).toEqual(new Set(['a', 'd']));
    });

    it('filters rejected images', () => {
        const rejected = filterByDecision(history, 'rejected');
        expect(rejected).toEqual(new Set(['b', 'c']));
    });

    it('skipped pairs appear in neither', () => {
        const accepted = filterByDecision(history, 'accepted');
        const rejected = filterByDecision(history, 'rejected');
        expect(accepted.has('e')).toBe(false);
        expect(accepted.has('f')).toBe(false);
        expect(rejected.has('e')).toBe(false);
        expect(rejected.has('f')).toBe(false);
    });

    it('returns empty set for empty history', () => {
        expect(filterByDecision([], 'accepted')).toEqual(new Set());
    });
});
