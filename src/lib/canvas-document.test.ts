import { describe, expect, it } from 'vitest';
import {
    CANVAS_DOCUMENT_VERSION,
    createEmptyCanvasDocument,
    parseCanvasDocumentLayout,
    validateCanvasDocument,
    type CanvasDocument,
} from './canvas-document';

function validDocument(): CanvasDocument {
    return {
        version: CANVAS_DOCUMENT_VERSION,
        viewport: { panX: 0, panY: 0, zoom: 1 },
        items: [{
            id: 'item-1',
            imageId: 'img-1',
            x: 10,
            y: 20,
            width: 300,
            height: 200,
            z: 0,
            hidden: false,
            label: 'Hero',
            groupId: 'group-1',
            transform: {
                crop: { x: 0, y: 0, width: 1, height: 1 },
                rotationDegrees: 0,
                fit: 'contain',
            },
            source: {
                contentHash: 'hash-1',
                lastKnownPath: '/photos/hero.png',
            },
        }],
        groups: [{
            id: 'group-1',
            name: 'Selects',
            itemIds: ['item-1'],
            label: 'Selects',
            x: 0,
            y: 0,
            width: 400,
            height: 300,
            z: 0,
            collapsed: false,
        }],
        connectors: [{
            id: 'connector-1',
            fromItemId: 'item-1',
            toItemId: 'item-1',
            relationship: 'lineage',
            label: 'variant',
        }],
        annotations: [{
            id: 'note-1',
            target: { type: 'item', itemId: 'item-1' },
            body: 'Needs export review',
            x: 12,
            y: 18,
        }],
        export: {
            defaultPresetId: null,
            background: 'transparent',
            bounds: 'content',
        },
    };
}

describe('canvas document v1', () => {
    it('creates an empty v1 document', () => {
        const doc = createEmptyCanvasDocument();

        expect(doc.version).toBe(CANVAS_DOCUMENT_VERSION);
        expect(doc.items).toEqual([]);
        expect(doc.groups).toEqual([]);
        expect(doc.connectors).toEqual([]);
        expect(doc.annotations).toEqual([]);
    });

    it('parses legacy empty layout as an empty v1 document', () => {
        const doc = parseCanvasDocumentLayout('{}');

        expect(doc).toEqual(createEmptyCanvasDocument());
    });

    it('accepts a valid document', () => {
        expect(validateCanvasDocument(validDocument())).toEqual([]);
    });

    it('rejects invalid version', () => {
        const doc = { ...validDocument(), version: 2 };

        expect(validateCanvasDocument(doc as CanvasDocument)).toContain('unsupported canvas document version 2');
    });

    it('rejects duplicate item ids', () => {
        const doc = validDocument();
        doc.items.push({ ...doc.items[0], imageId: 'img-2' });

        expect(validateCanvasDocument(doc)).toContain('duplicate item id item-1');
    });

    it('rejects invalid item dimensions', () => {
        const doc = validDocument();
        doc.items[0].width = 0;

        expect(validateCanvasDocument(doc)).toContain('item item-1 width must be greater than 0');
    });

    it('rejects bad group connector and annotation references', () => {
        const doc = validDocument();
        doc.groups[0].itemIds = ['missing-item'];
        doc.connectors[0].toItemId = 'missing-item';
        doc.annotations[0].target = { type: 'item', itemId: 'missing-item' };

        const errors = validateCanvasDocument(doc);

        expect(errors).toContain('group group-1 references missing item missing-item');
        expect(errors).toContain('connector connector-1 references missing item missing-item');
        expect(errors).toContain('annotation note-1 references missing item missing-item');
    });
});
