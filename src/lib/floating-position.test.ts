import { describe, expect, it } from 'vitest';
import { clampFloatingPosition } from './floating-position';

describe('floating menu positioning', () => {
    const viewport = { width: 800, height: 600 };
    const menu = { width: 240, height: 320 };

    it('keeps a menu at the requested point when it fits', () => {
        expect(clampFloatingPosition({ x: 120, y: 140 }, menu, viewport)).toEqual({ x: 120, y: 140 });
    });

    it('moves a menu left when it would overflow the right edge', () => {
        expect(clampFloatingPosition({ x: 700, y: 140 }, menu, viewport)).toEqual({ x: 552, y: 140 });
    });

    it('moves a menu up when it would overflow the bottom edge', () => {
        expect(clampFloatingPosition({ x: 120, y: 500 }, menu, viewport)).toEqual({ x: 120, y: 272 });
    });

    it('keeps oversized menus anchored to the viewport margin', () => {
        expect(clampFloatingPosition(
            { x: -20, y: -30 },
            { width: 1200, height: 900 },
            viewport,
        )).toEqual({ x: 8, y: 8 });
    });
});
