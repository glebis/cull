import { describe, expect, it } from 'vitest';
import type { ImageWithFile } from './api';
import {
    CANVAS_DOCUMENT_VERSION,
    createEmptyCanvasDocument,
    serializeCanvasDocumentLayout,
    validateCanvasDocument,
    type CanvasDocument,
} from './canvas-document';
import {
    createCanvasDocumentForImages,
    createCanvasDocumentFromLayoutJson,
    createCanvasViewItems,
    updateCanvasDocumentFromViewItems,
} from './canvas-view-model';

function image(id: string, width = 200, height = 200): ImageWithFile {
    return {
        image: {
            id,
            sha256_hash: `hash-${id}`,
            width,
            height,
            format: 'png',
            file_size: 1024,
            created_at: '2026-05-14T00:00:00Z',
            imported_at: '2026-05-14T00:00:00Z',
            ai_prompt: null,
            raw_metadata: null,
        },
        path: `/library/${id}.png`,
        thumbnail_path: `/library/${id}.thumb.png`,
        selection: null,
        source_label: null,
        missing_at: null,
    };
}

describe('canvas view model', () => {
    it('creates a v1 grid document from visible images', () => {
        const doc = createCanvasDocumentForImages([
            image('img-a', 200, 200),
            image('img-b', 100, 200),
        ]);

        expect(doc.version).toBe(CANVAS_DOCUMENT_VERSION);
        expect(doc.viewport).toEqual({ panX: 0, panY: 0, zoom: 1 });
        expect(doc.items).toMatchObject([
            {
                id: 'img-a',
                imageId: 'img-a',
                x: 0,
                y: 0,
                width: 200,
                height: 200,
                source: { contentHash: 'hash-img-a', lastKnownPath: '/library/img-a.png' },
            },
            {
                id: 'img-b',
                imageId: 'img-b',
                x: 220,
                y: 0,
                width: 100,
                height: 200,
                source: { contentHash: 'hash-img-b', lastKnownPath: '/library/img-b.png' },
            },
        ]);
        expect(validateCanvasDocument(doc)).toEqual([]);
    });

    it('maps saved document geometry to canvas view items', () => {
        const doc: CanvasDocument = {
            ...createEmptyCanvasDocument(),
            items: [{
                id: 'canvas-item-a',
                imageId: 'img-a',
                x: 42,
                y: 64,
                width: 320,
                height: 180,
                z: 3,
                hidden: false,
                label: 'Hero',
                groupId: null,
                transform: { crop: null, rotationDegrees: 0, fit: 'contain' },
                source: { contentHash: 'hash-img-a', lastKnownPath: '/library/img-a.png' },
            }],
        };

        const items = createCanvasViewItems(doc, [image('img-a'), image('img-b')]);

        expect(items).toHaveLength(1);
        expect(items[0]).toMatchObject({
            id: 'canvas-item-a',
            imageId: 'img-a',
            x: 42,
            y: 64,
            width: 320,
            height: 180,
            z: 3,
        });
        expect(items[0].image.image.id).toBe('img-a');
    });

    it('treats non-versioned legacy layout blobs as grid v1 documents', () => {
        const doc = createCanvasDocumentFromLayoutJson(
            '{"images":[{"id":"legacy","x":10,"y":20}]}',
            [image('img-a')],
        );

        expect(doc.version).toBe(CANVAS_DOCUMENT_VERSION);
        expect(doc.items).toHaveLength(1);
        expect(doc.items[0]).toMatchObject({
            id: 'img-a',
            imageId: 'img-a',
            x: 0,
            y: 0,
        });
    });

    it('preserves saved v1 document geometry when loading layout JSON', () => {
        const layout = serializeCanvasDocumentLayout({
            ...createEmptyCanvasDocument(),
            viewport: { panX: 4, panY: 5, zoom: 1.2 },
            items: [{
                id: 'canvas-item-a',
                imageId: 'img-a',
                x: 42,
                y: 64,
                width: 320,
                height: 180,
                z: 3,
                hidden: false,
                label: null,
                groupId: null,
                transform: { crop: null, rotationDegrees: 0, fit: 'contain' },
                source: { contentHash: 'old-hash', lastKnownPath: '/old/path.png' },
            }],
        });

        const doc = createCanvasDocumentFromLayoutJson(layout, [image('img-a')]);

        expect(doc.viewport).toEqual({ panX: 4, panY: 5, zoom: 1.2 });
        expect(doc.items[0]).toMatchObject({
            id: 'canvas-item-a',
            imageId: 'img-a',
            x: 42,
            y: 64,
            width: 320,
            height: 180,
            source: { contentHash: 'hash-img-a', lastKnownPath: '/library/img-a.png' },
        });
    });

    it('serializes moved view items back into a valid document', () => {
        const doc = createCanvasDocumentForImages([image('img-a')]);
        const items = createCanvasViewItems(doc, [image('img-a')]);
        items[0] = { ...items[0], x: 123, y: 45, width: 300, height: 150 };

        const updated = updateCanvasDocumentFromViewItems(doc, items, {
            panX: 11,
            panY: -7,
            zoom: 1.4,
        });

        expect(updated.viewport).toEqual({ panX: 11, panY: -7, zoom: 1.4 });
        expect(updated.items[0]).toMatchObject({
            id: 'img-a',
            imageId: 'img-a',
            x: 123,
            y: 45,
            width: 300,
            height: 150,
            source: { contentHash: 'hash-img-a', lastKnownPath: '/library/img-a.png' },
        });
        expect(validateCanvasDocument(updated)).toEqual([]);
        expect(() => serializeCanvasDocumentLayout(updated)).not.toThrow();
    });
});
