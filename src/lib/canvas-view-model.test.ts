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
    addImagesToCanvasDocument,
    addCanvasItemAnnotation,
    applyCanvasViewItemCrop,
    canvasItemAnnotations,
    createCanvasDocumentForImages,
    createCanvasDocumentFromLayoutJson,
    createCanvasViewItems,
    rotateCanvasViewItemClockwise,
    setCanvasViewItemCropFromPoints,
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

    it('keeps saved canvas items from other folders when loading a visible subset', () => {
        const layout = serializeCanvasDocumentLayout({
            ...createEmptyCanvasDocument(),
            items: [
                {
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
                    source: { contentHash: 'old-hash-a', lastKnownPath: '/old/a.png' },
                },
                {
                    id: 'canvas-item-b',
                    imageId: 'img-b',
                    x: 500,
                    y: 240,
                    width: 160,
                    height: 220,
                    z: 4,
                    hidden: false,
                    label: 'Other folder',
                    groupId: null,
                    transform: { crop: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 }, rotationDegrees: 90, fit: 'contain' },
                    source: { contentHash: 'old-hash-b', lastKnownPath: '/old/b.png' },
                },
            ],
        });

        const doc = createCanvasDocumentFromLayoutJson(layout, [image('img-a')]);

        expect(doc.items).toHaveLength(2);
        expect(doc.items.find(item => item.imageId === 'img-a')).toMatchObject({
            source: { contentHash: 'hash-img-a', lastKnownPath: '/library/img-a.png' },
        });
        expect(doc.items.find(item => item.imageId === 'img-b')).toMatchObject({
            x: 500,
            y: 240,
            label: 'Other folder',
            transform: { crop: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 }, rotationDegrees: 90 },
            source: { contentHash: 'old-hash-b', lastKnownPath: '/old/b.png' },
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

    it('saves visible canvas edits without dropping off-folder items or notes', () => {
        const doc: CanvasDocument = {
            ...createEmptyCanvasDocument(),
            items: [
                {
                    id: 'canvas-item-a',
                    imageId: 'img-a',
                    x: 0,
                    y: 0,
                    width: 200,
                    height: 200,
                    z: 0,
                    hidden: false,
                    label: null,
                    groupId: null,
                    transform: { crop: null, rotationDegrees: 0, fit: 'contain' },
                    source: { contentHash: 'hash-img-a', lastKnownPath: '/library/img-a.png' },
                },
                {
                    id: 'canvas-item-b',
                    imageId: 'img-b',
                    x: 500,
                    y: 240,
                    width: 160,
                    height: 220,
                    z: 1,
                    hidden: false,
                    label: 'Other folder',
                    groupId: null,
                    transform: { crop: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 }, rotationDegrees: 90, fit: 'contain' },
                    source: { contentHash: 'hash-img-b', lastKnownPath: '/library/img-b.png' },
                },
            ],
            annotations: [{
                id: 'note-b',
                target: { type: 'item', itemId: 'canvas-item-b' },
                body: 'Preserve this note',
                x: 0.5,
                y: 0.5,
            }],
        };
        const items = createCanvasViewItems(doc, [image('img-a')]);
        items[0] = { ...items[0], x: 123, y: 45, width: 300, height: 150 };

        const updated = updateCanvasDocumentFromViewItems(doc, items, {
            panX: 11,
            panY: -7,
            zoom: 1.4,
        });

        expect(updated.items).toHaveLength(2);
        expect(updated.items.find(item => item.imageId === 'img-a')).toMatchObject({
            x: 123,
            y: 45,
            width: 300,
            height: 150,
        });
        expect(updated.items.find(item => item.imageId === 'img-b')).toMatchObject({
            x: 500,
            y: 240,
            width: 160,
            height: 220,
            label: 'Other folder',
            transform: { crop: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 }, rotationDegrees: 90 },
        });
        expect(updated.annotations).toEqual(doc.annotations);
        expect(validateCanvasDocument(updated)).toEqual([]);
    });

    it('round-trips non-destructive item rotation through view items', () => {
        const doc: CanvasDocument = {
            ...createEmptyCanvasDocument(),
            items: [{
                id: 'canvas-item-a',
                imageId: 'img-a',
                x: 0,
                y: 0,
                width: 200,
                height: 120,
                z: 0,
                hidden: false,
                label: null,
                groupId: null,
                transform: { crop: null, rotationDegrees: 270, fit: 'contain' },
                source: { contentHash: 'hash-img-a', lastKnownPath: '/library/img-a.png' },
            }],
        };

        const items = createCanvasViewItems(doc, [image('img-a')]);
        expect(items[0].rotationDegrees).toBe(270);

        items[0] = { ...items[0], rotationDegrees: 0 };
        const updated = updateCanvasDocumentFromViewItems(doc, items, {
            panX: 0,
            panY: 0,
            zoom: 1,
        });

        expect(updated.items[0].transform).toMatchObject({
            crop: null,
            rotationDegrees: 0,
            fit: 'contain',
        });
    });

    it('rotates canvas view items clockwise in 90 degree steps', () => {
        const [item] = createCanvasViewItems(createCanvasDocumentForImages([image('img-a')]), [image('img-a')]);

        expect(rotateCanvasViewItemClockwise({ ...item, rotationDegrees: 270 }).rotationDegrees).toBe(0);
        expect(rotateCanvasViewItemClockwise({ ...item, rotationDegrees: -90 }).rotationDegrees).toBe(0);
    });

    it('round-trips non-destructive item crop through view items', () => {
        const doc: CanvasDocument = {
            ...createEmptyCanvasDocument(),
            items: [{
                id: 'canvas-item-a',
                imageId: 'img-a',
                x: 0,
                y: 0,
                width: 200,
                height: 120,
                z: 0,
                hidden: false,
                label: null,
                groupId: null,
                transform: { crop: { x: 0.2, y: 0.1, width: 0.5, height: 0.6 }, rotationDegrees: 0, fit: 'contain' },
                source: { contentHash: 'hash-img-a', lastKnownPath: '/library/img-a.png' },
            }],
        };

        const items = createCanvasViewItems(doc, [image('img-a')]);
        expect(items[0].crop).toEqual({ x: 0.2, y: 0.1, width: 0.5, height: 0.6 });

        items[0] = applyCanvasViewItemCrop(items[0], { x: 0.25, y: 0.2, width: 0.4, height: 0.5 });
        const updated = updateCanvasDocumentFromViewItems(doc, items, {
            panX: 0,
            panY: 0,
            zoom: 1,
        });

        expect(updated.items[0].transform.crop).toEqual({ x: 0.25, y: 0.2, width: 0.4, height: 0.5 });
        expect(validateCanvasDocument(updated)).toEqual([]);
    });

    it('sets normalized crop rectangles from dragged item points', () => {
        const [item] = createCanvasViewItems(createCanvasDocumentForImages([image('img-a')]), [image('img-a')]);

        const cropped = setCanvasViewItemCropFromPoints(
            item,
            { x: 0.75, y: 0.8 },
            { x: 0.25, y: 0.2 },
        );

        expect(cropped.crop).toEqual({ x: 0.25, y: 0.2, width: 0.5, height: 0.6 });
    });

    it('adds dropped images to a compact grid from the drop point', () => {
        const doc = createCanvasDocumentForImages([image('img-a')]);

        const result = addImagesToCanvasDocument(
            doc,
            [image('img-b', 400, 200), image('img-c', 100, 200), image('img-d', 200, 200)],
            { x: 50, y: 60 },
        );

        expect(result.addedImageIds).toEqual(['img-b', 'img-c', 'img-d']);
        expect(result.skippedImageIds).toEqual([]);
        expect(result.document.items.map(item => item.imageId)).toEqual(['img-a', 'img-b', 'img-c', 'img-d']);
        expect(result.document.items.slice(1)).toMatchObject([
            { imageId: 'img-b', x: 50, y: 60, width: 400, height: 200, z: 1 },
            { imageId: 'img-c', x: 470, y: 60, width: 100, height: 200, z: 2 },
            { imageId: 'img-d', x: 50, y: 280, width: 200, height: 200, z: 3 },
        ]);
        expect(validateCanvasDocument(result.document)).toEqual([]);
    });

    it('does not duplicate images already present on the canvas', () => {
        const doc = createCanvasDocumentForImages([image('img-a')]);

        const result = addImagesToCanvasDocument(doc, [image('img-a'), image('img-b')], { x: 10, y: 20 });

        expect(result.addedImageIds).toEqual(['img-b']);
        expect(result.skippedImageIds).toEqual(['img-a']);
        expect(result.document.items.map(item => item.imageId)).toEqual(['img-a', 'img-b']);
    });

    it('adds item annotations to the persisted canvas document', () => {
        const doc = createCanvasDocumentForImages([image('img-a')]);

        const updated = addCanvasItemAnnotation(doc, 'img-a', 'Use this as the hero crop', {
            id: 'note-a',
            x: 0.5,
            y: 0.25,
            createdAt: '2026-05-16T10:00:00.000Z',
        });

        expect(updated.annotations).toEqual([{
            id: 'note-a',
            target: { type: 'item', itemId: 'img-a' },
            body: 'Use this as the hero crop',
            x: 0.5,
            y: 0.25,
            createdAt: '2026-05-16T10:00:00.000Z',
            author: null,
        }]);
        expect(canvasItemAnnotations(updated, 'img-a')).toHaveLength(1);
        expect(validateCanvasDocument(updated)).toEqual([]);
    });
});
