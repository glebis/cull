import { describe, it, expect } from 'vitest';
import {
    recordRecentScope,
    markRecentScopeVisited,
    pruneRecentScopes,
    ancestorFolderPaths,
    kindLabel,
    type RecentScope,
} from './sidebar-utils';
import { buildDisplayFolders } from './sidebar-utils';

function scope(kind: RecentScope['kind'], id: string, ts: number, fresh = false): RecentScope {
    return { kind, id, name: id.split('/').pop() || id, ts, fresh };
}

describe('recordRecentScope', () => {
    it('prepends a new scope', () => {
        const next = recordRecentScope([], scope('folder', '/a', 100));
        expect(next).toHaveLength(1);
        expect(next[0].id).toBe('/a');
    });

    it('moves an existing scope to the front instead of duplicating it', () => {
        const list = [scope('folder', '/a', 100), scope('collection', 'c1', 90)];
        const next = recordRecentScope(list, scope('folder', '/a', 200));
        expect(next.map(s => s.id)).toEqual(['/a', 'c1']);
        expect(next[0].ts).toBe(200);
    });

    it('matches on kind + id, not id alone', () => {
        // A folder path and a collection id could theoretically collide;
        // recency must not merge them.
        const list = [scope('collection', 'x', 90)];
        const next = recordRecentScope(list, scope('folder', 'x', 100));
        expect(next).toHaveLength(2);
    });

    it('caps the list, dropping the oldest', () => {
        let list: RecentScope[] = [];
        for (let i = 1; i <= 10; i++) list = recordRecentScope(list, scope('folder', `/f${i}`, i));
        expect(list).toHaveLength(8);
        expect(list[0].id).toBe('/f10');
        expect(list.some(s => s.id === '/f2')).toBe(false);
    });

    it('honours an explicit cap', () => {
        let list: RecentScope[] = [];
        for (let i = 1; i <= 5; i++) list = recordRecentScope(list, scope('folder', `/f${i}`, i), 3);
        expect(list.map(s => s.id)).toEqual(['/f5', '/f4', '/f3']);
    });

    it('a plain revisit clears the fresh flag', () => {
        const list = [scope('folder', '/a', 100, true)];
        const next = recordRecentScope(list, scope('folder', '/a', 200, false));
        expect(next[0].fresh).toBe(false);
    });

    it('does not mutate the input list', () => {
        const list = [scope('folder', '/a', 100)];
        recordRecentScope(list, scope('folder', '/b', 200));
        expect(list).toHaveLength(1);
    });
});

describe('markRecentScopeVisited', () => {
    it('clears fresh on the matching scope only', () => {
        const list = [
            scope('folder', '/a', 100, true),
            scope('folder', '/b', 90, true),
        ];
        const next = markRecentScopeVisited(list, 'folder', '/a');
        expect(next[0].fresh).toBe(false);
        expect(next[1].fresh).toBe(true);
    });

    it('leaves order and timestamps untouched', () => {
        const list = [scope('folder', '/a', 100, true), scope('collection', 'c1', 90, true)];
        const next = markRecentScopeVisited(list, 'collection', 'c1');
        expect(next.map(s => s.id)).toEqual(['/a', 'c1']);
        expect(next[1].ts).toBe(90);
    });

    it('is a no-op for unknown scopes', () => {
        const list = [scope('folder', '/a', 100, true)];
        expect(markRecentScopeVisited(list, 'folder', '/zzz')).toEqual(list);
    });
});

describe('pruneRecentScopes', () => {
    it('drops scopes whose target no longer exists', () => {
        const list = [
            scope('folder', '/gone', 100),
            scope('folder', '/here', 99),
            scope('collection', 'c-dead', 98),
            scope('collection', 'c-live', 97),
            scope('smart', 's-dead', 96),
            scope('smart', 's-live', 95),
        ];
        const next = pruneRecentScopes(
            list,
            new Set(['/here']),
            new Set(['c-live']),
            new Set(['s-live']),
        );
        expect(next.map(s => s.id)).toEqual(['/here', 'c-live', 's-live']);
    });

    it('keeps everything when all targets are live', () => {
        const list = [scope('folder', '/a', 100)];
        const next = pruneRecentScopes(list, new Set(['/a']), new Set(), new Set());
        expect(next).toHaveLength(1);
    });
});

describe('ancestorFolderPaths', () => {
    it('returns expanded-row paths that must be open to reveal a target', () => {
        const rows = buildDisplayFolders([
            ['/lib/ai/midjourney/2026-08/run1', 5],
            ['/lib/ai/midjourney/2026-08/run2', 3],
            ['/lib/photos', 10],
        ]);
        // Single-child empty chains compress into one row: the only real
        // ancestor row of run1 is "ai/midjourney/2026-08" holding the deep path.
        const target = rows.find(r => r.fullPath.endsWith('run1'))!;
        const ancestors = ancestorFolderPaths(rows, target.fullPath);
        expect(ancestors).toEqual(['/lib/ai/midjourney/2026-08']);
        expect(ancestors).not.toContain('/lib/photos');
        // The target itself is never its own ancestor
        expect(ancestors).not.toContain(target.fullPath);
    });

    it('walks every level when compression is broken by own-counts', () => {
        const rows = buildDisplayFolders([
            ['/lib/ai', 1],
            ['/lib/ai/midjourney', 2],
            ['/lib/ai/midjourney/run1', 3],
        ]);
        // No compression: /lib/ai and /lib/ai/midjourney both hold images, so
        // both are real rows and both must be expanded to see run1.
        expect(ancestorFolderPaths(rows, '/lib/ai/midjourney/run1'))
            .toEqual(['/lib/ai', '/lib/ai/midjourney']);
    });

    it('returns [] for a top-level target', () => {
        const rows = buildDisplayFolders([['/photos', 10]]);
        expect(ancestorFolderPaths(rows, '/photos')).toEqual([]);
    });

    it('returns [] for an unknown target', () => {
        const rows = buildDisplayFolders([['/photos', 10]]);
        expect(ancestorFolderPaths(rows, '/nope')).toEqual([]);
    });
});

describe('kindLabel', () => {
    it('names each scope kind in user vocabulary', () => {
        expect(kindLabel('folder')).toBe('Folder');
        expect(kindLabel('collection')).toBe('Collection');
        expect(kindLabel('smart')).toBe('Smart');
    });
});
