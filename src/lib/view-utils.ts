export function getFilename(path: string, fallback: string = 'image'): string {
    return path.split('/').pop() || fallback;
}

export function getThumbnailBorderClass(focused: boolean, selected: boolean): string {
    if (focused) return 'focused';
    if (selected) return 'selected';
    return '';
}

export function buildRangeSelectionIds<T>(
    items: T[],
    anchorIndex: number,
    targetIndex: number,
    getId: (item: T) => string
): Set<string> {
    if (items.length === 0) return new Set();

    const maxIndex = items.length - 1;
    const anchor = Number.isFinite(anchorIndex) ? clamp(Math.trunc(anchorIndex), 0, maxIndex) : 0;
    const target = Number.isFinite(targetIndex) ? clamp(Math.trunc(targetIndex), 0, maxIndex) : anchor;
    const start = Math.min(anchor, target);
    const end = Math.max(anchor, target);

    return new Set(items.slice(start, end + 1).map(getId));
}

export interface GridClickSelectionInput<T> {
    items: T[];
    selectedIds: Set<string>;
    focusedIndex: number;
    anchorIndex: number | null;
    targetIndex: number;
    shiftKey: boolean;
    toggleKey: boolean;
    getId: (item: T) => string;
}

export interface GridClickSelectionResult {
    selectedIds: Set<string> | null;
    anchorIndex: number;
}

function normalizeGridIndex(index: number | null, fallback: number, maxIndex: number): number {
    const value = index === null || !Number.isFinite(index) ? fallback : index;
    return clamp(Math.trunc(value), 0, maxIndex);
}

export function computeGridClickSelection<T>({
    items,
    selectedIds,
    focusedIndex,
    anchorIndex,
    targetIndex,
    shiftKey,
    toggleKey,
    getId,
}: GridClickSelectionInput<T>): GridClickSelectionResult {
    if (items.length === 0) {
        return { selectedIds: null, anchorIndex: 0 };
    }

    const maxIndex = items.length - 1;
    const target = normalizeGridIndex(targetIndex, 0, maxIndex);
    const anchor = normalizeGridIndex(anchorIndex, focusedIndex, maxIndex);

    if (shiftKey) {
        const rangeIds = buildRangeSelectionIds(items, anchor, target, getId);
        const next = new Set(selectedIds);

        if (toggleKey) {
            for (const id of rangeIds) {
                if (next.has(id)) {
                    next.delete(id);
                } else {
                    next.add(id);
                }
            }
        } else {
            for (const id of rangeIds) next.add(id);
        }

        return { selectedIds: next, anchorIndex: anchor };
    }

    if (toggleKey) {
        const item = items[target];
        const next = new Set(selectedIds);
        const id = getId(item);
        if (next.has(id)) {
            next.delete(id);
        } else {
            next.add(id);
        }
        return { selectedIds: next, anchorIndex: target };
    }

    return { selectedIds: null, anchorIndex: target };
}

export interface LoupeImagePathCandidate {
    path: string;
    thumbnail_path?: string | null;
}

export function chooseLoupeImagePath(
    image: LoupeImagePathCandidate,
    isRaw: boolean,
    sourceLoadFailed: boolean
): string {
    if ((isRaw || sourceLoadFailed) && image.thumbnail_path) {
        return image.thumbnail_path;
    }
    return image.path;
}

export function computeGridLayout(
    containerWidth: number,
    thumbSize: number,
    gap: number,
    totalItems: number
): { cols: number; rows: number; cellSize: number; totalHeight: number } {
    const cols = Math.max(1, Math.floor((containerWidth + gap) / (thumbSize + gap)));
    const cellSize = thumbSize + gap;
    const rows = Math.ceil(totalItems / cols);
    const totalHeight = rows * cellSize;
    return { cols, rows, cellSize, totalHeight };
}

export interface VisibleItem {
    index: number;
    x: number;
    y: number;
}

export interface VisibleItemOptions {
    overscanRowsBefore?: number;
    overscanRowsAfter?: number;
}

function normalizeOverscanRows(value: number | undefined, fallback: number): number {
    if (value === undefined) return fallback;
    if (!Number.isFinite(value)) return fallback;
    return Math.max(0, Math.trunc(value));
}

export function computeVisibleItems(
    scrollTop: number,
    containerHeight: number,
    cols: number,
    cellSize: number,
    totalItems: number,
    options: VisibleItemOptions = {}
): VisibleItem[] {
    if (totalItems <= 0 || cols <= 0 || cellSize <= 0) return [];

    const firstVisibleRow = Math.max(0, Math.floor(scrollTop / cellSize));
    const visibleRowCount = Math.max(0, Math.ceil(containerHeight / cellSize));
    const overscanRowsBefore = normalizeOverscanRows(options.overscanRowsBefore, 0);
    const overscanRowsAfter = normalizeOverscanRows(options.overscanRowsAfter, 2);
    const rows = Math.ceil(totalItems / cols);
    const firstRenderedRow = Math.max(0, firstVisibleRow - overscanRowsBefore);
    const lastVisibleRow = Math.min(firstVisibleRow + visibleRowCount + overscanRowsAfter, rows);

    const items: VisibleItem[] = [];
    for (let row = firstRenderedRow; row < lastVisibleRow; row++) {
        for (let col = 0; col < cols; col++) {
            const index = row * cols + col;
            if (index >= totalItems) break;
            items.push({
                index,
                x: col * cellSize,
                y: row * cellSize,
            });
        }
    }
    return items;
}

