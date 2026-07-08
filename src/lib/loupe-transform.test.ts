import { describe, expect, it } from 'vitest';
import {
    clampLoupePan,
    computeLoupeActualSizeScale,
    computeLoupeFocalZoom,
    computeLoupeNaturalScale,
    computeLoupeViewportScaleForNaturalScale,
    computeLoupeSmartZoom,
    nextLoupeNaturalZoomPreset,
} from './loupe-transform';

describe('loupe transform helpers', () => {
    const viewport = { width: 1000, height: 800 };
    const image = { width: 2000, height: 1600 };

    it('computes actual size as 100% pixels when fit view downscales the image', () => {
        expect(computeLoupeActualSizeScale(viewport, image)).toBeCloseTo(2);
    });

    it('does not shrink images that already render at natural size', () => {
        expect(computeLoupeActualSizeScale(viewport, { width: 500, height: 400 })).toBe(1);
    });

    it('reports zoom relative to natural image pixels', () => {
        expect(computeLoupeNaturalScale(viewport, image, 1)).toBeCloseTo(0.5);
        expect(computeLoupeNaturalScale(viewport, image, 2)).toBeCloseTo(1);
    });

    it('converts natural pixel zoom back to the loupe viewport scale', () => {
        expect(computeLoupeViewportScaleForNaturalScale(viewport, image, 1)).toBeCloseTo(2);
        expect(computeLoupeViewportScaleForNaturalScale(viewport, image, 0.5)).toBeCloseTo(1);
    });

    it('steps through standard natural pixel zoom levels', () => {
        expect(nextLoupeNaturalZoomPreset(1, 1)).toBe(1.25);
        expect(nextLoupeNaturalZoomPreset(1.25, 1)).toBe(1.5);
        expect(nextLoupeNaturalZoomPreset(1.56, 1)).toBe(2);
        expect(nextLoupeNaturalZoomPreset(1.56, -1)).toBe(1.5);
    });

    it('preserves focal point when zooming', () => {
        const next = computeLoupeFocalZoom(
            { scale: 1, panX: 0, panY: 0 },
            viewport,
            image,
            { x: 600, y: 400 },
            2,
        );

        expect(next.scale).toBe(2);
        expect(next.panX).toBeLessThan(0);
    });

    it('clamps pan to zero when image fits viewport', () => {
        expect(clampLoupePan({ scale: 1, panX: 200, panY: -200 }, viewport, { width: 500, height: 400 })).toEqual({
            scale: 1,
            panX: 0,
            panY: 0,
        });
    });

    it('smart zoom toggles from fit to actual size and back', () => {
        const actual = computeLoupeSmartZoom({ scale: 1, panX: 0, panY: 0 }, viewport, image);
        expect(actual.scale).toBeCloseTo(2);

        const fit = computeLoupeSmartZoom(actual, viewport, image);
        expect(fit).toEqual({ scale: 1, panX: 0, panY: 0 });
    });
});
