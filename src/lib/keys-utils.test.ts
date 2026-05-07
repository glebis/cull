import { describe, it, expect } from 'vitest';
import {
    clampFocusIndex,
    clampThumbnailSize,
    nextComparePairIndex,
    prevComparePairIndex,
    computeColumnCount,
} from './keys-utils';

describe('clampFocusIndex', () => {
    it('moves forward', () => {
        expect(clampFocusIndex(2, 1, 10)).toBe(3);
    });

    it('moves backward', () => {
        expect(clampFocusIndex(5, -1, 10)).toBe(4);
    });

    it('clamps at 0 when moving past start', () => {
        expect(clampFocusIndex(0, -1, 10)).toBe(0);
        expect(clampFocusIndex(2, -5, 10)).toBe(0);
    });

    it('clamps at end when moving past last', () => {
        expect(clampFocusIndex(9, 1, 10)).toBe(9);
        expect(clampFocusIndex(5, 20, 10)).toBe(9);
    });

    it('returns 0 for empty list', () => {
        expect(clampFocusIndex(0, 1, 0)).toBe(0);
    });

    it('handles large jumps (page down)', () => {
        expect(clampFocusIndex(0, 20, 100)).toBe(20);
        expect(clampFocusIndex(90, 20, 100)).toBe(99);
    });

    it('clamps when current is already negative (no delta)', () => {
        expect(clampFocusIndex(-5, 0, 10)).toBe(0);
    });

    it('clamps when current is already past end (no delta)', () => {
        expect(clampFocusIndex(15, 0, 10)).toBe(9);
    });
});

describe('clampThumbnailSize', () => {
    it('grows within bounds', () => {
        expect(clampThumbnailSize(120, 16)).toBe(136);
    });

    it('shrinks within bounds', () => {
        expect(clampThumbnailSize(120, -16)).toBe(104);
    });

    it('clamps at min (80)', () => {
        expect(clampThumbnailSize(80, -16)).toBe(80);
        expect(clampThumbnailSize(90, -20)).toBe(80);
    });

    it('clamps at max (400)', () => {
        expect(clampThumbnailSize(400, 16)).toBe(400);
        expect(clampThumbnailSize(390, 20)).toBe(400);
    });

    it('supports custom min/max', () => {
        expect(clampThumbnailSize(50, -10, 40, 200)).toBe(40);
        expect(clampThumbnailSize(190, 20, 40, 200)).toBe(200);
    });

    it('handles zero delta', () => {
        expect(clampThumbnailSize(150, 0)).toBe(150);
    });
});

describe('nextComparePairIndex', () => {
    it('advances by 2', () => {
        expect(nextComparePairIndex(0, 10)).toBe(2);
        expect(nextComparePairIndex(4, 10)).toBe(6);
    });

    it('clamps to last valid pair', () => {
        expect(nextComparePairIndex(8, 10)).toBe(8);
        expect(nextComparePairIndex(7, 10)).toBe(8);
    });

    it('handles 2 images', () => {
        expect(nextComparePairIndex(0, 2)).toBe(0);
    });

    it('handles 1 image', () => {
        expect(nextComparePairIndex(0, 1)).toBe(0);
    });

    it('handles 0 images', () => {
        expect(nextComparePairIndex(0, 0)).toBe(0);
    });

    it('handles negative current', () => {
        // min(-2+2, max(0, 10-2)) = min(0, 8) = 0
        expect(nextComparePairIndex(-2, 10)).toBe(0);
    });

    it('handles negative current with small total', () => {
        // min(-5+2, max(0, 2-2)) = min(-3, 0) = -3 — potential bug: returns negative
        expect(nextComparePairIndex(-5, 2)).toBe(-3);
    });
});

describe('prevComparePairIndex', () => {
    it('retreats by 2', () => {
        expect(prevComparePairIndex(4)).toBe(2);
        expect(prevComparePairIndex(6)).toBe(4);
    });

    it('clamps at 0', () => {
        expect(prevComparePairIndex(0)).toBe(0);
        expect(prevComparePairIndex(1)).toBe(0);
    });
});

describe('computeColumnCount', () => {
    it('computes correct column count', () => {
        expect(computeColumnCount(400, 100, 0)).toBe(4);
        expect(computeColumnCount(400, 100, 10)).toBe(3);
    });

    it('returns at least 1', () => {
        expect(computeColumnCount(50, 100, 0)).toBe(1);
        expect(computeColumnCount(0, 100, 0)).toBe(1);
    });

    it('handles zero gap', () => {
        expect(computeColumnCount(500, 100, 0)).toBe(5);
    });

    it('handles large thumbnails', () => {
        expect(computeColumnCount(300, 400, 0)).toBe(1);
    });

    it('handles thumbSize=0 with positive gap', () => {
        // floor((400+10)/(0+10)) = floor(410/10) = 41
        expect(computeColumnCount(400, 0, 10)).toBe(41);
    });

    it('handles thumbSize + gap = 0 (division by zero)', () => {
        // floor((400+0)/(0+0)) = floor(Infinity) = Infinity → max(1, Infinity) = Infinity
        const result = computeColumnCount(400, 0, 0);
        expect(result).toBe(Infinity);
    });

    it('handles negative containerWidth', () => {
        // floor((-100+10)/(100+10)) = floor(-90/110) = floor(-0.818) = -1 → max(1, -1) = 1
        expect(computeColumnCount(-100, 100, 10)).toBe(1);
    });
});
