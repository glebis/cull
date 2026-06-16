export const THUMBNAIL_ZOOM_MIN = 80;
export const THUMBNAIL_ZOOM_MAX = 400;

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
