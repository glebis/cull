import { describe, expect, it } from 'vitest';
import { shouldDecodeGridOverviewThumbnails } from './grid-overview';

describe('grid overview rendering policy', () => {
    it('does not bulk-decode thumbnails at full-scope pixel density', () => {
        expect(shouldDecodeGridOverviewThumbnails(4)).toBe(false);
        expect(shouldDecodeGridOverviewThumbnails(8)).toBe(false);
        expect(shouldDecodeGridOverviewThumbnails(12)).toBe(false);
        expect(shouldDecodeGridOverviewThumbnails(13)).toBe(true);
    });
});
