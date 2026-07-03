import { describe, expect, it } from 'vitest';
import {
    canvasZoomFromPosition,
    canvasZoomPositionFromZoom,
    computeCanvasItemDragPosition,
    computeCanvasResize,
    computeCanvasWheelZoom,
    computeCanvasZoomAtPoint,
    computeCanvasZoomToLevel,
    isCanvasSpacePanKey,
    worldToCanvasScreen,
} from './canvas-interactions';

describe('canvas interactions', () => {
    it('keeps the world point under the cursor stable while wheel zooming', () => {
        const pointer = { x: 250, y: 180 };
        const viewport = { panX: 50, panY: -20, zoom: 2 };
        const worldPoint = {
            x: (pointer.x - viewport.panX) / viewport.zoom,
            y: (pointer.y - viewport.panY) / viewport.zoom,
        };

        const next = computeCanvasWheelZoom(viewport, pointer, -100);
        const screenPoint = worldToCanvasScreen(worldPoint, next);

        expect(next.zoom).toBeGreaterThan(viewport.zoom);
        expect(screenPoint.x).toBeCloseTo(pointer.x, 5);
        expect(screenPoint.y).toBeCloseTo(pointer.y, 5);
    });

    it('zooms canvas around a pointer using a factor', () => {
        const next = computeCanvasZoomAtPoint(
            { panX: 10, panY: 20, zoom: 1 },
            { x: 100, y: 80 },
            2,
        );

        expect(next).toEqual({ panX: -80, panY: -40, zoom: 2 });
    });

    it('zooms canvas to an explicit level around a pointer', () => {
        const next = computeCanvasZoomToLevel(
            { panX: 10, panY: 20, zoom: 1 },
            { x: 100, y: 80 },
            2,
        );

        expect(next).toEqual({ panX: -80, panY: -40, zoom: 2 });
    });

    it('maps the canvas zoom slider across the allowed zoom range', () => {
        expect(canvasZoomFromPosition(0)).toBeCloseTo(0.1, 5);
        expect(canvasZoomFromPosition(100)).toBeCloseTo(5, 5);

        const position = canvasZoomPositionFromZoom(2);
        expect(canvasZoomFromPosition(position)).toBeCloseTo(2, 5);
    });

    it('maps dragged item coordinates through pan and zoom', () => {
        const next = computeCanvasItemDragPosition(
            { x: 340, y: 220 },
            { panX: 40, panY: 20, zoom: 2 },
            { x: 15, y: 25 },
        );

        expect(next).toEqual({ x: 135, y: 75 });
    });

    it('preserves image aspect while resizing under zoom', () => {
        const next = computeCanvasResize({
            startClientX: 100,
            currentClientX: 180,
            startWidth: 120,
            startHeight: 60,
            imageWidth: 400,
            imageHeight: 200,
            zoom: 2,
            minWidth: 50,
        });

        expect(next).toEqual({ width: 160, height: 80 });
    });

    it('only enables Space-pan for plain Space outside editable targets', () => {
        expect(isCanvasSpacePanKey({ key: ' ', code: 'Space', targetTagName: 'DIV' })).toBe(true);
        expect(isCanvasSpacePanKey({ key: 'Spacebar', code: 'Space', targetTagName: 'DIV' })).toBe(true);
        expect(isCanvasSpacePanKey({ key: ' ', code: 'Space', targetTagName: 'INPUT' })).toBe(false);
        expect(isCanvasSpacePanKey({ key: ' ', code: 'Space', isContentEditable: true })).toBe(false);
        expect(isCanvasSpacePanKey({ key: ' ', code: 'Space', ctrlKey: true, targetTagName: 'DIV' })).toBe(false);
    });
});
