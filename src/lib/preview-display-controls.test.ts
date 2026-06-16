import { describe, expect, it } from 'vitest';
import {
    isPreviewDisplayPresetCycleShortcut,
    nextPreviewDisplayPresetMode,
    overlayForPreviewDisplayMode,
    previewDisplayStatusLabel,
    previewSyncImageId,
    withPreviewDisplayField,
} from './preview-display';
import { parsePreviewDisplayOverlay } from './preview-display-store';
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
        showDimensions: false,
        showFormat: false,
        showSource: false,
        showPrompt: false,
        showTags: false,
        showHistogram: false,
        railSide: 'right',
        railWidth: 'medium',
        railTextSize: 'medium',
    },
    frozen: true,
    blanked: false,
    version: 4,
    updated_at_ms: 1000,
};

describe('Preview Display controls', () => {
    it('cycles display presets in menu order from Ctrl+Tab', () => {
        expect(nextPreviewDisplayPresetMode('image_only')).toBe('client_review');
        expect(nextPreviewDisplayPresetMode('client_review')).toBe('metadata_review');
        expect(nextPreviewDisplayPresetMode('metadata_review')).toBe('image_only');
    });

    it('accepts only Ctrl+Tab as the preview display preset cycle shortcut', () => {
        expect(isPreviewDisplayPresetCycleShortcut({ key: 'Tab', ctrlKey: true })).toBe(true);
        expect(isPreviewDisplayPresetCycleShortcut({ key: 'Tab', ctrlKey: false })).toBe(false);
        expect(isPreviewDisplayPresetCycleShortcut({ key: 'Tab', ctrlKey: true, metaKey: true })).toBe(false);
        expect(isPreviewDisplayPresetCycleShortcut({ key: 'Enter', ctrlKey: true })).toBe(false);
    });

    it('maps display presets to bounded overlay fields', () => {
        expect(overlayForPreviewDisplayMode('image_only')).toEqual({
            showFilename: false,
            showRating: false,
            showDecision: false,
            showMetadataRail: false,
            showDimensions: false,
            showFormat: false,
            showSource: false,
            showPrompt: false,
            showTags: false,
            showHistogram: false,
            railSide: 'right',
            railWidth: 'medium',
            railTextSize: 'medium',
        });
        expect(overlayForPreviewDisplayMode('client_review')).toEqual({
            showFilename: true,
            showRating: true,
            showDecision: true,
            showMetadataRail: false,
            showDimensions: false,
            showFormat: false,
            showSource: false,
            showPrompt: false,
            showTags: false,
            showHistogram: false,
            railSide: 'right',
            railWidth: 'medium',
            railTextSize: 'medium',
        });
        expect(overlayForPreviewDisplayMode('metadata_review')).toEqual({
            showFilename: true,
            showRating: true,
            showDecision: true,
            showMetadataRail: true,
            showDimensions: true,
            showFormat: true,
            showSource: true,
            showPrompt: true,
            showTags: true,
            showHistogram: false,
            railSide: 'right',
            railWidth: 'medium',
            railTextSize: 'medium',
        });
    });

    it('parses persisted field toggles and bounds invalid rail options', () => {
        expect(parsePreviewDisplayOverlay(JSON.stringify({
            showFilename: true,
            showDimensions: true,
            showSource: true,
            showPrompt: true,
            showTags: true,
            showHistogram: true,
            railSide: 'top',
            railWidth: 'huge',
            railTextSize: 'microscopic',
        }))).toEqual({
            showFilename: true,
            showRating: false,
            showDecision: false,
            showMetadataRail: false,
            showDimensions: true,
            showFormat: false,
            showSource: true,
            showPrompt: true,
            showTags: true,
            showHistogram: true,
            railSide: 'right',
            railWidth: 'medium',
            railTextSize: 'medium',
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

    it('hides the metadata rail when the last rail field is disabled', () => {
        const enabled = withPreviewDisplayField(overlayForPreviewDisplayMode('image_only'), 'showPrompt', true);

        expect(enabled.showMetadataRail).toBe(true);

        const disabled = withPreviewDisplayField(enabled, 'showPrompt', false);

        expect(disabled.showPrompt).toBe(false);
        expect(disabled.showMetadataRail).toBe(false);
    });
});
