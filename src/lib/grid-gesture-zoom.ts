export const GRID_GESTURE_ZOOM_MIN = 80;
export const GRID_GESTURE_ZOOM_MAX = 400;

export interface GridGestureZoomPreset {
    name: string;
    size: number;
    gap: number;
}

export interface GridGestureZoomState {
    size: number;
    gap: number;
    preset: number;
}

function clamp(value: number, min: number, max: number): number {
    return Math.min(Math.max(value, min), max);
}

export function gridGestureZoom(
    current: GridGestureZoomState,
    factor: number,
    presets: readonly GridGestureZoomPreset[],
): GridGestureZoomState {
    const size = Math.round(clamp(current.size * factor, GRID_GESTURE_ZOOM_MIN, GRID_GESTURE_ZOOM_MAX));
    const preset = closestPresetIndex(size, presets, current.preset);
    return {
        size,
        preset,
        gap: presets[preset]?.gap ?? current.gap,
    };
}

function closestPresetIndex(size: number, presets: readonly GridGestureZoomPreset[], fallback: number): number {
    if (presets.length === 0) return fallback;
    let closest = 0;
    let closestDistance = Number.POSITIVE_INFINITY;
    for (let i = 0; i < presets.length; i += 1) {
        const distance = Math.abs(presets[i].size - size);
        if (distance < closestDistance) {
            closest = i;
            closestDistance = distance;
        }
    }
    return closest;
}
