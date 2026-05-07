export function getFilename(path: string, fallback: string = 'image'): string {
    return path.split('/').pop() || fallback;
}

export function getThumbnailBorderClass(focused: boolean, selected: boolean): string {
    if (focused) return 'focused';
    if (selected) return 'selected';
    return '';
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

export function computeVisibleItems(
    scrollTop: number,
    containerHeight: number,
    cols: number,
    cellSize: number,
    totalItems: number
): VisibleItem[] {
    const firstVisibleRow = Math.floor(scrollTop / cellSize);
    const visibleRowCount = Math.ceil(containerHeight / cellSize) + 2;
    const rows = Math.ceil(totalItems / cols);
    const lastVisibleRow = Math.min(firstVisibleRow + visibleRowCount, rows);

    const items: VisibleItem[] = [];
    for (let row = firstVisibleRow; row < lastVisibleRow; row++) {
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
