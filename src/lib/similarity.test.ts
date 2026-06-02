import { describe, expect, it } from 'vitest';
import { orderBySimilarity } from './similarity';
import type { ImageWithFile } from './api';

function img(id: string): ImageWithFile {
    return { image: { id } } as unknown as ImageWithFile;
}

describe('similarity ordering', () => {
    it('reorders fetched images to match the ranked similarity order', () => {
        const ranked = ['c', 'a', 'b'];
        const fetched = [img('a'), img('b'), img('c')];
        expect(orderBySimilarity(ranked, fetched).map(i => i.image.id)).toEqual(['c', 'a', 'b']);
    });

    it('sorts images missing from the ranking to the end', () => {
        const ranked = ['b'];
        const fetched = [img('a'), img('b')];
        expect(orderBySimilarity(ranked, fetched).map(i => i.image.id)).toEqual(['b', 'a']);
    });

    it('returns an empty array for no input', () => {
        expect(orderBySimilarity([], [])).toEqual([]);
    });
});
