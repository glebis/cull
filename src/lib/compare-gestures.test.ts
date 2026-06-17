import { describe, expect, it } from 'vitest';
import { nextCompareFocusedIndex } from './compare-gestures';

describe('compare gesture navigation', () => {
    it('advances adjacent compare pairs by two images', () => {
        expect(nextCompareFocusedIndex(0, 10, 'next')).toBe(2);
        expect(nextCompareFocusedIndex(2, 10, 'previous')).toBe(0);
    });

    it('clamps compare pair navigation at collection bounds', () => {
        expect(nextCompareFocusedIndex(8, 10, 'next')).toBe(8);
        expect(nextCompareFocusedIndex(0, 10, 'previous')).toBe(0);
        expect(nextCompareFocusedIndex(0, 1, 'next')).toBe(0);
    });
});
