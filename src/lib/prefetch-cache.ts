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
    onload?: ((event: Event) => void) | null;
    onerror?: ((event: Event | string) => void) | null;
}

export interface PrefetchCandidate {
    url: string;
    estimatedBytes: number;
}

export type PrefetchDirection = 'up' | 'down' | 'none';

export interface PrefetchCacheOptions {
    maxEntries: number;
    maxDecodedBytes: number;
    concurrency: number;
}

export interface PrefetchCacheStats {
    entries: number;
    active: number;
    queued: number;
    estimatedBytes: number;
}

export interface PrefetchCache {
    /** Warm a URL (no-op if empty or already warm; refreshes recency if warm). */
    warm(url: string): void;
    /** Replace pending look-ahead work with the newest directional candidate window. */
    schedule(candidates: readonly PrefetchCandidate[], direction: PrefetchDirection): void;
    has(url: string): boolean;
    size(): number;
    stats(): PrefetchCacheStats;
    /** Tear down and forget every warmed image. Call on scope change / teardown. */
    clear(): void;
}

export function createPrefetchCache(
    options: number | PrefetchCacheOptions,
    makeImage: () => PrefetchImage = () => new Image()
): PrefetchCache {
    const normalized = typeof options === 'number'
        ? { maxEntries: options, maxDecodedBytes: Number.POSITIVE_INFINITY, concurrency: options }
        : options;
    const maxEntries = Math.max(1, Math.trunc(normalized.maxEntries) || 1);
    const maxDecodedBytes = Number.isFinite(normalized.maxDecodedBytes)
        ? Math.max(1, Math.trunc(normalized.maxDecodedBytes) || 1)
        : Number.POSITIVE_INFINITY;
    const concurrency = Math.max(1, Math.trunc(normalized.concurrency) || 1);
    interface Entry {
        image: PrefetchImage;
        estimatedBytes: number;
        active: boolean;
        cancelled: boolean;
    }
    const entries = new Map<string, Entry>();
    let queue: PrefetchCandidate[] = [];
    let direction: PrefetchDirection = 'none';
    let protectedUrls = new Set<string>();
    let estimatedBytes = 0;
    let activeCount = 0;

    function teardown(img: PrefetchImage): void {
        img.onload = null;
        img.onerror = null;
        img.src = '';
    }

    function removeEntry(url: string): void {
        const entry = entries.get(url);
        if (!entry) return;
        entry.cancelled = true;
        if (entry.active) activeCount -= 1;
        estimatedBytes -= entry.estimatedBytes;
        entries.delete(url);
        teardown(entry.image);
    }

    function oldestReadyUrl(): string | null {
        for (const [url, entry] of entries) {
            if (!entry.active && !protectedUrls.has(url)) return url;
        }
        return null;
    }

    function reserve(candidate: PrefetchCandidate): boolean {
        if (candidate.estimatedBytes > maxDecodedBytes) return false;
        while (entries.size + 1 > maxEntries || estimatedBytes + candidate.estimatedBytes > maxDecodedBytes) {
            const oldest = oldestReadyUrl();
            if (!oldest) return false;
            removeEntry(oldest);
        }
        return true;
    }

    function pump(): void {
        while (activeCount < concurrency && queue.length > 0) {
            const candidate = queue[0];
            if (entries.has(candidate.url)) {
                queue.shift();
                continue;
            }
            if (candidate.estimatedBytes > maxDecodedBytes) {
                queue.shift();
                continue;
            }
            if (!reserve(candidate)) return;
            queue.shift();

            const image = makeImage();
            const entry: Entry = {
                image,
                estimatedBytes: candidate.estimatedBytes,
                active: true,
                cancelled: false,
            };
            entries.set(candidate.url, entry);
            estimatedBytes += candidate.estimatedBytes;
            activeCount += 1;
            image.decoding = 'async';
            image.onload = () => {
                if (entry.cancelled || entries.get(candidate.url) !== entry) return;
                entry.active = false;
                activeCount -= 1;
                entries.delete(candidate.url);
                entries.set(candidate.url, entry);
                pump();
            };
            image.onerror = () => {
                if (entry.cancelled || entries.get(candidate.url) !== entry) return;
                removeEntry(candidate.url);
                pump();
            };
            image.src = candidate.url;
        }
    }

    return {
        warm(url: string): void {
            if (!url) return;
            protectedUrls.clear();
            queue = [];
            const existing = entries.get(url);
            if (existing) {
                entries.delete(url);
                entries.set(url, existing);
                return;
            }
            queue = queue.filter(candidate => candidate.url !== url);
            while (entries.size >= maxEntries) {
                const oldest = oldestReadyUrl();
                if (!oldest) return;
                removeEntry(oldest);
            }
            const img = makeImage();
            img.decoding = 'async';
            img.src = url;
            entries.set(url, { image: img, estimatedBytes: 0, active: false, cancelled: false });
        },
        schedule(candidates: readonly PrefetchCandidate[], nextDirection: PrefetchDirection): void {
            const seen = new Set<string>();
            const normalizedCandidates = candidates
                .map(candidate => ({
                    url: candidate.url,
                    estimatedBytes: Math.max(0, Math.trunc(candidate.estimatedBytes) || 0),
                }))
                .filter(candidate => candidate.url !== '' && !seen.has(candidate.url) && seen.add(candidate.url));
            protectedUrls = new Set(normalizedCandidates.map(candidate => candidate.url));
            if (nextDirection !== direction) {
                direction = nextDirection;
                queue = [];
                for (const [url, entry] of [...entries]) {
                    if (entry.active) removeEntry(url);
                }
            } else {
                for (const [url, entry] of [...entries]) {
                    if (entry.active && !protectedUrls.has(url)) removeEntry(url);
                }
            }
            queue = normalizedCandidates.filter(candidate => !entries.has(candidate.url));
            pump();
        },
        has(url: string): boolean {
            return entries.has(url);
        },
        size(): number {
            return entries.size;
        },
        stats(): PrefetchCacheStats {
            return { entries: entries.size, active: activeCount, queued: queue.length, estimatedBytes };
        },
        clear(): void {
            queue = [];
            direction = 'none';
            protectedUrls.clear();
            for (const url of [...entries.keys()]) removeEntry(url);
        },
    };
}

export function estimateDecodedImageBytes(width: number, height: number, decodedEdge: number): number {
    const edge = Number.isFinite(decodedEdge) && decodedEdge > 0 ? Math.ceil(decodedEdge) : 1;
    if (!Number.isFinite(width) || width <= 0 || !Number.isFinite(height) || height <= 0) {
        return edge * edge * 4;
    }
    const sourceWidth = Math.ceil(width);
    const sourceHeight = Math.ceil(height);
    if (sourceWidth >= sourceHeight) {
        return edge * Math.max(1, Math.ceil(edge * sourceHeight / sourceWidth)) * 4;
    }
    return Math.max(1, Math.ceil(edge * sourceWidth / sourceHeight)) * edge * 4;
}
