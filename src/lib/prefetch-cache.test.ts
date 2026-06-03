import { describe, it, expect } from 'vitest';
import { createPrefetchCache, type PrefetchImage } from './prefetch-cache';

function makeFakeFactory() {
    const created: Array<{ src: string; decoding?: string }> = [];
    const factory = (): PrefetchImage => {
        const img = { src: '', decoding: undefined as string | undefined };
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
