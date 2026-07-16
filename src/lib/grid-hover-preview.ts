export const GRID_SINGLE_HOVER_MIN_SIZE = 32;
export const GRID_HOVER_FOOTPRINT_PX = 32;
export const GRID_HOVER_SAMPLE_LIMIT = 9;

export interface GridHoverPreviewPlan {
    mode: 'single' | 'group';
    previewKey: string;
    anchorIndex: number;
    indices: number[];
    groupCount: number;
}

export interface GridPointerInput {
    pointerX: number;
    pointerY: number;
    scrollTop: number;
    cols: number;
    cellSize: number;
    totalItems: number;
}

export interface GridHoverPreviewInput extends GridPointerInput {
    thumbnailSize: number;
}

export interface GridHoverGroupBounds {
    startRow: number;
    startCol: number;
    side: number;
    rows: number;
    cols: number;
}

export function gridHoverGroupBounds(
    anchorIndex: number,
    cols: number,
    cellSize: number,
    totalItems: number,
): GridHoverGroupBounds {
    const side = Math.max(2, Math.ceil(GRID_HOVER_FOOTPRINT_PX / cellSize));
    const anchorRow = Math.floor(anchorIndex / cols);
    const anchorCol = anchorIndex % cols;
    const startRow = Math.floor(anchorRow / side) * side;
    const startCol = Math.floor(anchorCol / side) * side;
    const actualCols = Math.min(side, cols - startCol);
    const remainingItems = Math.max(0, totalItems - (startRow * cols + startCol));
    return {
        startRow,
        startCol,
        side,
        rows: Math.min(side, Math.ceil(remainingItems / cols)),
        cols: Math.min(actualCols, remainingItems),
    };
}

export function gridIndexAtPointer(
    pointerX: number,
    pointerY: number,
    scrollTop: number,
    cols: number,
    cellSize: number,
    totalItems: number,
): number | null {
    if (
        pointerX < 0 ||
        pointerY < 0 ||
        cols <= 0 ||
        cellSize <= 0 ||
        totalItems <= 0
    ) return null;

    const col = Math.floor(pointerX / cellSize);
    if (col < 0 || col >= cols) return null;
    const row = Math.floor((scrollTop + pointerY) / cellSize);
    const index = row * cols + col;
    return index >= 0 && index < totalItems ? index : null;
}

function boundedSample(indices: number[], limit: number): number[] {
    if (indices.length <= limit) return indices;
    const sampled: number[] = [];
    for (let i = 0; i < limit; i += 1) {
        const offset = limit === 1 ? 0 : Math.round(i * (indices.length - 1) / (limit - 1));
        sampled.push(indices[offset]);
    }
    return sampled;
}

export function planGridHoverPreview(input: GridHoverPreviewInput): GridHoverPreviewPlan | null {
    const anchorIndex = gridIndexAtPointer(
        input.pointerX,
        input.pointerY,
        input.scrollTop,
        input.cols,
        input.cellSize,
        input.totalItems,
    );
    if (anchorIndex === null) return null;

    if (input.thumbnailSize >= GRID_SINGLE_HOVER_MIN_SIZE) {
        return { mode: 'single', previewKey: `image:${anchorIndex}`, anchorIndex, indices: [anchorIndex], groupCount: 1 };
    }

    const { startRow, startCol, side } = gridHoverGroupBounds(
        anchorIndex,
        input.cols,
        input.cellSize,
        input.totalItems,
    );
    const group: number[] = [];

    for (let row = startRow; row < startRow + side; row += 1) {
        for (let col = startCol; col < Math.min(input.cols, startCol + side); col += 1) {
            const index = row * input.cols + col;
            if (index >= input.totalItems) break;
            group.push(index);
        }
    }

    return {
        mode: 'group',
        previewKey: `group:${startRow}:${startCol}:${side}`,
        anchorIndex,
        indices: boundedSample(group, GRID_HOVER_SAMPLE_LIMIT),
        groupCount: group.length,
    };
}
