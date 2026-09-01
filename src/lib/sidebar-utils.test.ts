import { describe, it, expect } from 'vitest';
import { folderName, buildDisplayFolders, buildPinnedCollectionRows, formatImportResult, formatSidebarCount, formatFolderCount, visibleFolderRows, prunePinnedIds } from './sidebar-utils';

describe('folderName', () => {
    it('returns last segment of a path', () => {
        expect(folderName('/Users/test/Photos')).toBe('Photos');
    });

    it('handles single segment', () => {
        expect(folderName('Photos')).toBe('Photos');
    });

    it('returns full path for root slash', () => {
        expect(folderName('/')).toBe('/');
    });

    it('falls back to full path for trailing slash', () => {
        expect(folderName('/Users/test/')).toBe('/Users/test/');
    });

    it('handles deeply nested path', () => {
        expect(folderName('/a/b/c/d/e')).toBe('e');
    });

    it('returns empty string for empty input', () => {
        expect(folderName('')).toBe('');
    });

    it('handles repeated slashes', () => {
        expect(folderName('//a///b//')).toBe('//a///b//');
    });

    it('handles path with only slashes', () => {
        expect(folderName('///')).toBe('///');
    });
});

describe('buildDisplayFolders', () => {
    it('returns empty array for empty input', () => {
        expect(buildDisplayFolders([])).toEqual([]);
    });

    it('builds a single folder with depth 0', () => {
        const result = buildDisplayFolders([['/Users/test/Photos', 10]]);
        expect(result).toHaveLength(1);
        expect(result[0].name).toBe('Photos');
        expect(result[0].fullPath).toBe('/Users/test/Photos');
        expect(result[0].count).toBe(10);
        expect(result[0].depth).toBe(0);
    });

    it('strips common prefix and builds tree', () => {
        const result = buildDisplayFolders([
            ['/Users/test/project/src/assets', 5],
            ['/Users/test/project/src/images', 3],
            ['/Users/test/project/docs/photos', 2],
        ]);
        // Common prefix: /Users/test/project
        // Tree: docs/photos (collapsed), src (group) -> assets, images
        expect(result.map(f => f.name)).toEqual(['docs/photos', 'src', 'assets', 'images']);
        expect(result[0].depth).toBe(0);
        expect(result[0].count).toBe(2);
        const src = result.find(f => f.name === 'src');
        expect(src?.depth).toBe(0);
        expect(src?.hasChildren).toBe(true);
        expect(src?.count).toBe(0);
        const assets = result.find(f => f.name === 'assets');
        expect(assets?.depth).toBe(1);
        expect(assets?.count).toBe(5);
    });

    it('sorts siblings alphabetically', () => {
        const result = buildDisplayFolders([
            ['/root/Zebras', 1],
            ['/root/Apples', 2],
            ['/root/Mangos', 3],
        ]);
        expect(result.map(f => f.name)).toEqual(['Apples', 'Mangos', 'Zebras']);
    });

    it('nests children under parents with correct depth', () => {
        const result = buildDisplayFolders([
            ['/root/a/child1', 1],
            ['/root/a/child2', 2],
            ['/root/b', 3],
        ]);
        // a is a group at depth 0, child1/child2 at depth 1, b at depth 0
        const a = result.find(f => f.name === 'a');
        expect(a?.depth).toBe(0);
        expect(a?.hasChildren).toBe(true);
        const child1 = result.find(f => f.name === 'child1');
        expect(child1?.depth).toBe(1);
        const b = result.find(f => f.name === 'b');
        expect(b?.depth).toBe(0);
    });

    it('collapses single-child chains', () => {
        const result = buildDisplayFolders([
            ['/root/a/b/c/deep', 5],
        ]);
        expect(result).toHaveLength(1);
        expect(result[0].name).toBe('deep');
        expect(result[0].depth).toBe(0);
    });

    it('collapses intermediate single-child nodes into combined names', () => {
        const result = buildDisplayFolders([
            ['/root/node_modules/zod/lib/assets', 3],
            ['/root/src/images', 5],
        ]);
        // node_modules/zod/lib/assets collapses into one entry
        // src/images collapses into one entry
        const nm = result.find(f => f.name.includes('node_modules'));
        expect(nm?.name).toBe('node_modules/zod/lib/assets');
        const src = result.find(f => f.name.includes('src'));
        expect(src?.name).toBe('src/images');
    });

    it('handles single folder (no common prefix stripping)', () => {
        const result = buildDisplayFolders([['/Photos', 5]]);
        expect(result).toHaveLength(1);
        expect(result[0].name).toBe('Photos');
        expect(result[0].depth).toBe(0);
    });
});

