import type { ImageHistogram } from './api';

export interface HistogramExposureWarnings {
    clippedShadows: boolean;
    clippedHighlights: boolean;
}

export function histogramExposureWarnings(
    histogram: Pick<ImageHistogram, 'luma' | 'pixel_count'>,
    threshold = 0.02
): HistogramExposureWarnings {
    const pixelCount = Math.max(histogram.pixel_count, 1);
    return {
        clippedShadows: ((histogram.luma[0] ?? 0) / pixelCount) >= threshold,
        clippedHighlights: ((histogram.luma[255] ?? 0) / pixelCount) >= threshold,
    };
}

export function histogramPolyline(bins: number[], height: number, width = 255): string {
    if (bins.length === 0 || height <= 0 || width <= 0) return '';
    const max = Math.max(...bins, 1);
    const xDenominator = Math.max(bins.length - 1, 1);

    return bins
        .map((value, index) => {
            const x = Math.round((index / xDenominator) * width);
            const y = height - Math.round((value / max) * height);
            return `${x},${y}`;
        })
        .join(' ');
}
