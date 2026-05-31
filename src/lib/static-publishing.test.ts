import { describe, expect, it } from 'vitest';
import type { Canvas } from './api';
import {
    buildStaticPublishRequestFromSavedCanvas,
    buildStaticPublishShareItems,
    countSavedCanvasItems,
    parseStaticPublishLinks,
} from './static-publishing';
import type { StaticPublishResult, StaticPublishServerResult } from './api';

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
            siteTitle: ' Shared Gallery ',
            siteDescription: ' A compact client review page. ',
            indexable: false,
            links: [{ label: 'Project brief', url: 'https://example.test/brief' }],
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
        expect(request.site_title).toBe('Shared Gallery');
        expect(request.site_description).toBe('A compact client review page.');
        expect(request.indexable).toBe(false);
        expect(request.links).toEqual([{ label: 'Project brief', url: 'https://example.test/brief' }]);
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

    it('parses one custom static site link per line', () => {
        expect(parseStaticPublishLinks([
            'Project brief | https://example.test/brief',
            'Moodboard: https://example.test/moodboard',
            'broken line',
            ' | https://example.test/no-label',
        ].join('\n'))).toEqual([
            { label: 'Project brief', url: 'https://example.test/brief' },
            { label: 'Moodboard', url: 'https://example.test/moodboard' },
        ]);
    });

    it('parses public tunnel links for static publish handoff', () => {
        expect(parseStaticPublishLinks([
            'ngrok preview | https://cull-demo.ngrok-free.app',
            'Tailscale Funnel | https://studio.tailnet.ts.net',
        ].join('\n'))).toEqual([
            { label: 'ngrok preview', url: 'https://cull-demo.ngrok-free.app' },
            { label: 'Tailscale Funnel', url: 'https://studio.tailnet.ts.net' },
        ]);
    });

    it('builds copyable and shareable result items with openable target and preview URLs', () => {
        const result: StaticPublishResult = {
            export_dir: '/tmp/cull-publish',
            site_dir: '/tmp/cull-publish/site',
            manifest_path: '/tmp/cull-publish/site/data/canvas.json',
            instructions_path: '/tmp/cull-publish/instructions/CLAUDE.md',
            qr_svg_path: '/tmp/cull-publish/site/qr.svg',
            qr_target_url: 'https://cull-demo.ngrok-free.app/',
            access_phrase: 'amber-canvas-river',
            image_count: 4,
            skipped_count: 0,
            warnings: [],
        };
        const serverResult: StaticPublishServerResult = {
            url: 'http://127.0.0.1:8000/',
            host: '127.0.0.1',
            port: 8000,
            site_dir: '/tmp/cull-publish/site',
        };

        const items = buildStaticPublishShareItems(result, serverResult);

        expect(items.map(item => item.id)).toEqual([
            'site-folder',
            'manifest',
            'agent-notes',
            'qr-code',
            'target-url',
            'access-phrase',
            'preview-url',
        ]);
        expect(items.every(item => item.copyable)).toBe(true);
        expect(items.every(item => item.shareable)).toBe(true);
        expect(items.find(item => item.id === 'target-url')).toMatchObject({
            label: 'Target URL',
            value: 'https://cull-demo.ngrok-free.app/',
            kind: 'url',
            openable: true,
        });
        expect(items.find(item => item.id === 'preview-url')).toMatchObject({
            label: 'Preview URL',
            value: 'http://127.0.0.1:8000/',
            kind: 'url',
            openable: true,
        });
        expect(items.find(item => item.id === 'access-phrase')).toMatchObject({
            kind: 'secret',
            openable: false,
        });
    });
});
