import { describe, expect, it } from 'vitest';
import {
    DEFAULT_PREVIEW_OVERLAY,
    isPreviewDisplayRoute,
    nextPreviewFocusPayload,
    previewDisplayImageSourcePath,
} from './preview-display';
import type { ImageWithFile, PreviewState } from './api';

function image(id: string, format = 'png', thumbnailPath: string | null = '/thumbs/a.jpg'): ImageWithFile {
    return {
        image: {
            id,
            sha256_hash: `${id}-hash`,
            width: 1600,
            height: 1000,
            format,
            file_size: 1200,
            created_at: '2026-06-01T00:00:00Z',
            imported_at: '2026-06-01T00:00:00Z',
            ai_prompt: null,
            raw_metadata: null,
        },
        path: `/images/${id}.${format}`,
        thumbnail_path: thumbnailPath,
        selection: null,
        source_label: null,
        missing_at: null,
    };
}

const metadataState: PreviewState = {
    image_id: 'old',
    display_mode: 'metadata_review',
    overlay: {
        ...DEFAULT_PREVIEW_OVERLAY,
        showFilename: true,
        showRating: true,
        showDecision: true,
        showMetadataRail: true,
    },
    frozen: false,
    blanked: false,
    version: 7,
    updated_at_ms: 1000,
};

describe('Preview Display utilities', () => {
    it('detects the dedicated preview display route', () => {
        expect(isPreviewDisplayRoute('?previewDisplay=1')).toBe(true);
        expect(isPreviewDisplayRoute('?previewDisplay=true')).toBe(true);
        expect(isPreviewDisplayRoute('?window=preview-display')).toBe(true);
        expect(isPreviewDisplayRoute('?previewDisplay=0')).toBe(false);
        expect(isPreviewDisplayRoute('?view=grid')).toBe(false);
    });

    it('builds a focus sync payload while preserving display mode and overlays', () => {
        expect(nextPreviewFocusPayload(image('new'), metadataState)).toEqual({
            imageId: 'new',
            displayMode: 'metadata_review',
            overlay: metadataState.overlay,
        });
    });

    it('falls back to the default image-only overlay without current preview state', () => {
        expect(nextPreviewFocusPayload(null, null)).toEqual({
            imageId: null,
            displayMode: 'image_only',
            overlay: DEFAULT_PREVIEW_OVERLAY,
        });
    });

    it('uses RAW thumbnails and source-load fallback consistently with Loupe', () => {
        expect(previewDisplayImageSourcePath(image('raw', 'dng'), false)).toBe('/thumbs/a.jpg');
        expect(previewDisplayImageSourcePath(image('pdf', 'pdf'), false)).toBe('/thumbs/a.jpg');
        expect(previewDisplayImageSourcePath(image('png'), true)).toBe('/thumbs/a.jpg');
        expect(previewDisplayImageSourcePath(image('png'), false)).toBe('/images/png.png');
    });
});
