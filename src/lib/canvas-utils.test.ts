import { describe, it, expect } from 'vitest';
import {
    computeVisibleCanvasItems,
    capCanvasItems,
    CANVAS_RENDER_CAP,
    type CanvasViewportRect,
} from './canvas-utils';

const VP: CanvasViewportRect = { panX: 0, panY: 0, zoom: 1, width: 1000, height: 800 };

function item(x: number, y: number, width = 50, height = 50, rotationDegrees = 0) {
    return { id: `${x},${y}`, x, y, width, height, rotationDegrees };
}

describe('computeVisibleCanvasItems', () => {
    it('keeps an item fully inside the viewport', () => {
        const items = [item(100, 100)];
        expect(computeVisibleCanvasItems(items, VP)).toEqual(items);
    });

    it('drops an item fully outside the viewport', () => {
        const items = [item(2000, 100)];
        expect(computeVisibleCanvasItems(items, VP)).toEqual([]);
    });

    it('keeps an item straddling the left edge', () => {
        const items = [item(-25, 100)]; // screen x -25..25 intersects [0,1000]
        expect(computeVisibleCanvasItems(items, VP)).toEqual(items);
    });

    it('respects panning: a far item panned into view is kept', () => {
        const items = [item(2000, 100)];
        const panned: CanvasViewportRect = { ...VP, panX: -1950 }; // screen x 50..100
        expect(computeVisibleCanvasItems(items, panned)).toEqual(items);
    });

    it('respects zoom: an item pushed out by zoom is dropped', () => {
        const items = [item(900, 100)]; // at zoom 2 -> screen left 1800 > 1000
        const zoomed: CanvasViewportRect = { ...VP, zoom: 2 };
        expect(computeVisibleCanvasItems(items, zoomed)).toEqual([]);
    });

    it('uses the rotated AABB so a rotated item near an edge is not clipped', () => {
        const flat = item(100, -60, 100, 20, 0); // axis-aligned bbox is above the viewport
        const rotated = item(100, -60, 100, 20, 90); // rotated tall, pokes into the viewport
        expect(computeVisibleCanvasItems([flat], VP)).toEqual([]);
        expect(computeVisibleCanvasItems([rotated], VP)).toEqual([rotated]);
    });

    it('expands the viewport by the margin', () => {
        const items = [item(1010, 100, 50, 50)]; // screen left 1010, just outside
        expect(computeVisibleCanvasItems(items, VP)).toEqual([]);
        expect(computeVisibleCanvasItems(items, VP, { margin: 100 })).toEqual(items);
    });

    it('preserves order of kept items', () => {
        const items = [item(0, 0), item(2000, 0), item(200, 200)];
        expect(computeVisibleCanvasItems(items, VP).map((i) => i.id)).toEqual(['0,0', '200,200']);
    });
});

describe('capCanvasItems', () => {
    const items = Array.from({ length: 10 }, (_, i) => ({ id: i }));

    it('renders everything when under the cap', () => {
        const res = capCanvasItems(items, 20);
        expect(res.rendered).toHaveLength(10);
        expect(res.droppedCount).toBe(0);
    });

    it('caps and reports the dropped count when over the cap', () => {
        const res = capCanvasItems(items, 4);
        expect(res.rendered).toHaveLength(4);
        expect(res.rendered.map((i) => i.id)).toEqual([0, 1, 2, 3]);
        expect(res.droppedCount).toBe(6);
    });

    it('defaults to CANVAS_RENDER_CAP', () => {
        expect(CANVAS_RENDER_CAP).toBeGreaterThan(0);
        const many = Array.from({ length: CANVAS_RENDER_CAP + 5 }, (_, i) => ({ id: i }));
        const res = capCanvasItems(many);
        expect(res.rendered).toHaveLength(CANVAS_RENDER_CAP);
        expect(res.droppedCount).toBe(5);
    });
});
