import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

vi.mock('$lib/api', () => ({
    evaluateSmartCollection: vi.fn(),
    getImageCount: vi.fn(),
    listCollectionImages: vi.fn(),
    listImagesByDetectedClass: vi.fn(),
    listImages: vi.fn(),
    listImagesByFolder: vi.fn(),
    listImagesFiltered: vi.fn(),
}));

import { getImageCount, listImages } from '$lib/api';
import {
    clearImageScope,
    invalidateImageCache,
    loadImagesForCurrentScope,
    resetImagePaging,
} from '$lib/image-loading';
import { imageLoadState, images } from '$lib/stores';
import { resolveLibraryViewState } from '$lib/library-view-state';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

function stateInput() {
    const state = get(imageLoadState);
    return {
        loading: state.loading,
        error: state.error,
        loaded: state.loaded,
        imageCount: get(images).length,
    };
}

describe('backend init failure produces a distinct error state', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        images.set([]);
        clearImageScope();
        invalidateImageCache();
        resetImagePaging();
        vi.mocked(getImageCount).mockResolvedValue(0);
    });

    it('a rejected initial load yields the error state, not the empty-library state', async () => {
        vi.mocked(listImages).mockRejectedValue('database is locked');

        await loadImagesForCurrentScope();

        const state = get(imageLoadState);
        expect(state.error).toContain('database is locked');
        expect(state.loading).toBe(false);
        expect(resolveLibraryViewState(stateInput())).toBe('error');
        expect(resolveLibraryViewState(stateInput())).not.toBe('empty');
    });

    it('a successful empty first query yields the genuine empty state', async () => {
        vi.mocked(listImages).mockResolvedValue([]);

        await loadImagesForCurrentScope();

        const state = get(imageLoadState);
        expect(state.error).toBeNull();
        expect(state.loaded).toBe(true);
        expect(resolveLibraryViewState(stateInput())).toBe('empty');
    });

    it('a retry that succeeds clears the error state', async () => {
        vi.mocked(listImages).mockRejectedValueOnce('init failed');
        await loadImagesForCurrentScope();
        expect(get(imageLoadState).error).not.toBeNull();

        vi.mocked(listImages).mockResolvedValue([]);
        await loadImagesForCurrentScope({ force: true });

        expect(get(imageLoadState).error).toBeNull();
        expect(resolveLibraryViewState(stateInput())).toBe('empty');
    });

    it('never reports empty before the first query settles', () => {
        expect(
            resolveLibraryViewState({ loading: true, error: null, loaded: false, imageCount: 0 }),
        ).toBe('loading');
        expect(
            resolveLibraryViewState({ loading: false, error: null, loaded: false, imageCount: 0 }),
        ).toBe('loading');
        expect(
            resolveLibraryViewState({ loading: false, error: 'boom', loaded: false, imageCount: 0 }),
        ).toBe('error');
        expect(
            resolveLibraryViewState({ loading: false, error: null, loaded: true, imageCount: 3 }),
        ).toBe('loaded');
    });

    it('Grid renders an alert with retry instead of the empty copy on error', () => {
        const grid = source('src/lib/components/Grid.svelte');

        expect(grid).toContain('role="alert"');
        expect(grid).toContain('Retry');
        expect(grid).toContain('resolveLibraryViewState');
        expect(grid).toContain('var(--red)');
        // Empty-state copy must be gated behind the resolved view state,
        // not bare `$images.length === 0`.
        expect(grid).not.toContain('{#if $images.length === 0}');
    });

    it('Sidebar initial loads surface failures instead of console.error-only', () => {
        const sidebar = source('src/lib/components/Sidebar.svelte');

        expect(sidebar).toContain("showToast('Failed to load folders'");
        expect(sidebar).toContain("showToast('Failed to load collections'");
    });
});
