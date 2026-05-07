export function clampFocusIndex(current: number, delta: number, total: number): number {
    if (total === 0) return 0;
    let next = current + delta;
    if (next < 0) next = 0;
    if (next >= total) next = total - 1;
    return next;
}

export function clampThumbnailSize(current: number, delta: number, min = 80, max = 400): number {
    const next = current + delta;
    if (next < min) return min;
    if (next > max) return max;
    return next;
}

export function nextComparePairIndex(current: number, total: number): number {
    return Math.min(current + 2, Math.max(0, total - 2));
}

export function prevComparePairIndex(current: number): number {
    return Math.max(0, current - 2);
}

export function computeColumnCount(containerWidth: number, thumbSize: number, gap: number): number {
    return Math.max(1, Math.floor((containerWidth + gap) / (thumbSize + gap)));
}
