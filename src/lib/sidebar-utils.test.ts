import { describe, it, expect } from 'vitest';
import { folderName, buildDisplayFolders, formatImportResult, formatSidebarCount } from './sidebar-utils';

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

    it('uses zero for missing counts', () => {
        expect(formatSidebarCount(null)).toBe('0');
        expect(formatSidebarCount(undefined)).toBe('0');
    });
});
