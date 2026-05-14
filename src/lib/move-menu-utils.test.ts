import { describe, expect, it } from 'vitest';
import { filterMoveFolders, folderDisplayName, folderParentPath } from './move-menu-utils';

describe('move menu folder helpers', () => {
    it('formats folder labels and parent paths', () => {
        expect(folderDisplayName('/Users/test/Images')).toBe('Images');
        expect(folderDisplayName('/Users/test/Images/')).toBe('Images');
        expect(folderParentPath('/Users/test/Images')).toBe('/Users/test');
        expect(folderParentPath('/Images')).toBe('/');
    });

    it('filters folders by case-insensitive path terms', () => {
        const folders: [string, number][] = [
            ['/Users/test/Midjourney/Final Picks', 4],
            ['/Users/test/Stable Diffusion/Drafts', 8],
            ['/Volumes/Archive/Final Renders', 2],
        ];

        expect(filterMoveFolders(folders, 'final').map(([path]) => path)).toEqual([
            '/Users/test/Midjourney/Final Picks',
            '/Volumes/Archive/Final Renders',
        ]);
        expect(filterMoveFolders(folders, 'stable drafts').map(([path]) => path)).toEqual([
            '/Users/test/Stable Diffusion/Drafts',
        ]);
    });
});
