// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { ImageWithFile } from '$lib/api';

const mocks = vi.hoisted(() => ({
    getImagesByIds: vi.fn(),
    listen: vi.fn(),
    openImageInLoupe: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: vi.fn((path: string) => path) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('$lib/view-utils', () => ({ safeAssetPreviewPath: vi.fn(() => null) }));
vi.mock('$lib/image-loading', () => ({
    invalidateImageCache: vi.fn(),
    loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined),
}));
vi.mock('$lib/similarity', () => ({ loadSimilarImages: vi.fn() }));
vi.mock('$lib/api', () => ({
    addToCollection: vi.fn(),
    createCollection: vi.fn(),
    getImagesByIds: mocks.getImagesByIds,
    listCollections: vi.fn().mockResolvedValue([]),
    listFolders: vi.fn().mockResolvedValue([]),
    listOpenWithApplications: vi.fn().mockResolvedValue([]),
    moveImage: vi.fn(),
    openImagesWithApplication: vi.fn(),
    removeFromCollection: vi.fn(),
    renameImage: vi.fn(),
    setDecision: vi.fn(),
    setRating: vi.fn(),
    shareImages: vi.fn(),
}));
vi.mock('$lib/stores', async importOriginal => ({
    ...(await importOriginal<typeof import('$lib/stores')>()),
    openImageInLoupe: mocks.openImageInLoupe,
}));

import GenerationResultsStrip from './GenerationResultsStrip.svelte';

function image(id: string): ImageWithFile {
    return {
        image: {
            id,
            sha256_hash: `hash-${id}`,
            width: 100,
            height: 100,
            format: 'png',
            file_size: 100,
            created_at: '2026-08-08T00:00:00Z',
            imported_at: '2026-08-08T00:00:00Z',
            ai_prompt: null,
            raw_metadata: null,
        },
        path: `/generated/${id}.png`,
        thumbnail_path: null,
        selection: null,
        source_label: null,
        missing_at: null,
    };
}

afterEach(() => cleanup());
beforeEach(() => vi.clearAllMocks());

describe('GenerationResultsStrip context menu', () => {
    it('opens the menu on right-click without navigating to the image', async () => {
        let completionHandler: ((event: { payload: { image_ids: string[]; job_id: string } }) => void) | undefined;
        mocks.listen.mockImplementation(async (name: string, handler: typeof completionHandler) => {
            if (name === 'generation-complete') completionHandler = handler;
            return vi.fn();
        });
        mocks.getImagesByIds.mockResolvedValue([image('one'), image('two')]);

        render(GenerationResultsStrip);
        await waitFor(() => expect(completionHandler).toBeTypeOf('function'));
        await completionHandler?.({ payload: { image_ids: ['one', 'two'], job_id: 'job-one' } });
        expect(await screen.findByTitle('two.png')).toBeInTheDocument();
        mocks.openImageInLoupe.mockClear();

        await fireEvent.contextMenu(screen.getByTitle('two.png'), { clientX: 40, clientY: 60 });

        expect(mocks.openImageInLoupe).not.toHaveBeenCalled();
        expect(screen.getByRole('menu', { hidden: true })).toBeInTheDocument();
    });
});
