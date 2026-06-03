/**
 * Bounded, LRU decode-warming cache for grid thumbnails.
 *
 * The grid renders `asset://` URLs, so the webview (not JS) owns the decoded bitmaps. We
 * "warm" upcoming thumbnails by constructing detached Image objects and assigning their
 * `src`, which primes the webview decode cache so the image paints instantly once the row
 * mounts. The LRU bounds how many we keep warm; evicted entries drop their `src` so the
 * webview can reclaim their memory — this is the "unload behind" half of the design.
 */

export interface PrefetchImage {
    src: string;
    decoding?: string;
}

export interface PrefetchCache {
    /** Warm a URL (no-op if empty or already warm; refreshes recency if warm). */
    warm(url: string): void;
    has(url: string): boolean;
    size(): number;
    /** Tear down and forget every warmed image. Call on scope change / teardown. */
    clear(): void;
}

export function createPrefetchCache(
    maxEntries: number,
    makeImage: () => PrefetchImage = () => new Image()
): PrefetchCache {
    const max = Math.max(1, Math.trunc(maxEntries) || 1);
    // Map preserves insertion order, so the first key is always the least-recently warmed.
    const entries = new Map<string, PrefetchImage>();

    function teardown(img: PrefetchImage): void {
        // Dropping the src lets the webview release the decoded bitmap.
        img.src = '';
    }

    return {
        warm(url: string): void {
            if (!url) return;
            const existing = entries.get(url);
            if (existing) {
                // Refresh recency: re-insert to move to the most-recent position.
                entries.delete(url);
                entries.set(url, existing);
                return;
            }
            const img = makeImage();
            img.decoding = 'async';
            img.src = url;
            entries.set(url, img);
            while (entries.size > max) {
                const oldestKey = entries.keys().next().value as string | undefined;
                if (oldestKey === undefined) break;
                const oldest = entries.get(oldestKey);
                entries.delete(oldestKey);
                if (oldest) teardown(oldest);
            }
        },
        has(url: string): boolean {
            return entries.has(url);
        },
        size(): number {
            return entries.size;
        },
        clear(): void {
            for (const img of entries.values()) teardown(img);
            entries.clear();
        },
    };
}