export function formatLoupeInfo(filename: string, width: number, height: number, format: string): string {
    return `${filename} | ${width}x${height} | ${format}`;
}

export function computeWheelZoom(
    currentScale: number,
    deltaY: number,
    min: number = 0.1,
    max: number = 20
): number {
    const factor = deltaY < 0 ? 1.15 : 1 / 1.15;
    return Math.max(min, Math.min(max, currentScale * factor));
}

export function computePanDrag(
    startPan: { x: number; y: number },
    startMouse: { x: number; y: number },
    currentMouse: { x: number; y: number }
): { x: number; y: number } {
    return {
        x: startPan.x + (currentMouse.x - startMouse.x),
        y: startPan.y + (currentMouse.y - startMouse.y),
    };
}

export interface RectLike {
    left: number;
    top: number;
    width: number;
    height: number;
}

export interface CropPoint {
    x: number;
    y: number;
}

export interface CropRect {
    x: number;
    y: number;
    width: number;
    height: number;
}

export interface CropSelectionPercent {
    left: number;
    top: number;
    width: number;
    height: number;
}

export type CropResizeHandle = 'n' | 's' | 'e' | 'w' | 'nw' | 'ne' | 'sw' | 'se';

function clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, value));
}

function cropRectFromSides(left: number, top: number, right: number, bottom: number): CropRect {
    return {
        x: Math.round(left),
        y: Math.round(top),
        width: Math.round(right - left),
        height: Math.round(bottom - top),
    };
}

export function clientToImagePoint(
    clientX: number,
    clientY: number,
    imageRect: RectLike,
    imageWidth: number,
    imageHeight: number
): CropPoint | null {
    if (imageRect.width <= 0 || imageRect.height <= 0 || imageWidth <= 0 || imageHeight <= 0) {
        return null;
    }

    const x = ((clientX - imageRect.left) / imageRect.width) * imageWidth;
    const y = ((clientY - imageRect.top) / imageRect.height) * imageHeight;

    return {
        x: clamp(x, 0, imageWidth),
        y: clamp(y, 0, imageHeight),
    };
}

export function cropRectFromImagePoints(
    start: CropPoint,
    end: CropPoint,
    imageWidth: number,
    imageHeight: number
): CropRect {
    const x1 = clamp(Math.min(start.x, end.x), 0, imageWidth);
    const y1 = clamp(Math.min(start.y, end.y), 0, imageHeight);
    const x2 = clamp(Math.max(start.x, end.x), 0, imageWidth);
    const y2 = clamp(Math.max(start.y, end.y), 0, imageHeight);

    return cropRectFromSides(x1, y1, x2, y2);
}

export function cropSelectionPercentFromImagePoints(
    start: CropPoint,
    end: CropPoint,
    imageWidth: number,
    imageHeight: number
): CropSelectionPercent | null {
    if (imageWidth <= 0 || imageHeight <= 0) return null;

    const x1 = clamp(Math.min(start.x, end.x), 0, imageWidth);
    const y1 = clamp(Math.min(start.y, end.y), 0, imageHeight);
    const x2 = clamp(Math.max(start.x, end.x), 0, imageWidth);
    const y2 = clamp(Math.max(start.y, end.y), 0, imageHeight);

    return {
        left: (x1 / imageWidth) * 100,
        top: (y1 / imageHeight) * 100,
        width: ((x2 - x1) / imageWidth) * 100,
        height: ((y2 - y1) / imageHeight) * 100,
    };
}

export function moveCropRect(
    rect: CropRect,
    deltaX: number,
    deltaY: number,
    imageWidth: number,
    imageHeight: number
): CropRect {
    const width = clamp(rect.width, 0, imageWidth);
    const height = clamp(rect.height, 0, imageHeight);

    return {
        x: Math.round(clamp(rect.x + deltaX, 0, imageWidth - width)),
        y: Math.round(clamp(rect.y + deltaY, 0, imageHeight - height)),
        width: Math.round(width),
        height: Math.round(height),
    };
}

export function resizeCropRectFromHandle(
    rect: CropRect,
    handle: CropResizeHandle,
    point: CropPoint,
    imageWidth: number,
    imageHeight: number,
    minSize: number = 1
): CropRect {
    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;

    const minWidth = Math.min(minSize, imageWidth);
    const minHeight = Math.min(minSize, imageHeight);

    if (handle.includes('w')) {
        left = clamp(point.x, 0, right - minWidth);
    }
    if (handle.includes('e')) {
        right = clamp(point.x, left + minWidth, imageWidth);
    }
    if (handle.includes('n')) {
        top = clamp(point.y, 0, bottom - minHeight);
    }
    if (handle.includes('s')) {
        bottom = clamp(point.y, top + minHeight, imageHeight);
    }

    return cropRectFromSides(left, top, right, bottom);
}
