import { describe, expect, it } from 'vitest';
import {
    GRID_GESTURE_ZOOM_MAX,
    GRID_GESTURE_ZOOM_MIN,
    gridGestureZoom,
} from './grid-gesture-zoom';

describe('grid gesture zoom', () => {
    it('keeps gesture zoom size and preset state coherent', () => {
        const presets = [
            { name: 'compact', size: 80, gap: 2 },
            { name: 'normal', size: 160, gap: 4 },
            { name: 'large', size: 280, gap: 8 },
        ];

        const next = gridGestureZoom({ size: 160, gap: 4, preset: 1 }, 1.8, presets);

        expect(next.size).toBe(288);
        expect(next.preset).toBe(2);
        expect(next.gap).toBe(8);
    });

    it('clamps gesture zoom to grid thumbnail bounds', () => {
        const presets = [
            { name: 'compact', size: 80, gap: 2 },
            { name: 'xl', size: 400, gap: 12 },
        ];

        expect(gridGestureZoom({ size: 80, gap: 2, preset: 0 }, 0.1, presets).size).toBe(GRID_GESTURE_ZOOM_MIN);
        expect(gridGestureZoom({ size: 400, gap: 12, preset: 1 }, 10, presets).size).toBe(GRID_GESTURE_ZOOM_MAX);
    });
});
