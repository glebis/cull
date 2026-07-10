export interface LoupeTransform {
    scale: number;
    panX: number;
    panY: number;
}

export interface LoupeViewport {
    width: number;
    height: number;
}

export interface LoupeImageSize {
    width: number;
    height: number;
}

export interface LoupePoint {
    x: number;
    y: number;
}

const DEFAULT_MIN_SCALE = 0.1;
const DEFAULT_MAX_SCALE = 20;
const FIT_EPSILON = 0.02;
const ZOOM_STEP_EPSILON = 0.001;

export const LOUPE_NATURAL_ZOOM_PRESETS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 4];

export function computeLoupeActualSizeScale(viewport: LoupeViewport, image: LoupeImageSize): number {
    const scale = fitScale(viewport, image);
    if (!Number.isFinite(scale) || scale <= 0) return 1;
    return clamp(Math.max(1, 1 / scale), DEFAULT_MIN_SCALE, DEFAULT_MAX_SCALE);
}

export function computeLoupeFitSize(viewport: LoupeViewport, image: LoupeImageSize): LoupeImageSize {
    if (viewport.width <= 0 || viewport.height <= 0 || image.width <= 0 || image.height <= 0) {
        return { width: 0, height: 0 };
    }

    const scale = fitScale(viewport, image);
    return {
        width: image.width * scale,
        height: image.height * scale,
    };
}

export function computeLoupeNaturalScale(
    viewport: LoupeViewport,
    image: LoupeImageSize,
    viewportScale: number,
): number {
    const scale = fitScale(viewport, image) * viewportScale;
    return Number.isFinite(scale) && scale > 0 ? scale : 1;
}

export function computeLoupeViewportScaleForNaturalScale(
    viewport: LoupeViewport,
    image: LoupeImageSize,
    naturalScale: number,
): number {
    const scale = fitScale(viewport, image);
    if (!Number.isFinite(scale) || scale <= 0) return 1;
    return clamp(naturalScale / scale, DEFAULT_MIN_SCALE, DEFAULT_MAX_SCALE);
}

export function nextLoupeNaturalZoomPreset(currentScale: number, direction: 1 | -1): number {
    if (!Number.isFinite(currentScale) || currentScale <= 0) {
        return direction > 0 ? 1 : LOUPE_NATURAL_ZOOM_PRESETS[0];
    }

    if (direction > 0) {
        return LOUPE_NATURAL_ZOOM_PRESETS.find(scale => scale > currentScale + ZOOM_STEP_EPSILON)
            ?? LOUPE_NATURAL_ZOOM_PRESETS[LOUPE_NATURAL_ZOOM_PRESETS.length - 1];
    }

    return [...LOUPE_NATURAL_ZOOM_PRESETS].reverse().find(scale => scale < currentScale - ZOOM_STEP_EPSILON)
        ?? LOUPE_NATURAL_ZOOM_PRESETS[0];
}

export function computeLoupeFocalZoom(
    transform: LoupeTransform,
    viewport: LoupeViewport,
    image: LoupeImageSize,
    focalPoint: LoupePoint,
    factor: number,
    minScale = DEFAULT_MIN_SCALE,
    maxScale = DEFAULT_MAX_SCALE,
): LoupeTransform {
    const scale = clamp(transform.scale * factor, minScale, maxScale);
    if (transform.scale <= 0) return clampLoupePan({ ...transform, scale }, viewport, image);
    const ratio = scale / transform.scale;

    return clampLoupePan({
        scale,
        panX: focalPoint.x - (focalPoint.x - transform.panX) * ratio,
        panY: focalPoint.y - (focalPoint.y - transform.panY) * ratio,
    }, viewport, image);
}

export function clampLoupePan(transform: LoupeTransform, viewport: LoupeViewport, image: LoupeImageSize): LoupeTransform {
    const scale = fitScale(viewport, image);
    const renderedWidth = image.width * scale * transform.scale;
    const renderedHeight = image.height * scale * transform.scale;

    return {
        scale: transform.scale,
        panX: clampAxis(transform.panX, renderedWidth, viewport.width),
        panY: clampAxis(transform.panY, renderedHeight, viewport.height),
    };
}

export function computeLoupeSmartZoom(
    transform: LoupeTransform,
    viewport: LoupeViewport,
    image: LoupeImageSize,
    lastInspectionScale?: number,
): LoupeTransform {
    if (Math.abs(transform.scale - 1) <= FIT_EPSILON) {
        const scale = lastInspectionScale && lastInspectionScale > 1
            ? lastInspectionScale
            : computeLoupeActualSizeScale(viewport, image);
        return clampLoupePan({ scale, panX: 0, panY: 0 }, viewport, image);
    }

    return { scale: 1, panX: 0, panY: 0 };
}

function fitScale(viewport: LoupeViewport, image: LoupeImageSize): number {
    const scale = Math.min(1, viewport.width / image.width, viewport.height / image.height);
    return Number.isFinite(scale) && scale > 0 ? scale : 1;
}

function clampAxis(pan: number, rendered: number, viewport: number): number {
    if (rendered <= viewport) return 0;
    const limit = (rendered - viewport) / 2;
    return clamp(pan, -limit, limit);
}

function clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, value));
}
