export const THUMBNAIL_ZOOM_MIN = 4;
export const THUMBNAIL_ZOOM_MAX = 800;

function clamp(value: number, min: number, max: number): number {
    return Math.min(Math.max(value, min), max);
}

function smoothstep(t: number): number {
    return t * t * (3 - 2 * t);
}

export function thumbnailSizeFromZoomPosition(
    position: number,
    min = THUMBNAIL_ZOOM_MIN,
    max = THUMBNAIL_ZOOM_MAX,
): number {
    const lower = Math.min(min, max);
    const upper = Math.max(min, max);
    const t = clamp(position, 0, 100) / 100;
    return Math.round(lower + smoothstep(t) * (upper - lower));
}

export function zoomPositionFromThumbnailSize(
    size: number,
    min = THUMBNAIL_ZOOM_MIN,
    max = THUMBNAIL_ZOOM_MAX,
): number {
    const lower = Math.min(min, max);
    const upper = Math.max(min, max);
    if (lower === upper) return 0;
    const target = (clamp(size, lower, upper) - lower) / (upper - lower);
    let lo = 0;
    let hi = 1;

    for (let i = 0; i < 20; i += 1) {
        const mid = (lo + hi) / 2;
        if (smoothstep(mid) < target) lo = mid;
        else hi = mid;
    }

    return ((lo + hi) / 2) * 100;
}

export function nudgeThumbnailSize(
    size: number,
    direction: -1 | 1,
    min = THUMBNAIL_ZOOM_MIN,
    max = THUMBNAIL_ZOOM_MAX,
): number {
    const lower = Math.min(min, max);
    const upper = Math.max(min, max);
    const factor = direction > 0 ? 1.25 : 0.8;
    const boundedSize = clamp(size, lower, upper);
    const next = Math.round(clamp(boundedSize * factor, lower, upper));
    if (next === boundedSize && boundedSize > lower && direction < 0) return boundedSize - 1;
    if (next === boundedSize && boundedSize < upper && direction > 0) return boundedSize + 1;
    return next;
}
