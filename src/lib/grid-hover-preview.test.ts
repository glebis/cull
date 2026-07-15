import { describe, expect, it } from 'vitest';
import { gridIndexAtPointer, planGridHoverPreview } from './grid-hover-preview';

describe('grid hover preview planning', () => {
    it('targets the exact image under the pointer at readable thumbnail sizes', () => {
        const plan = planGridHoverPreview({
            pointerX: 45,
            pointerY: 25,
            scrollTop: 100,
            cols: 10,
            cellSize: 40,
            thumbnailSize: 36,
            totalItems: 1_000,
        });

        expect(plan).toEqual({ mode: 'single', previewKey: 'image:31', anchorIndex: 31, indices: [31], groupCount: 1 });
    });

    it('previews a bounded sample from the pointer neighbourhood in pixel overview', () => {
        const plan = planGridHoverPreview({
            pointerX: 17,
            pointerY: 9,
            scrollTop: 0,
            cols: 100,
            cellSize: 4,
            thumbnailSize: 4,
            totalItems: 100_000,
        });

        expect(plan?.mode).toBe('group');
        expect(plan?.anchorIndex).toBe(204);
        expect(plan?.groupCount).toBe(64);
        expect(plan?.indices).toHaveLength(9);
        expect(plan?.previewKey).toBe('group:0:0:8');

        const sameGroup = planGridHoverPreview({
            pointerX: 29,
            pointerY: 25,
            scrollTop: 0,
            cols: 100,
            cellSize: 4,
            thumbnailSize: 4,
            totalItems: 100_000,
        });
        expect(sameGroup?.previewKey).toBe(plan?.previewKey);
        expect(sameGroup?.indices).toEqual(plan?.indices);
    });

    it('maps canvas pointers to scrolled grid indices and rejects empty space', () => {
        expect(gridIndexAtPointer(17, 7, 80, 25, 8, 1_000)).toBe(252);
        expect(gridIndexAtPointer(-1, 7, 80, 25, 8, 100)).toBeNull();
        expect(gridIndexAtPointer(200, 7, 80, 25, 8, 100)).toBeNull();
    });

});
