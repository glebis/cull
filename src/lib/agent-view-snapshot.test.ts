import { describe, expect, it } from 'vitest';
import {
    buildAgentSnapshotManifest,
    collectVisibleImageTargetsFromRects,
    imageIdsForSnapshotLabels,
    type AgentSnapshotRectItem,
} from './agent-view-snapshot';

const items: AgentSnapshotRectItem[] = [
    {
        imageId: 'img-a',
        filename: 'a.png',
        path: '/library/a.png',
        thumbnailPath: '/library/.thumbs/a.jpg',
        aiPrompt: 'portrait in a quiet neon studio',
        generationPrompt: 'portrait in a quiet neon studio',
        generationProvider: 'openai',
        generationModel: 'gpt-image-2',
        generationSeed: '42',
        generationSettingsJson: '{"size":"1024x1024"}',
        bounds: { left: 0, top: 0, width: 80, height: 80 },
        rating: 5,
        decision: 'accept',
        viewRole: 'grid-cell',
    },
    {
        imageId: 'img-b',
        filename: 'b.png',
        path: '/library/b.png',
        thumbnailPath: null,
        aiPrompt: null,
        generationPrompt: null,
        generationProvider: null,
        generationModel: null,
        generationSeed: null,
        generationSettingsJson: null,
        bounds: { left: 170, top: 0, width: 100, height: 100 },
        rating: null,
        decision: 'undecided',
        viewRole: 'grid-cell',
    },
    {
        imageId: 'img-c',
        filename: 'c.png',
        path: '/library/c.png',
        thumbnailPath: null,
        aiPrompt: null,
        generationPrompt: null,
        generationProvider: null,
        generationModel: null,
        generationSeed: null,
        generationSettingsJson: null,
        bounds: { left: 190, top: 0, width: 100, height: 100 },
        rating: 2,
        decision: 'reject',
        viewRole: 'grid-cell',
    },
];

describe('agent view snapshot helpers', () => {
    it('collects only meaningfully visible image targets and assigns stable labels', () => {
        const targets = collectVisibleImageTargetsFromRects({
            viewMode: 'grid',
            viewport: { left: 0, top: 0, width: 200, height: 100 },
            devicePixelRatio: 2,
            visibleThreshold: 0.2,
            selectedIds: new Set(['img-b']),
            focusedImageId: 'img-a',
            items,
        });

        expect(targets.map(target => target.image_id)).toEqual(['img-a', 'img-b']);
        expect(targets.map(target => target.label)).toEqual(['1', '2']);
        expect(targets[0]).toMatchObject({
            image_id: 'img-a',
            filename: 'a.png',
            path: '/library/a.png',
            thumbnail_path: '/library/.thumbs/a.jpg',
            ai_prompt: 'portrait in a quiet neon studio',
            generation_prompt: 'portrait in a quiet neon studio',
            generation_provider: 'openai',
            generation_model: 'gpt-image-2',
            generation_seed: '42',
            generation_settings_json: '{"size":"1024x1024"}',
            bounds_css: { left: 0, top: 0, width: 80, height: 80 },
            bounds_px: { left: 0, top: 0, width: 160, height: 160 },
            visible_ratio: 1,
            focused: true,
            selected: false,
            rating: 5,
            decision: 'accept',
            view_role: 'grid-cell',
        });
        expect(targets[1]).toMatchObject({
            image_id: 'img-b',
            bounds_css: { left: 170, top: 0, width: 100, height: 100 },
            bounds_px: { left: 340, top: 0, width: 200, height: 200 },
            visible_ratio: 0.3,
            focused: false,
            selected: true,
        });
    });

    it('builds a manifest with capture scope, file outputs, and visible images', () => {
        const targets = collectVisibleImageTargetsFromRects({
            viewMode: 'compare',
            viewport: { left: 0, top: 0, width: 200, height: 100 },
            devicePixelRatio: 1,
            selectedIds: new Set(['img-a']),
            focusedImageId: 'img-b',
            items: items.slice(0, 2),
        });

        const manifest = buildAgentSnapshotManifest({
            snapshotId: 'snap_123',
            createdAt: '2026-06-04T10:30:00.000Z',
            viewMode: 'compare',
            captureReason: 'shortcut',
            destination: { kind: 'local', detail: '/app/Agent Snapshots/snap_123' },
            files: {
                raw_png: '/app/Agent Snapshots/snap_123/raw.png',
                annotated_png: '/app/Agent Snapshots/snap_123/annotated.png',
                manifest_json: '/app/Agent Snapshots/snap_123/manifest.json',
            },
            window: {
                label: 'main',
                title: 'Cull',
                width_css: 200,
                height_css: 100,
                device_pixel_ratio: 1,
            },
            scope: {
                kind: 'folder',
                id: null,
                label: 'Library',
                path: '/library',
            },
            visibleImages: targets,
        });

        expect(manifest).toMatchObject({
            schema_version: 1,
            snapshot_id: 'snap_123',
            created_at: '2026-06-04T10:30:00.000Z',
            view_mode: 'compare',
            capture_reason: 'shortcut',
            destination: { kind: 'local', detail: '/app/Agent Snapshots/snap_123' },
            files: {
                raw_png: '/app/Agent Snapshots/snap_123/raw.png',
                annotated_png: '/app/Agent Snapshots/snap_123/annotated.png',
                manifest_json: '/app/Agent Snapshots/snap_123/manifest.json',
            },
            window: { label: 'main', width_css: 200, height_css: 100, device_pixel_ratio: 1 },
            scope: { kind: 'folder', path: '/library' },
        });
        expect(manifest.visible_images.map(image => image.label)).toEqual(['1', '2']);
    });

    it('maps selected labels back to image IDs and rejects unknown labels', () => {
        const visibleImages = collectVisibleImageTargetsFromRects({
            viewMode: 'grid',
            viewport: { left: 0, top: 0, width: 200, height: 100 },
            devicePixelRatio: 1,
            selectedIds: new Set(),
            focusedImageId: null,
            items: items.slice(0, 2),
        });
        const manifest = buildAgentSnapshotManifest({
            snapshotId: 'snap_123',
            createdAt: '2026-06-04T10:30:00.000Z',
            viewMode: 'grid',
            captureReason: 'shortcut',
            destination: { kind: 'local', detail: '/app/Agent Snapshots/snap_123' },
            files: {
                raw_png: '/app/Agent Snapshots/snap_123/raw.png',
                annotated_png: '/app/Agent Snapshots/snap_123/annotated.png',
                manifest_json: '/app/Agent Snapshots/snap_123/manifest.json',
            },
            window: {
                label: 'main',
                title: 'Cull',
                width_css: 200,
                height_css: 100,
                device_pixel_ratio: 1,
            },
            scope: { kind: 'all', id: null, label: 'All Images', path: null },
            visibleImages,
        });

        expect(imageIdsForSnapshotLabels(manifest, ['2', '1'])).toEqual(['img-b', 'img-a']);
        expect(() => imageIdsForSnapshotLabels(manifest, ['99'])).toThrow('Unknown snapshot label: 99');
    });
});
