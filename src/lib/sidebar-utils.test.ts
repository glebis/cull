import { describe, it, expect } from 'vitest';
import { folderName, buildDisplayFolders, formatImportResult } from './sidebar-utils';

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

    it('builds a single folder', () => {
        const result = buildDisplayFolders([['/Users/test/Photos', 10]]);
        expect(result).toHaveLength(1);
        expect(result[0].name).toBe('Photos');
        expect(result[0].disambig).toBe('');
        expect(result[0].fullPath).toBe('/Users/test/Photos');
        expect(result[0].count).toBe(10);
    });

    it('sorts by name alphabetically', () => {
        const result = buildDisplayFolders([
            ['/z/Zebras', 1],
            ['/a/Apples', 2],
            ['/m/Mangos', 3],
        ]);
        expect(result.map(f => f.name)).toEqual(['Apples', 'Mangos', 'Zebras']);
    });

    it('disambiguates duplicate folder names with exact parent context', () => {
        const result = buildDisplayFolders([
            ['/Users/alice/Photos', 5],
            ['/Users/bob/Photos', 3],
        ]);
        expect(result).toHaveLength(2);
        const byPath = Object.fromEntries(result.map(f => [f.fullPath, f.disambig]));
        expect(byPath['/Users/alice/Photos']).toBe('Users/alice');
        expect(byPath['/Users/bob/Photos']).toBe('Users/bob');
    });

    it('does not disambiguate unique names', () => {
        const result = buildDisplayFolders([
            ['/a/Cats', 1],
            ['/b/Dogs', 2],
        ]);
        expect(result[0].disambig).toBe('');
        expect(result[1].disambig).toBe('');
    });

    it('shows up to 2 parent segments for disambiguation', () => {
        const result = buildDisplayFolders([
            ['/root/level1/level2/Photos', 1],
            ['/other/path/here/Photos', 2],
        ]);
        const byPath = Object.fromEntries(result.map(f => [f.fullPath, f.disambig]));
        expect(byPath['/root/level1/level2/Photos']).toBe('level1/level2');
        expect(byPath['/other/path/here/Photos']).toBe('path/here');
    });

    it('disambiguates root-level duplicate names', () => {
        const result = buildDisplayFolders([
            ['/Photos', 5],
            ['/Users/Photos', 3],
        ]);
        expect(result).toHaveLength(2);
        const byPath = Object.fromEntries(result.map(f => [f.fullPath, f.disambig]));
        expect(byPath['/Photos']).toBe('');
        expect(byPath['/Users/Photos']).toBe('Users');
    });

    it('disambiguates three folders with the same name', () => {
        const result = buildDisplayFolders([
            ['/a/b/Imports', 1],
            ['/c/d/Imports', 2],
            ['/e/f/Imports', 3],
        ]);
        const disambigs = result.map(f => f.disambig);
        expect(new Set(disambigs).size).toBe(3);
        expect(disambigs.every(d => d.length > 0)).toBe(true);
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
