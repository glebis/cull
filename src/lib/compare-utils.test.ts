import { describe, it, expect } from 'vitest';
import { resolveComparePair, ratingStars, decisionLabel, computeCompareSwap } from './compare-utils';
import type { ImageWithFile } from './api';

function makeImage(id: string, overrides?: { star_rating?: number | null; decision?: string }): ImageWithFile {
    return {
        image: { id, sha256_hash: '', width: 100, height: 100, format: 'jpeg', file_size: 1000, created_at: '', imported_at: '', ai_prompt: null },
        source_label: null,
        path: `/photos/${id}.jpg`,
        thumbnail_path: null,
        selection: overrides ? {
            image_id: id,
            project_id: null,
            star_rating: overrides.star_rating ?? null,
            color_label: null,
            decision: overrides.decision ?? 'undecided',
        } : null,
    };
}

describe('resolveComparePair', () => {
    const imgs = [makeImage('a'), makeImage('b'), makeImage('c')];

    it('returns two selected images when two are selected', () => {
        const [left, right] = resolveComparePair(imgs, new Set(['a', 'c']), 0);
        expect(left?.image.id).toBe('a');
        expect(right?.image.id).toBe('c');
    });

    it('falls back to focused + next when fewer than two selected', () => {
        const [left, right] = resolveComparePair(imgs, new Set(['a']), 1);
        expect(left?.image.id).toBe('b');
        expect(right?.image.id).toBe('c');
    });

    it('falls back to focused + next when none selected', () => {
        const [left, right] = resolveComparePair(imgs, new Set(), 0);
        expect(left?.image.id).toBe('a');
        expect(right?.image.id).toBe('b');
    });

    it('returns [last, null] when focused is at end of list', () => {
        const [left, right] = resolveComparePair(imgs, new Set(), 2);
        expect(left?.image.id).toBe('c');
        expect(right).toBeNull();
    });

    it('returns [null, null] when focused is out of bounds', () => {
        const [left, right] = resolveComparePair(imgs, new Set(), 99);
        expect(left).toBeNull();
        expect(right).toBeNull();
    });

    it('returns [null, null] for empty images array', () => {
        const [left, right] = resolveComparePair([], new Set(), 0);
        expect(left).toBeNull();
        expect(right).toBeNull();
    });

    it('falls back to focused pair when selected IDs are stale (not in images)', () => {
        const [left, right] = resolveComparePair(imgs, new Set(['deleted1', 'deleted2']), 0);
        expect(left?.image.id).toBe('a');
        expect(right?.image.id).toBe('b');
    });

    it('falls back when only one selected ID is stale', () => {
        const [left, right] = resolveComparePair(imgs, new Set(['a', 'deleted']), 1);
        expect(left?.image.id).toBe('b');
        expect(right?.image.id).toBe('c');
    });

    it('uses Set insertion order for selected pair', () => {
        const [left, right] = resolveComparePair(imgs, new Set(['c', 'a']), 0);
        expect(left?.image.id).toBe('c');
        expect(right?.image.id).toBe('a');
    });

    it('returns [null, null] for negative focusedIndex with no selection', () => {
        const [left, right] = resolveComparePair(imgs, new Set(), -1);
        expect(left).toBeNull();
        expect(right).toBeNull();
    });
});

describe('ratingStars', () => {
    it('returns star rating when present', () => {
        expect(ratingStars(makeImage('a', { star_rating: 4 }))).toBe(4);
    });

    it('returns 0 when star_rating is null', () => {
        expect(ratingStars(makeImage('a', { star_rating: null }))).toBe(0);
    });

    it('returns 0 when selection is null', () => {
        expect(ratingStars(makeImage('a'))).toBe(0);
    });

    it('returns 0 for null image', () => {
        expect(ratingStars(null)).toBe(0);
    });
});

describe('decisionLabel', () => {
    it('returns accept', () => {
        expect(decisionLabel(makeImage('a', { decision: 'accept' }))).toBe('accept');
    });

    it('returns reject', () => {
        expect(decisionLabel(makeImage('a', { decision: 'reject' }))).toBe('reject');
    });

    it('returns undecided for default selection', () => {
        expect(decisionLabel(makeImage('a', {}))).toBe('undecided');
    });

    it('returns undecided when selection is null', () => {
        expect(decisionLabel(makeImage('a'))).toBe('undecided');
    });

    it('returns undecided for null image', () => {
        expect(decisionLabel(null)).toBe('undecided');
    });
});

describe('computeCompareSwap', () => {
    const ids = ['a', 'b', 'c', 'd', 'e'];

    it('moves active side down when two are selected', () => {
        const result = computeCompareSwap(ids, new Set(['a', 'c']), 0, 1, 1);
        expect(result).not.toBeNull();
        expect(result!.newSelectedIds).toEqual(new Set(['a', 'd']));
    });

    it('moves active side up when two are selected', () => {
        const result = computeCompareSwap(ids, new Set(['b', 'd']), 0, 0, -1);
        expect(result).not.toBeNull();
        expect(result!.newSelectedIds).toEqual(new Set(['a', 'd']));
    });

    it('prevents swapping to the same image as the other side', () => {
        const result = computeCompareSwap(ids, new Set(['a', 'b']), 0, 0, 1);
        expect(result).toBeNull();
    });

    it('clamps at start of list (no change)', () => {
        const result = computeCompareSwap(ids, new Set(['a', 'c']), 0, 0, -1);
        expect(result).not.toBeNull();
        expect(result!.newSelectedIds).toEqual(new Set(['a', 'c']));
    });

    it('clamps at end of list (no change)', () => {
        const result = computeCompareSwap(ids, new Set(['a', 'e']), 0, 1, 1);
        expect(result).not.toBeNull();
        expect(result!.newSelectedIds).toEqual(new Set(['a', 'e']));
    });

    it('promotes to explicit selection from implicit pair', () => {
        const result = computeCompareSwap(ids, new Set(), 1, 0, -1);
        expect(result).not.toBeNull();
        expect(result!.newSelectedIds).toEqual(new Set(['a', 'c']));
    });

    it('moves right side independently from implicit pair', () => {
        const result = computeCompareSwap(ids, new Set(), 0, 1, 1);
        expect(result).not.toBeNull();
        expect(result!.newSelectedIds).toEqual(new Set(['a', 'c']));
    });

    it('returns null for fewer than 2 images', () => {
        expect(computeCompareSwap(['a'], new Set(), 0, 0, 1)).toBeNull();
        expect(computeCompareSwap([], new Set(), 0, 0, 1)).toBeNull();
    });
});