describe('formatImportResult', () => {
    it('formats with no errors', () => {
        expect(formatImportResult(10, 5, 0)).toBe('+10 imported, 5 skipped');
    });

    it('formats with errors', () => {
        expect(formatImportResult(10, 5, 3)).toBe('+10 imported, 5 skipped, 3 errors');
    });

    it('handles zero counts', () => {
        expect(formatImportResult(0, 0, 0)).toBe('+0 imported, 0 skipped');
    });
});

describe('formatSidebarCount', () => {
    it('formats counts as plain numbers', () => {
        expect(formatSidebarCount(42)).toBe('42');
    });

    it('omits zero and missing counts (imageview-1i2k.3)', () => {
        expect(formatSidebarCount(0)).toBe('');
        expect(formatSidebarCount(null)).toBe('');
        expect(formatSidebarCount(undefined)).toBe('');
    });
});

describe('buildPinnedCollectionRows', () => {
    it('moves pinned collections to the top in pin order', () => {
        const rows: [string, string, number][] = [
            ['a', 'A', 1],
            ['b', 'B', 2],
            ['c', 'C', 3],
            ['d', 'D', 4],
        ];

        expect(buildPinnedCollectionRows(rows, ['c', 'b']).map(([id]) => id)).toEqual(['c', 'b', 'a', 'd']);
    });

    it('ignores stale pinned collection ids', () => {
        const rows: [string, string, number][] = [
            ['a', 'A', 1],
            ['b', 'B', 2],
        ];

        expect(buildPinnedCollectionRows(rows, ['missing', 'b']).map(([id]) => id)).toEqual(['b', 'a']);
    });
});

describe('formatFolderCount', () => {
    it('shows a single number when the folder has no subfolders', () => {
        expect(formatFolderCount(4, 4)).toBe('4');
    });

    it('shows direct and subtree when they differ', () => {
        expect(formatFolderCount(4, 27)).toBe('4 (27)');
    });

    it('shows only the subtree number for a group folder that only carries descendants', () => {
        expect(formatFolderCount(0, 27)).toBe('27');
    });

    it('omits the count for a completely empty subtree (imageview-1i2k.3)', () => {
        expect(formatFolderCount(0, 0)).toBe('');
    });
});

describe('buildDisplayFolders subtree counts', () => {
    it('sums descendants into subtreeCount while keeping the direct count', () => {
        const rows = buildDisplayFolders([
            // The sibling keeps /lib/photos from being swallowed by the
            // common-prefix stripping.
            ['/lib/other', 1],
            ['/lib/photos', 4],
            ['/lib/photos/sub', 20],
            ['/lib/photos/sub/deep', 3],
        ]);
        const photos = rows.find(r => r.fullPath === '/lib/photos')!;
        expect(photos.count).toBe(4);
        expect(photos.subtreeCount).toBe(27);
    });

    it('gives group folders a real navigable path instead of an empty string', () => {
        // /lib/art has no images of its own, so the backend never reports it.
        const rows = buildDisplayFolders([
            ['/lib/other', 1],
            ['/lib/art/a', 2],
            ['/lib/art/b', 3],
        ]);
        const group = rows.find(r => r.name === 'art')!;
        expect(group.fullPath).toBe('/lib/art');
        expect(group.isGroup).toBe(true);
        expect(group.count).toBe(0);
        expect(group.subtreeCount).toBe(5);
    });

    it('attributes a compressed single-child chain to its terminal folder', () => {
        const rows = buildDisplayFolders([
            ['/lib/a/b/c', 7],
        ]);
        expect(rows).toHaveLength(1);
        expect(rows[0].fullPath).toBe('/lib/a/b/c');
        expect(rows[0].count).toBe(7);
        expect(rows[0].subtreeCount).toBe(7);
    });
});

