export const THUMBNAIL_ZOOM_MIN = 4;
export const THUMBNAIL_ZOOM_MAX = 800;

function clamp(value: number, min: number, max: number): number {
    return Math.min(Math.max(value, min), max);
}

function smoothstep(t: number): number {
    return t * t * (3 - 2 * t);
}

export function thumbnailSizeFromZoomPosition(position: number): number {
    const t = clamp(position, 0, 100) / 100;
    return Math.round(THUMBNAIL_ZOOM_MIN + smoothstep(t) * (THUMBNAIL_ZOOM_MAX - THUMBNAIL_ZOOM_MIN));
}

export function zoomPositionFromThumbnailSize(size: number): number {
    const target = (clamp(size, THUMBNAIL_ZOOM_MIN, THUMBNAIL_ZOOM_MAX) - THUMBNAIL_ZOOM_MIN) /
        (THUMBNAIL_ZOOM_MAX - THUMBNAIL_ZOOM_MIN);
    let lo = 0;
    let hi = 1;

    for (let i = 0; i < 20; i += 1) {
        const mid = (lo + hi) / 2;
        if (smoothstep(mid) < target) lo = mid;
        else hi = mid;
    }

    return ((lo + hi) / 2) * 100;
}

export function nudgeThumbnailSize(size: number, direction: -1 | 1): number {
    const factor = direction > 0 ? 1.25 : 0.8;
    const next = Math.round(clamp(size * factor, THUMBNAIL_ZOOM_MIN, THUMBNAIL_ZOOM_MAX));
    if (next === size && size > THUMBNAIL_ZOOM_MIN && direction < 0) return size - 1;
    if (next === size && size < THUMBNAIL_ZOOM_MAX && direction > 0) return size + 1;
    return next;
}
