import { describe, expect, it } from 'vitest';
import type { Canvas } from './api';
import { buildStaticPublishRequestFromSavedCanvas, countSavedCanvasItems } from './static-publishing';

function canvas(layoutJson: string): Canvas {
    return {
        id: 'canvas-1',
        session_id: 'session-1',
        name: 'Saved Board',
        canvas_type: 'manual',
        layout_json: layoutJson,
        filter_json: null,
        grid_config_json: null,
        sort_order: 0,
        created_at: '2026-05-14T00:00:00Z',
        updated_at: '2026-05-14T01:00:00Z',
    };
}

describe('static publishing canvas helpers', () => {
    it('builds a static publish request from saved canvas layout', () => {
        const layoutJson = JSON.stringify({
            version: 1,
            viewport: { panX: 10, panY: 20, zoom: 1.25 },
            items: [{
                id: 'item-b',
                imageId: 'img-b',
                x: 42,
                y: 64,
                width: 320,
                height: 180,
                z: 2,
                hidden: true,
                label: 'Alt',
                groupId: null,
                transform: { crop: null, rotationDegrees: 0, fit: 'contain' },
                source: { contentHash: 'hash-b', lastKnownPath: '/library/b.png' },
            }],
            groups: [],
            connectors: [],
            annotations: [],
            export: { defaultPresetId: null, background: 'transparent', bounds: 'content' },
        });

        const request = buildStaticPublishRequestFromSavedCanvas({
            canvas: canvas(layoutJson),
            canvasName: ' Shared Gallery ',
            outputDir: ' /tmp/export ',
            shareUrl: ' https://example.test/canvas ',
            includeThumbnails: true,
            includeWeb: false,
            includeFull: true,
        });

        expect(request.canvas_name).toBe('Shared Gallery');
        expect(request.items).toEqual([{
            image_id: 'img-b',
            x: 42,
            y: 64,
            width: 320,
            height: 180,
            hidden: true,
        }]);
        expect(request.output_dir).toBe('/tmp/export');
        expect(request.share_url).toBe('https://example.test/canvas');
        expect(request.include_thumbnails).toBe(true);
        expect(request.include_web).toBe(false);
        expect(request.include_full).toBe(true);
        expect(JSON.parse(request.layout_json ?? '{}')).toMatchObject({
            version: 1,
            viewport: { panX: 10, panY: 20, zoom: 1.25 },
            items: [{ id: 'item-b', imageId: 'img-b', hidden: true }],
        });
    });

    it('counts saved canvas items without using visible image state', () => {
        const layoutJson = JSON.stringify({
            version: 1,
            viewport: { panX: 0, panY: 0, zoom: 1 },
            items: [
                { id: 'item-a', imageId: 'img-a', x: 0, y: 0, width: 100, height: 100, z: 0, hidden: false },
                { id: 'item-b', imageId: 'img-b', x: 120, y: 0, width: 100, height: 100, z: 1, hidden: true },
            ],
            groups: [],
            connectors: [],
            annotations: [],
            export: { defaultPresetId: null, background: 'transparent', bounds: 'content' },
        });

        expect(countSavedCanvasItems(canvas(layoutJson))).toBe(2);
        expect(countSavedCanvasItems(null)).toBe(0);
    });
});
