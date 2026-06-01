import { describe, expect, it } from 'vitest';
import {
    overlayForPreviewDisplayMode,
    previewDisplayStatusLabel,
    previewSyncImageId,
} from './preview-display';
import type { ImageWithFile, PreviewState } from './api';

function image(id: string): ImageWithFile {
    return {
        image: {
            id,
            sha256_hash: `${id}-hash`,
            width: 1000,
            height: 800,
            format: 'png',
            file_size: 100,
            created_at: '2026-06-01T00:00:00Z',
            imported_at: '2026-06-01T00:00:00Z',
            ai_prompt: null,
            raw_metadata: null,
        },
        path: `/images/${id}.png`,
        thumbnail_path: null,
        selection: null,
        source_label: null,
        missing_at: null,
    };
}

const frozenState: PreviewState = {
    image_id: 'held',
    display_mode: 'client_review',
    overlay: {
        showFilename: true,
        showRating: true,
        showDecision: true,
        showMetadataRail: false,
    },
    frozen: true,
    blanked: false,
    version: 4,
    updated_at_ms: 1000,
};

describe('Preview Display controls', () => {
    it('maps display presets to bounded overlay fields', () => {
        expect(overlayForPreviewDisplayMode('image_only')).toEqual({
            showFilename: false,
            showRating: false,
            showDecision: false,
            showMetadataRail: false,
        });
        expect(overlayForPreviewDisplayMode('client_review')).toEqual({
            showFilename: true,
            showRating: true,
            showDecision: true,
            showMetadataRail: false,
        });
        expect(overlayForPreviewDisplayMode('metadata_review')).toEqual({
            showFilename: true,
            showRating: true,
            showDecision: true,
            showMetadataRail: true,
        });
    });

    it('keeps the displayed image stable while frozen and hides it while blanked', () => {
        expect(previewSyncImageId(image('current'), frozenState, true, false)).toBe('held');
        expect(previewSyncImageId(image('current'), frozenState, false, true)).toBeNull();
        expect(previewSyncImageId(image('current'), frozenState, false, false)).toBe('current');
    });

    it('summarizes active safety state for the main app indicator', () => {
        expect(previewDisplayStatusLabel(false, false)).toBeNull();
        expect(previewDisplayStatusLabel(true, false)).toBe('Preview frozen');
        expect(previewDisplayStatusLabel(false, true)).toBe('Preview blanked');
        expect(previewDisplayStatusLabel(true, true)).toBe('Preview blanked');
    });
});
