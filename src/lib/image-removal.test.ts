import { describe, expect, it } from 'vitest';
import { clampFocusIndexToList, nextFocusIndexAfterFocusedRemoval } from './image-removal';

describe('nextFocusIndexAfterFocusedRemoval', () => {
    it('keeps the removed index when there is a next image', () => {
        expect(nextFocusIndexAfterFocusedRemoval(1, 4)).toBe(1);
    });

    it('wraps to the first image when the removed image was last', () => {
        expect(nextFocusIndexAfterFocusedRemoval(3, 4)).toBe(0);
    });

    it('keeps focus at zero after removing the only image', () => {
        expect(nextFocusIndexAfterFocusedRemoval(0, 1)).toBe(0);
    });
});

describe('clampFocusIndexToList', () => {
    it('preserves in-range focus', () => {
        expect(clampFocusIndexToList(2, 5)).toBe(2);
    });

    it('falls back to first image when focus is outside the current list', () => {
        expect(clampFocusIndexToList(5, 5)).toBe(0);
    });
});