describe('visibleFolderRows', () => {
    const tree = () => buildDisplayFolders([
        ['/lib/art', 1],
        ['/lib/art/raw', 2],
        ['/lib/art/raw/nested', 3],
        ['/lib/photos', 4],
    ]);

    it('hides descendants of a collapsed node', () => {
        const rows = visibleFolderRows(tree(), new Set());
        expect(rows.map(r => r.name)).toEqual(['art', 'photos']);
    });

    it('reveals one level per expanded node', () => {
        const rows = visibleFolderRows(tree(), new Set(['/lib/art']));
        expect(rows.map(r => r.name)).toEqual(['art', 'raw', 'photos']);
    });

    it('does not reveal a grandchild whose parent is still collapsed', () => {
        const rows = visibleFolderRows(tree(), new Set(['/lib/art/raw']));
        expect(rows.map(r => r.name)).toEqual(['art', 'photos']);
    });

    it('keeps ancestors of a filter match as context', () => {
        const rows = visibleFolderRows(tree(), new Set(), 'nested');
        expect(rows.map(r => r.name)).toEqual(['art', 'raw', 'nested']);
    });

    it('ignores collapse state while filtering', () => {
        // Everything is collapsed, yet the match and its subtree still show.
        const collapsedRows = visibleFolderRows(tree(), new Set(), 'raw');
        expect(collapsedRows.map(r => r.name)).toEqual(['art', 'raw', 'nested']);
    });

    it('matches case-insensitively and returns nothing on no match', () => {
        expect(visibleFolderRows(tree(), new Set(), 'PHOTOS').map(r => r.name)).toEqual(['photos']);
        expect(visibleFolderRows(tree(), new Set(), 'zzz')).toEqual([]);
    });
});

describe('prunePinnedIds', () => {
    it('drops pins whose collection no longer exists', () => {
        expect(prunePinnedIds(['a', 'gone', 'b'], [['a', 'A', 1], ['b', 'B', 2]])).toEqual(['a', 'b']);
    });

    it('preserves pin order', () => {
        expect(prunePinnedIds(['b', 'a'], [['a', 'A', 1], ['b', 'B', 2]])).toEqual(['b', 'a']);
    });
});

describe('buildDisplayFolders regression guards', () => {
    it('keeps a folder that is itself the common prefix (its images used to vanish)', () => {
        const rows = buildDisplayFolders([
            ['/lib', 5],
            ['/lib/sub', 3],
        ]);
        const lib = rows.find(r => r.fullPath === '/lib')!;
        expect(lib).toBeDefined();
        expect(lib.count).toBe(5);
        expect(lib.subtreeCount).toBe(8);
    });

    it('still reconstructs a lone folder path', () => {
        const rows = buildDisplayFolders([['/Photos', 5]]);
        expect(rows).toHaveLength(1);
        expect(rows[0].fullPath).toBe('/Photos');
        expect(rows[0].subtreeCount).toBe(5);
    });

    it('emits the filesystem root instead of dropping it', () => {
        // list_folders() returns ("/", n) for files stored at the root; with no
        // path segments to walk, this used to vanish from the sidebar entirely.
        const rows = buildDisplayFolders([['/', 3]]);
        expect(rows).toHaveLength(1);
        expect(rows[0].fullPath).toBe('/');
        expect(rows[0].count).toBe(3);
    });

    it('renders the root alongside its siblings', () => {
        const rows = buildDisplayFolders([['/', 3], ['/lib', 5]]);
        expect(rows.map(r => r.fullPath)).toEqual(['/', '/lib']);
        expect(rows[0].count).toBe(3);
        expect(rows[0].subtreeCount).toBe(8);
    });
});

describe('visibleFolderRows filter reveals matched subtrees', () => {
    const groups = () => buildDisplayFolders([
        ['/p/src/a', 5],
        ['/p/src/b', 3],
        ['/p/docs', 2],
    ]);

    it('shows children of a matched group instead of a dead end', () => {
        // /p/src holds no images of its own. Excluding descendants would render
        // one row advertising 8 images with no way to reach them, and the
        // chevron cannot rescue it because filtering ignores expansion state.
        const rows = visibleFolderRows(groups(), new Set(), 'src');
        expect(rows.map(r => r.name)).toEqual(['src', 'a', 'b']);
        expect(rows[0].isGroup).toBe(true);
        expect(rows[0].subtreeCount).toBe(8);
    });

    it('keeps ancestors and descendants of a match', () => {
        const rows = visibleFolderRows(buildDisplayFolders([
            ['/lib/art', 1],
            ['/lib/art/raw', 2],
            ['/lib/art/raw/nested', 3],
            ['/lib/photos', 4],
        ]), new Set(), 'raw');
        expect(rows.map(r => r.name)).toEqual(['art', 'raw', 'nested']);
    });

    it('excludes unrelated subtrees entirely', () => {
        expect(visibleFolderRows(groups(), new Set(), 'docs').map(r => r.name)).toEqual(['docs']);
    });
});
