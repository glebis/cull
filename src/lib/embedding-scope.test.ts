import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LibraryScope } from './library-scope';

const mocks = vi.hoisted(() => ({
    getScopedEmbeddingPage: vi.fn(),
    listScopedImageIds: vi.fn(),
}));
vi.mock('./api', () => mocks);

import {
    getEmbeddingCountForScope,
    getEmbeddingPageForScope,
    getImageCountForScope,
    listImageIdsForScope,
} from './embedding-scope';

const allScope: LibraryScope = { type: 'all', include_rejected: false };
const folderScope: LibraryScope = {
    type: 'folder',
    path: '/Photos/Scoped',
    min_size: 512,
    include_rejected: false,
};

beforeEach(() => {
    vi.clearAllMocks();
    mocks.getScopedEmbeddingPage.mockResolvedValue({
        ids: ['folder-a'], vectors: [3, 4], dims: 2, total: 7, offset: 0, limit: 5000, has_more: false,
    });
    mocks.listScopedImageIds.mockResolvedValue({
        ids: ['folder-a', 'folder-b'], total: 2, offset: 0, limit: 100, has_more: false,
    });
});

describe('embedding scope API routing', () => {
    it('routes All Images through the scoped API so rejected visibility is preserved', async () => {
        await expect(getEmbeddingPageForScope(allScope, 'clip-vit-b32', 5000, 0))
            .resolves.toMatchObject({ ids: ['folder-a'], total: 7 });
        await expect(getEmbeddingCountForScope(allScope, 'clip-vit-b32'))
            .resolves.toBe(7);
        await expect(getImageCountForScope(allScope)).resolves.toBe(2);
        await expect(listImageIdsForScope(allScope)).resolves.toEqual(['folder-a', 'folder-b']);

        expect(mocks.getScopedEmbeddingPage).toHaveBeenNthCalledWith(
            1,
            allScope,
            'clip-vit-b32',
            5000,
            0,
        );
        expect(mocks.listScopedImageIds).toHaveBeenNthCalledWith(1, allScope, 1, 0);
        expect(mocks.listScopedImageIds).toHaveBeenNthCalledWith(2, allScope, 100, 0);
    });

    it('filters before pagination and generation for an active scope', async () => {
        await expect(getEmbeddingPageForScope(folderScope, 'clip-vit-b32', 5000, 0))
            .resolves.toMatchObject({ ids: ['folder-a'], total: 7 });
        await expect(getEmbeddingCountForScope(folderScope, 'clip-vit-b32'))
            .resolves.toBe(7);
        await expect(listImageIdsForScope(folderScope)).resolves.toEqual(['folder-a', 'folder-b']);

        expect(mocks.getScopedEmbeddingPage).toHaveBeenNthCalledWith(
            1,
            folderScope,
            'clip-vit-b32',
            5000,
            0,
        );
        expect(mocks.getScopedEmbeddingPage).toHaveBeenNthCalledWith(
            2,
            folderScope,
            'clip-vit-b32',
            1,
            0,
        );
        expect(mocks.listScopedImageIds).toHaveBeenCalledWith(folderScope, 100, 0);
    });

    it('collects every scoped ID through service-sized pages', async () => {
        mocks.listScopedImageIds
            .mockResolvedValueOnce({
                ids: ['a', 'b'], total: 3, offset: 0, limit: 100, has_more: true,
            })
            .mockResolvedValueOnce({
                ids: ['c'], total: 3, offset: 2, limit: 100, has_more: false,
            });

        await expect(listImageIdsForScope(folderScope)).resolves.toEqual(['a', 'b', 'c']);
        expect(mocks.listScopedImageIds).toHaveBeenNthCalledWith(1, folderScope, 100, 0);
        expect(mocks.listScopedImageIds).toHaveBeenNthCalledWith(2, folderScope, 100, 2);
    });
});
