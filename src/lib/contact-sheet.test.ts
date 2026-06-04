import { describe, expect, it } from 'vitest';
import {
    computeContactSheetLayout,
    contactSheetCellLabel,
    DEFAULT_CONTACT_SHEET_CONFIG,
    type ContactSheetConfig,
} from './contact-sheet';
import type { ImageWithFile } from './api';

const config: ContactSheetConfig = {
    ...DEFAULT_CONTACT_SHEET_CONFIG,
    columns: 3,
    cellWidth: 100,
    cellHeight: 80,
    gap: 10,
    margin: 20,
    labelHeight: 20,
};

describe('contact sheet layout', () => {
    it('arranges cells into a grid and sizes the canvas to fit', () => {
        const layout = computeContactSheetLayout(5, config);
        // 3 columns, 5 items -> 2 rows.
        expect(layout.cells).toHaveLength(5);
        expect(layout.cells[0]).toMatchObject({ row: 0, col: 0, x: 20, y: 20 });
        expect(layout.cells[3]).toMatchObject({ row: 1, col: 0 });
        // width = 2*margin + 3*cellW + 2*gap = 40 + 300 + 20 = 360
        expect(layout.width).toBe(360);
        // outerH = cellH + labelHeight = 100; height = 40 + 2*100 + 1*gap = 250
        expect(layout.height).toBe(250);
    });

    it('places the second column with gap offset', () => {
        const layout = computeContactSheetLayout(2, config);
        expect(layout.cells[1].x).toBe(20 + 100 + 10);
        expect(layout.cells[1].y).toBe(20);
    });

    it('drops the label band when no caption fields are enabled', () => {
        const noLabels = { ...config, showFilename: false, showRating: false, showMetadata: false };
        const layout = computeContactSheetLayout(3, noLabels);
        // single row, outerH == cellH (80): height = 40 + 80 = 120
        expect(layout.height).toBe(120);
    });

    it('handles an empty set without negative dimensions', () => {
        const layout = computeContactSheetLayout(0, config);
        expect(layout.cells).toHaveLength(0);
        expect(layout.height).toBe(40);
    });
});

function img(name: string, rating: number | null, w: number, h: number): ImageWithFile {
    return {
        image: { id: name, width: w, height: h } as never,
        path: `/photos/${name}.jpg`,
        thumbnail_path: null,
        selection: rating === null ? null : ({ star_rating: rating } as never),
    } as unknown as ImageWithFile;
}

describe('contact sheet cell labels', () => {
    it('combines filename, rating, and metadata when enabled', () => {
        const label = contactSheetCellLabel(img('sunset', 4, 1920, 1080), {
            ...config,
            showFilename: true,
            showRating: true,
            showMetadata: true,
        });
        expect(label).toContain('sunset.jpg');
        expect(label).toContain('★★★★');
        expect(label).toContain('1920×1080');
    });

    it('omits rating stars when unrated', () => {
        const label = contactSheetCellLabel(img('plain', null, 100, 100), {
            ...config,
            showFilename: true,
            showRating: true,
            showMetadata: false,
        });
        expect(label).toBe('plain.jpg');
    });
});
