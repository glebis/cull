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

export function computeLoupeActualSizeScale(viewport: LoupeViewport, image: LoupeImageSize): number {
    const scale = fitScale(viewport, image);
    if (!Number.isFinite(scale) || scale <= 0) return 1;
    return clamp(Math.max(1, 1 / scale), DEFAULT_MIN_SCALE, DEFAULT_MAX_SCALE);
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
    const scale = Math.min(viewport.width / image.width, viewport.height / image.height);
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
