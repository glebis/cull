import { describe, expect, it } from 'vitest';
import {
    clampPreviewDisplayPan,
    clampPreviewDisplayZoom,
    isPreviewDisplayPresetCycleShortcut,
    nextPreviewDisplayPresetMode,
    overlayForPreviewDisplayMode,
    previewDisplayFitSize,
    previewDisplayNormalizedFocus,
    previewDisplayPanForNormalizedFocus,
    previewDisplayStatusLabel,
    previewSyncImageId,
    previewSyncImageIds,
    withPreviewDisplayField,
} from './preview-display';
import { parsePreviewDisplayLayout, parsePreviewDisplayOverlay } from './preview-display-store';
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
    image_ids: ['held'],
    display_mode: 'client_review',
    layout: 'single',
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
        expect(parsePreviewDisplayLayout('compare')).toBe('compare');
        expect(parsePreviewDisplayLayout('grid')).toBe('grid');
        expect(parsePreviewDisplayLayout('poster')).toBe('single');

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

    it('builds layout image sets from focus, selection, and loaded neighbours', () => {
        const loaded = [image('a'), image('b'), image('c'), image('d'), image('e')];

        expect(previewSyncImageIds(loaded[1], loaded, new Set(), null, false, false, 'compare')).toEqual(['b', 'c']);
        expect(previewSyncImageIds(loaded[1], loaded, new Set(['d', 'a']), null, false, false, 'grid')).toEqual(['b', 'a', 'd']);
        expect(previewSyncImageIds(loaded[1], loaded, new Set(), {
            ...frozenState,
            image_ids: ['x', 'y'],
        }, true, false, 'grid')).toEqual(['x', 'y']);
        expect(previewSyncImageIds(loaded[1], loaded, new Set(), null, false, true, 'grid')).toEqual([]);
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

    it('fits Preview Display images into the viewport before applying user zoom', () => {
        expect(previewDisplayFitSize(
            { width: 4000, height: 2000 },
            { width: 1000, height: 1000 }
        )).toEqual({ width: 1000, height: 500 });

        expect(previewDisplayFitSize(
            { width: 2000, height: 4000 },
            { width: 1000, height: 1000 }
        )).toEqual({ width: 500, height: 1000 });
    });

    it('bounds Preview Display zoom and clamps pan to visible image overflow', () => {
        const imageSize = { width: 4000, height: 2000 };
        const viewport = { width: 1000, height: 1000 };

        expect(clampPreviewDisplayZoom(0.25)).toBe(1);
        expect(clampPreviewDisplayZoom(100)).toBe(20);
        expect(clampPreviewDisplayPan(imageSize, viewport, 1, { x: 500, y: 500 })).toEqual({ x: 0, y: 0 });
        expect(clampPreviewDisplayPan(imageSize, viewport, 3, { x: 2000, y: -2000 })).toEqual({
            x: 1000,
            y: -250,
        });
    });

    it('reapplies a saved Preview Display focus point to a different image when possible', () => {
        const landscape = { width: 4000, height: 2000 };
        const portrait = { width: 2000, height: 4000 };
        const viewport = { width: 1000, height: 1000 };
        const zoom = 3;
        const pan = { x: -300, y: 100 };

        const focus = previewDisplayNormalizedFocus(landscape, viewport, zoom, pan);
        const nextPan = previewDisplayPanForNormalizedFocus(portrait, viewport, zoom, focus);
        const restoredFocus = previewDisplayNormalizedFocus(portrait, viewport, zoom, nextPan);

        expect(focus).toEqual({ x: 0.6, y: 0.43333333333333335 });
        expect(restoredFocus.x).toBeCloseTo(focus.x);
        expect(restoredFocus.y).toBeCloseTo(focus.y);
    });
});
