import { describe, expect, it } from 'vitest';
import { clampFloatingPosition, placeAdjacentSubmenu } from './floating-position';

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

describe('adjacent submenu positioning', () => {
    const viewport = { width: 800, height: 600 };
    const submenu = { width: 180, height: 220 };

    it('opens a submenu to the right when it fits', () => {
        expect(placeAdjacentSubmenu(
            { x: 240, y: 120, width: 200, height: 30 },
            submenu,
            viewport,
            460,
        )).toMatchObject({ left: 199, top: -4 });
    });

    it('flips a submenu left near the right viewport edge', () => {
        expect(placeAdjacentSubmenu(
            { x: 590, y: 120, width: 200, height: 30 },
            submenu,
            viewport,
            460,
        )).toMatchObject({ left: -179, top: -4 });
    });

    it('keeps a tall submenu inside the lower viewport edge', () => {
        expect(placeAdjacentSubmenu(
            { x: 240, y: 520, width: 200, height: 30 },
            { width: 180, height: 220 },
            viewport,
            460,
        )).toMatchObject({ left: 199, top: -186 });
    });

    it('clamps horizontally to the right edge when neither side can fully fit and right has more room', () => {
        expect(placeAdjacentSubmenu(
            { x: 90, y: 120, width: 200, height: 30 },
            { width: 760, height: 220 },
            viewport,
            460,
        )).toMatchObject({ left: -58, top: -4 });
    });

    it('clamps horizontally to the left edge when neither side can fully fit and left has more room', () => {
        expect(placeAdjacentSubmenu(
            { x: 520, y: 120, width: 200, height: 30 },
            { width: 760, height: 220 },
            viewport,
            460,
        )).toMatchObject({ left: -512, top: -4 });
    });
});
