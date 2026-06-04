// Pure layout + labeling logic for the contact sheet exporter. Kept free of
// DOM/canvas so it can be unit-tested; the Svelte component does the drawing.

import type { ImageWithFile } from './api';

export interface ContactSheetConfig {
    columns: number;
    cellWidth: number;
    cellHeight: number;
    gap: number;
    margin: number;
    // Extra height reserved under each cell for its caption when labels show.
    labelHeight: number;
    showFilename: boolean;
    showRating: boolean;
    showMetadata: boolean;
}

export interface ContactSheetCell {
    index: number;
    row: number;
    col: number;
    // Image draw box.
    x: number;
    y: number;
    width: number;
    height: number;
    // Caption baseline position (top of the label band under the image).
    labelX: number;
    labelY: number;
}

export interface ContactSheetLayout {
    width: number;
    height: number;
    cells: ContactSheetCell[];
}

export const DEFAULT_CONTACT_SHEET_CONFIG: ContactSheetConfig = {
    columns: 4,
    cellWidth: 320,
    cellHeight: 240,
    gap: 16,
    margin: 32,
    labelHeight: 28,
    showFilename: true,
    showRating: true,
    showMetadata: false,
};

function cellOuterHeight(config: ContactSheetConfig): number {
    const labelBand = config.showFilename || config.showRating || config.showMetadata
        ? config.labelHeight
        : 0;
    return config.cellHeight + labelBand;
}

export function computeContactSheetLayout(count: number, config: ContactSheetConfig): ContactSheetLayout {
    const columns = Math.max(1, Math.floor(config.columns));
    const rows = count > 0 ? Math.ceil(count / columns) : 0;
    const outerH = cellOuterHeight(config);

    const width = config.margin * 2 + columns * config.cellWidth + Math.max(0, columns - 1) * config.gap;
    const height = rows === 0
        ? config.margin * 2
        : config.margin * 2 + rows * outerH + Math.max(0, rows - 1) * config.gap;

    const cells: ContactSheetCell[] = [];
    for (let index = 0; index < count; index += 1) {
        const row = Math.floor(index / columns);
        const col = index % columns;
        const x = config.margin + col * (config.cellWidth + config.gap);
        const y = config.margin + row * (outerH + config.gap);
        cells.push({
            index,
            row,
            col,
            x,
            y,
            width: config.cellWidth,
            height: config.cellHeight,
            labelX: x,
            labelY: y + config.cellHeight,
        });
    }

    return { width, height, cells };
}

// Build the caption shown beneath a cell from the enabled fields.
export function contactSheetCellLabel(image: ImageWithFile, config: ContactSheetConfig): string {
    const parts: string[] = [];
    if (config.showFilename) {
        parts.push(image.path.split('/').filter(Boolean).pop() ?? image.image.id);
    }
    if (config.showRating) {
        const rating = image.selection?.star_rating ?? 0;
        if (rating > 0) parts.push('★'.repeat(rating));
    }
    if (config.showMetadata) {
        const w = image.image.width;
        const h = image.image.height;
        if (w && h) parts.push(`${w}×${h}`);
    }
    return parts.join('  ');
}
