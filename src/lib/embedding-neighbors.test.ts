import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LibraryScope } from './library-scope';

const mocks = vi.hoisted(() => ({
    findSimilarImagesInScope: vi.fn(),
    getImagesByIds: vi.fn(),
}));

vi.mock('./api', () => mocks);

import { loadEmbeddingNeighbors } from './embedding-neighbors';

const scope: LibraryScope = {
    type: 'collection',
    id: 'collection-1',
    include_rejected: false,
};

beforeEach(() => {
    vi.clearAllMocks();
});

describe('embedding neighbors', () => {
    it('keeps similarity scores paired with backend-ranked images', async () => {
        mocks.findSimilarImagesInScope.mockResolvedValue([
            ['near-a', 0.982],
            ['near-b', 0.761],
        ]);
        mocks.getImagesByIds.mockResolvedValue([
            { image: { id: 'near-b' }, path: '/photos/b.png' },
            { image: { id: 'near-a' }, path: '/photos/a.png' },
        ]);

        await expect(loadEmbeddingNeighbors(scope, 'source', 'clip-vit-b32', 6))
            .resolves.toEqual([
                expect.objectContaining({ score: 0.982, image: expect.objectContaining({ path: '/photos/a.png' }) }),
                expect.objectContaining({ score: 0.761, image: expect.objectContaining({ path: '/photos/b.png' }) }),
            ]);
        expect(mocks.findSimilarImagesInScope).toHaveBeenCalledWith(
            scope,
            'source',
            6,
            'clip-vit-b32',
        );
        expect(mocks.getImagesByIds).toHaveBeenCalledWith(['near-a', 'near-b']);
    });

    it('drops stale database rows while preserving ranked score identity', async () => {
        mocks.findSimilarImagesInScope.mockResolvedValue([
            ['missing', 0.9],
            ['present', 0.8],
        ]);
        mocks.getImagesByIds.mockResolvedValue([
            { image: { id: 'present' }, path: '/photos/present.png' },
        ]);

        await expect(loadEmbeddingNeighbors(scope, 'source', 'clip-vit-b32', 6))
            .resolves.toEqual([
                expect.objectContaining({ score: 0.8, image: expect.objectContaining({ path: '/photos/present.png' }) }),
            ]);
    });
});
