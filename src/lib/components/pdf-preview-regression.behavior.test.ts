// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';

const mediaAsset = {
    id: 'ma_pdf-1',
    media_type: 'pdf',
    primary_image_id: 'pdf-1',
    sha256_hash: 'hash',
    format: 'pdf',
    file_size: 4096,
    page_count: 2,
    title: 'Test PDF',
    created_at: '2026-08-06T20:00:00Z',
    imported_at: '2026-08-06T20:00:00Z',
};

const pdfPages = [0, 1].map(page_index => ({
    id: `page-${page_index}`,
    media_asset_id: mediaAsset.id,
    page_index,
    width_points: 612,
    height_points: 792,
    thumbnail_path: null,
    preview_path: null,
    extracted_text: null,
    text_extracted_at: null,
}));

const pdfImage = {
    image: {
        id: 'pdf-1',
        sha256_hash: 'hash',
        width: 1200,
        height: 1200,
        format: 'pdf',
        file_size: 4096,
        created_at: '2026-08-06T20:00:00Z',
        imported_at: '2026-08-06T20:00:00Z',
        ai_prompt: null,
        raw_metadata: null,
    },
    path: '/Users/test/document.pdf',
    thumbnail_path: '/Users/test/Library/Application Support/com.glebkalinin.cull/thumbnails/pdf-1.jpg',
    selection: null,
    source_label: null,
    missing_at: null,
};

const api = vi.hoisted(() => ({
    getMediaAssetForImage: vi.fn(),
    listPdfPages: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => `asset://${path}`,
}));

vi.mock('$lib/api', () => ({
    cropImage: vi.fn(),
    getDetections: vi.fn().mockResolvedValue([]),
    getGenerationRun: vi.fn().mockResolvedValue(null),
    getImageFileBytes: vi.fn(),
    getImageHistogram: vi.fn(),
    getImagesByIds: vi.fn().mockResolvedValue([]),
    getMediaAssetForImage: api.getMediaAssetForImage,
    getVisionMetadata: vi.fn().mockResolvedValue([]),
    isRawFormat: (format: string) => format.toLowerCase() === 'pdf',
    listPdfPages: api.listPdfPages,
    regenerateSingleThumbnail: vi.fn(),
}));

import Loupe from './Loupe.svelte';
import Thumbnail from './Thumbnail.svelte';
import { focusedIndex, images } from '$lib/stores';

beforeAll(() => {
    vi.stubGlobal('ResizeObserver', class {
        observe() {}
        disconnect() {}
    });
});

afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    images.set([]);
    focusedIndex.set(0);
});

describe('PDF preview metadata requests', () => {
    it('loads a mounted PDF thumbnail page count once and renders its badge', async () => {
        api.getMediaAssetForImage.mockResolvedValue(mediaAsset);

        render(Thumbnail, {
            item: pdfImage,
            size: 160,
            focused: true,
            selected: false,
            onclick: vi.fn(),
            ondblclick: vi.fn(),
        });

        expect(await screen.findByText('PDF · 2p')).toBeVisible();
        await new Promise(resolve => setTimeout(resolve, 20));
        expect(api.getMediaAssetForImage).toHaveBeenCalledOnce();
    });

    it('loads focused PDF metadata once and allows the page label to settle', async () => {
        api.getMediaAssetForImage.mockResolvedValue(mediaAsset);
        api.listPdfPages.mockResolvedValue(pdfPages);
        images.set([pdfImage]);
        focusedIndex.set(0);

        render(Loupe);

        expect(await screen.findByText('Page 1/2')).toBeVisible();
        await waitFor(() => expect(api.listPdfPages).toHaveBeenCalledOnce());
        expect(api.getMediaAssetForImage).toHaveBeenCalledOnce();
    });
});
