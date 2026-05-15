export interface FloatingPoint {
    x: number;
    y: number;
}

export interface FloatingSize {
    width: number;
    height: number;
}

export interface ViewportSize {
    width: number;
    height: number;
}

function clampAxis(anchor: number, size: number, viewportSize: number, margin: number) {
    const max = viewportSize - size - margin;
    if (max < margin) return margin;
    return Math.min(Math.max(anchor, margin), max);
}

export function clampFloatingPosition(
    anchor: FloatingPoint,
    size: FloatingSize,
    viewport: ViewportSize,
    margin = 8,
): FloatingPoint {
    return {
        x: clampAxis(anchor.x, size.width, viewport.width, margin),
        y: clampAxis(anchor.y, size.height, viewport.height, margin),
    };
}
