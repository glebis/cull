/**
 * Viewport culling and render cap for the freeform Canvas view.
 *
 * Canvas items live in canvas-space (`x,y,width,height` px) inside a layer transformed by
 * `translate(panX,panY) scale(zoom)`. Rendering every item as a DOM node is unusable at
 * thousands; these pure helpers let the component render only items whose transformed
 * bounding box intersects the viewport (plus a margin), capped to a hard node budget.
 */

export interface CanvasViewportRect {
    panX: number;
    panY: number;
    zoom: number;
    width: number;
    height: number;
}

export interface CanvasCullableItem {
    x: number;
    y: number;
    width: number;
    height: number;
    rotationDegrees?: number;
}

export interface CanvasCullOptions {
    /** Expand the viewport by this many screen px so panning feels instant. */
    margin?: number;
}

/**
 * Keep only items whose screen-space (rotated) bounding box intersects the viewport
 * expanded by `margin`. Order is preserved. A degenerate viewport (no size / non-positive
 * zoom) disables culling so we never blank the canvas on a bad measurement.
 */
export function computeVisibleCanvasItems<T extends CanvasCullableItem>(
    items: T[],
    viewport: CanvasViewportRect,
    opts: CanvasCullOptions = {}
): T[] {
    const { panX, panY, zoom, width, height } = viewport;
    if (!(zoom > 0) || width <= 0 || height <= 0) return items.slice();

    const margin = Math.max(0, opts.margin ?? 0);
    const minX = -margin;
    const minY = -margin;
    const maxX = width + margin;
    const maxY = height + margin;

    return items.filter((it) => {
        const rot = ((it.rotationDegrees ?? 0) * Math.PI) / 180;
        const cos = Math.abs(Math.cos(rot));
        const sin = Math.abs(Math.sin(rot));
        const halfW = (it.width * zoom) / 2;
        const halfH = (it.height * zoom) / 2;
        // Rotated axis-aligned bounding-box half-extents.
        const extX = halfW * cos + halfH * sin;
        const extY = halfW * sin + halfH * cos;
        // Screen-space centre.
        const cx = (it.x + it.width / 2) * zoom + panX;
        const cy = (it.y + it.height / 2) * zoom + panY;
        const left = cx - extX;
        const right = cx + extX;
        const top = cy - extY;
        const bottom = cy + extY;
        return right >= minX && left <= maxX && bottom >= minY && top <= maxY;
    });
}

export interface CanvasCapResult<T> {
    rendered: T[];
    droppedCount: number;
}

/** Hard cap on simultaneously-rendered canvas DOM nodes. */
export const CANVAS_RENDER_CAP = 1500;

/**
 * Limit the rendered item count to `max`, reporting how many were dropped so the UI can
 * surface a "showing N of M" hint.
 */
export function capCanvasItems<T>(items: T[], max: number = CANVAS_RENDER_CAP): CanvasCapResult<T> {
    const cap = Math.max(0, Math.trunc(max));
    if (items.length <= cap) return { rendered: items, droppedCount: 0 };
    return { rendered: items.slice(0, cap), droppedCount: items.length - cap };
}
