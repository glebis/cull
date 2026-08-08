import { describe, expect, it } from 'vitest';
import {
    THUMBNAIL_ZOOM_MAX,
    THUMBNAIL_ZOOM_MIN,
    thumbnailSizeFromZoomPosition,
    nudgeThumbnailSize,
    zoomPositionFromThumbnailSize,
} from './thumbnail-zoom';

describe('thumbnail zoom curve', () => {
    it('maps slider endpoints to the visible grid thumbnail bounds', () => {
        expect(thumbnailSizeFromZoomPosition(0)).toBe(THUMBNAIL_ZOOM_MIN);
        expect(thumbnailSizeFromZoomPosition(100)).toBe(THUMBNAIL_ZOOM_MAX);
    });

    it('changes size more precisely near both ends than in the middle', () => {
        const leftStep = thumbnailSizeFromZoomPosition(5) - thumbnailSizeFromZoomPosition(0);
        const middleStep = thumbnailSizeFromZoomPosition(55) - thumbnailSizeFromZoomPosition(50);
        const rightStep = thumbnailSizeFromZoomPosition(100) - thumbnailSizeFromZoomPosition(95);

        expect(leftStep).toBeGreaterThan(0);
        expect(rightStep).toBeGreaterThan(0);
        expect(middleStep).toBeGreaterThan(leftStep);
        expect(middleStep).toBeGreaterThan(rightStep);
    });

    it('round-trips existing thumbnail sizes back to slider positions', () => {
        for (const size of [4, 8, 32, 80, 160, 400, 800]) {
            const position = zoomPositionFromThumbnailSize(size);

            expect(thumbnailSizeFromZoomPosition(position)).toBeCloseTo(size, 0);
        }
    });

    it('nudges proportionally across the extended range without crossing its bounds', () => {
        expect(nudgeThumbnailSize(4, 1)).toBe(5);
        expect(nudgeThumbnailSize(5, -1)).toBe(4);
        expect(nudgeThumbnailSize(800, 1)).toBe(800);
        expect(nudgeThumbnailSize(4, -1)).toBe(4);
    });
});
