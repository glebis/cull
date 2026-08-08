import { describe, it, expect } from 'vitest';
import { createPrefetchCache, type PrefetchImage } from './prefetch-cache';

function makeFakeFactory() {
    const created: PrefetchImage[] = [];
    const factory = (): PrefetchImage => {
        const img: PrefetchImage = {
            src: '',
            decoding: undefined,
            onload: null,
            onerror: null,
        };
        created.push(img);
        return img;
    };
    return { created, factory };
}

describe('createPrefetchCache', () => {
    it('warms a url by constructing an image and setting its src', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache(4, factory);

        cache.warm('asset://a.jpg');

        expect(cache.size()).toBe(1);
        expect(cache.has('asset://a.jpg')).toBe(true);
        expect(created).toHaveLength(1);
        expect(created[0].src).toBe('asset://a.jpg');
        expect(created[0].decoding).toBe('async');
    });

    it('is idempotent: warming the same url twice does not create a second image', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache(4, factory);

        cache.warm('asset://a.jpg');
        cache.warm('asset://a.jpg');

        expect(cache.size()).toBe(1);
        expect(created).toHaveLength(1);
    });

    it('evicts the oldest entry and tears it down when exceeding maxEntries', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache(2, factory);

        cache.warm('a');
        cache.warm('b');
        cache.warm('c'); // should evict 'a'

        expect(cache.size()).toBe(2);
        expect(cache.has('a')).toBe(false);
        expect(cache.has('b')).toBe(true);
        expect(cache.has('c')).toBe(true);
        // The evicted image (first created) was torn down by clearing its src.
        expect(created[0].src).toBe('');
    });

    it('re-warming refreshes recency so a different entry is evicted', () => {
        const { factory } = makeFakeFactory();
        const cache = createPrefetchCache(2, factory);

        cache.warm('a');
        cache.warm('b');
        cache.warm('a'); // 'a' becomes most-recent again
        cache.warm('c'); // should evict 'b', not 'a'

        expect(cache.has('a')).toBe(true);
        expect(cache.has('b')).toBe(false);
        expect(cache.has('c')).toBe(true);
    });

    it('clear() empties the cache and tears down every image', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache(4, factory);

        cache.warm('a');
        cache.warm('b');
        cache.clear();

        expect(cache.size()).toBe(0);
        expect(cache.has('a')).toBe(false);
        expect(created.every((img) => img.src === '')).toBe(true);
    });

    it('ignores empty urls', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache(4, factory);

        cache.warm('');

        expect(cache.size()).toBe(0);
        expect(created).toHaveLength(0);
    });
});

describe('memory-aware prefetch scheduling', () => {
    it('bounds active decodes and starts queued work as images finish', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache({ maxEntries: 10, maxDecodedBytes: 1_000, concurrency: 2 }, factory);

        cache.schedule([
            { url: 'a', estimatedBytes: 100 },
            { url: 'b', estimatedBytes: 100 },
            { url: 'c', estimatedBytes: 100 },
        ], 'down');

        expect(created.map(image => image.src)).toEqual(['a', 'b']);
        expect(cache.stats()).toEqual(expect.objectContaining({ active: 2, queued: 1, estimatedBytes: 200 }));

        created[0].onload?.(new Event('load'));

        expect(created.map(image => image.src)).toEqual(['a', 'b', 'c']);
        expect(cache.stats()).toEqual(expect.objectContaining({ active: 2, queued: 0, estimatedBytes: 300 }));
    });

    it('cancels active and queued work when scroll direction reverses', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache({ maxEntries: 10, maxDecodedBytes: 1_000, concurrency: 2 }, factory);

        cache.schedule([
            { url: 'down-1', estimatedBytes: 100 },
            { url: 'down-2', estimatedBytes: 100 },
            { url: 'down-3', estimatedBytes: 100 },
        ], 'down');
        const obsolete = [...created];

        cache.schedule([{ url: 'up-1', estimatedBytes: 100 }], 'up');

        expect(obsolete.every(image => image.src === '')).toBe(true);
        expect(created.at(-1)?.src).toBe('up-1');
        expect(cache.has('down-1')).toBe(false);
        expect(cache.has('up-1')).toBe(true);
        expect(cache.stats()).toEqual(expect.objectContaining({ active: 1, queued: 0, estimatedBytes: 100 }));
    });

    it('deduplicates URLs across the candidate list and active cache', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache({ maxEntries: 10, maxDecodedBytes: 1_000, concurrency: 2 }, factory);

        cache.schedule([
            { url: 'same', estimatedBytes: 100 },
            { url: 'same', estimatedBytes: 100 },
        ], 'down');
        cache.schedule([{ url: 'same', estimatedBytes: 100 }], 'down');

        expect(created).toHaveLength(1);
        expect(cache.stats()).toEqual(expect.objectContaining({ active: 1, queued: 0 }));
    });

    it('does not evict a nearer candidate to churn through a window larger than the budget', () => {
        const { created, factory } = makeFakeFactory();
        const cache = createPrefetchCache({ maxEntries: 2, maxDecodedBytes: 100, concurrency: 2 }, factory);

        cache.schedule([
            { url: 'a', estimatedBytes: 60 },
            { url: 'b', estimatedBytes: 60 },
            { url: 'c', estimatedBytes: 60 },
        ], 'down');

        expect(created.map(image => image.src)).toEqual(['a']);
        expect(cache.stats()).toEqual(expect.objectContaining({ entries: 1, active: 1, queued: 2, estimatedBytes: 60 }));

        created[0].onload?.(new Event('load'));
        expect(created).toHaveLength(1);
        expect(created[0].src).toBe('a');
        expect(cache.stats()).toEqual(expect.objectContaining({ entries: 1, active: 0, queued: 2, estimatedBytes: 60 }));
    });

    it('estimates decoded RGBA memory from source aspect ratio and decoded edge', async () => {
        const { estimateDecodedImageBytes } = await import('./prefetch-cache');

        expect(estimateDecodedImageBytes(400, 200, 128)).toBe(128 * 64 * 4);
        expect(estimateDecodedImageBytes(200, 400, 128)).toBe(64 * 128 * 4);
        expect(estimateDecodedImageBytes(Number.NaN, 0, 128)).toBe(128 * 128 * 4);
    });
});
