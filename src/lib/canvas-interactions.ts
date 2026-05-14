export interface CanvasViewportTransform {
    panX: number;
    panY: number;
    zoom: number;
}

export interface CanvasPoint {
    x: number;
    y: number;
}

export interface CanvasKeyboardInput {
    key: string;
    code?: string;
    altKey?: boolean;
    ctrlKey?: boolean;
    metaKey?: boolean;
    targetTagName?: string | null;
    isContentEditable?: boolean;
}

export interface CanvasResizeInput {
    startClientX: number;
    currentClientX: number;
    startWidth: number;
    startHeight: number;
    imageWidth: number;
    imageHeight: number;
    zoom: number;
    minWidth?: number;
}

const DEFAULT_MIN_ZOOM = 0.1;
const DEFAULT_MAX_ZOOM = 5;
const DEFAULT_MIN_ITEM_WIDTH = 50;

export function computeCanvasWheelZoom(
    viewport: CanvasViewportTransform,
    pointer: CanvasPoint,
    deltaY: number,
    minZoom = DEFAULT_MIN_ZOOM,
    maxZoom = DEFAULT_MAX_ZOOM,
): CanvasViewportTransform {
    const factor = deltaY > 0 ? 0.9 : 1.1;
    const newZoom = clamp(viewport.zoom * factor, minZoom, maxZoom);
    if (viewport.zoom === 0) {
        return { ...viewport, zoom: newZoom };
    }

    return {
        panX: pointer.x - (pointer.x - viewport.panX) * (newZoom / viewport.zoom),
        panY: pointer.y - (pointer.y - viewport.panY) * (newZoom / viewport.zoom),
        zoom: newZoom,
    };
}

export function computeCanvasPanDrag(
    origin: CanvasViewportTransform,
    start: CanvasPoint,
    current: CanvasPoint,
): Pick<CanvasViewportTransform, 'panX' | 'panY'> {
    return {
        panX: origin.panX + (current.x - start.x),
        panY: origin.panY + (current.y - start.y),
    };
}

export function computeCanvasItemDragPosition(
    pointer: CanvasPoint,
    viewport: CanvasViewportTransform,
    dragOffset: CanvasPoint,
): CanvasPoint {
    return {
        x: (pointer.x - viewport.panX) / viewport.zoom - dragOffset.x,
        y: (pointer.y - viewport.panY) / viewport.zoom - dragOffset.y,
    };
}

export function computeCanvasResize(input: CanvasResizeInput): { width: number; height: number } {
    const zoom = input.zoom === 0 ? 1 : input.zoom;
    const dx = (input.currentClientX - input.startClientX) / zoom;
    const width = Math.max(input.minWidth ?? DEFAULT_MIN_ITEM_WIDTH, input.startWidth + dx);
    const aspect = safeAspect(input.imageWidth, input.imageHeight, input.startWidth, input.startHeight);

    return {
        width,
        height: width / aspect,
    };
}

export function worldToCanvasScreen(point: CanvasPoint, viewport: CanvasViewportTransform): CanvasPoint {
    return {
        x: point.x * viewport.zoom + viewport.panX,
        y: point.y * viewport.zoom + viewport.panY,
    };
}

export function isCanvasSpacePanKey(input: CanvasKeyboardInput): boolean {
    if (input.ctrlKey || input.metaKey || input.altKey) return false;
    if (input.key !== ' ' && input.key !== 'Spacebar' && input.code !== 'Space') return false;
    if (input.isContentEditable) return false;

    const tagName = input.targetTagName?.toUpperCase();
    return tagName !== 'INPUT' && tagName !== 'TEXTAREA' && tagName !== 'SELECT';
}

function safeAspect(imageWidth: number, imageHeight: number, fallbackWidth: number, fallbackHeight: number) {
    if (imageWidth > 0 && imageHeight > 0) return imageWidth / imageHeight;
    if (fallbackWidth > 0 && fallbackHeight > 0) return fallbackWidth / fallbackHeight;
    return 1;
}

function clamp(value: number, min: number, max: number) {
    return Math.max(min, Math.min(max, value));
}
