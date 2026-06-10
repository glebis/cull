import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { createStaleGuard } from '$lib/stale-guard';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

interface Deferred<T> {
    promise: Promise<T>;
    resolve: (value: T) => void;
}

function deferred<T>(): Deferred<T> {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>(r => { resolve = r; });
    return { promise, resolve };
}

describe('prompt resubmit cost estimate stale guard', () => {
    it('discards a late-arriving response with an older seq', async () => {
        const guard = createStaleGuard();
        let costEstimate: string | null = null;

        const first = deferred<string>();
        const second = deferred<string>();

        // Params change twice -> two in-flight estimate requests.
        const seq1 = guard.next();
        const req1 = first.promise.then(c => {
            if (guard.isCurrent(seq1)) costEstimate = c;
        });

        const seq2 = guard.next();
        const req2 = second.promise.then(c => {
            if (guard.isCurrent(seq2)) costEstimate = c;
        });

        // Resolve the FIRST (stale) request LAST.
        second.resolve('estimate-for-latest-params');
        await req2;
        first.resolve('estimate-for-stale-params');
        await req1;

        expect(costEstimate).toBe('estimate-for-latest-params');
    });

    it('lets the newest request win regardless of resolution order', async () => {
        const guard = createStaleGuard();
        let result: number | null = null;

        const responses = [deferred<number>(), deferred<number>(), deferred<number>()];
        const settled = responses.map((d, i) => {
            const seq = guard.next();
            return d.promise.then(v => {
                if (guard.isCurrent(seq)) result = v;
            });
        });

        // Resolve out of order: newest first, then the stale ones.
        responses[2].resolve(2);
        responses[0].resolve(0);
        responses[1].resolve(1);
        await Promise.all(settled);

        expect(result).toBe(2);
    });

    it('only the latest seq is current', () => {
        const guard = createStaleGuard();
        const a = guard.next();
        const b = guard.next();

        expect(guard.isCurrent(a)).toBe(false);
        expect(guard.isCurrent(b)).toBe(true);
    });

    it('PromptResubmitDialog guards the cost estimate effect with the seq token', () => {
        const dialog = source('src/lib/components/PromptResubmitDialog.svelte');

        expect(dialog).toContain('stale-guard');
        expect(dialog).toContain('createStaleGuard');
        // The raw unguarded assignment must be gone.
        expect(dialog).not.toContain('.then(c => costEstimate = c)');
        expect(dialog).toContain('isCurrent');
    });
});
