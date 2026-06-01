import { describe, expect, it } from 'vitest';
import { histogramExposureWarnings } from './histogram-utils';
import type { ImageHistogram } from './api';

function histogram(luma: number[], pixelCount = 100): ImageHistogram {
    return {
        image_id: 'img-1',
        source: 'original',
        pixel_count: pixelCount,
        red: Array(256).fill(0),
        green: Array(256).fill(0),
        blue: Array(256).fill(0),
        luma,
    };
}

describe('histogram exposure warnings', () => {
    it('flags clipped shadows and highlights from real luma bins', () => {
        const luma = Array(256).fill(0);
        luma[0] = 3;
        luma[255] = 2;

        expect(histogramExposureWarnings(histogram(luma))).toEqual({
            clippedShadows: true,
            clippedHighlights: true,
        });
    });

    it('ignores small edge-bin counts below the threshold', () => {
        const luma = Array(256).fill(0);
        luma[0] = 1;
        luma[255] = 1;

        expect(histogramExposureWarnings(histogram(luma))).toEqual({
            clippedShadows: false,
            clippedHighlights: false,
        });
    });
});
