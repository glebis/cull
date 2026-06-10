/**
 * Monotonic sequence guard for async request/response races.
 *
 * Same shape as the inline `histogramRequestSeq` token in Loupe.svelte and the
 * projection seq in EmbeddingExplorer.svelte: each new request takes `next()`,
 * and a response is only applied when `isCurrent(seq)` — so late-arriving
 * responses from superseded requests are discarded and the newest request wins
 * regardless of resolution order.
 */
export interface StaleGuard {
    /** Claim a new sequence token, invalidating all previous ones. */
    next(): number;
    /** True only for the most recently claimed token. */
    isCurrent(seq: number): boolean;
}

export function createStaleGuard(): StaleGuard {
    let latest = 0;
    return {
        next: () => ++latest,
        isCurrent: (seq: number) => seq === latest,
    };
}
