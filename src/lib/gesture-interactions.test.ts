import { describe, expect, it } from 'vitest';
import {
    classifySwipe,
    normalizeWheelDelta,
    shouldIgnoreGestureTarget,
    wheelGestureIntent,
    wheelZoomFactor,
} from './gesture-interactions';

describe('gesture interactions', () => {
    it('normalizes wheel delta modes to pixels', () => {
        expect(normalizeWheelDelta({ deltaX: 2, deltaY: 3, deltaMode: 0 }, 800)).toEqual({ deltaX: 2, deltaY: 3 });
        expect(normalizeWheelDelta({ deltaX: 2, deltaY: 3, deltaMode: 1 }, 800)).toEqual({ deltaX: 32, deltaY: 48 });
        expect(normalizeWheelDelta({ deltaX: 0.5, deltaY: 1, deltaMode: 2 }, 800)).toEqual({ deltaX: 400, deltaY: 800 });
    });

    it('does not route unmodified wheel input to zoom', () => {
        const intent = wheelGestureIntent({
            surface: 'loupe',
            deltaX: 0,
            deltaY: -120,
            deltaMode: 0,
            clientX: 50,
            clientY: 60,
            ctrlKey: false,
            metaKey: false,
            altKey: false,
            shiftKey: false,
            viewportHeight: 800,
            target: null,
        });

        expect(intent).toEqual({ type: 'pan', deltaX: 0, deltaY: -120, source: 'wheel' });
    });

    it('routes modifier wheel input to zoom around the pointer', () => {
        const intent = wheelGestureIntent({
            surface: 'loupe',
            deltaX: 0,
            deltaY: -120,
            deltaMode: 0,
            clientX: 50,
            clientY: 60,
            ctrlKey: true,
            metaKey: false,
            altKey: false,
            shiftKey: false,
            viewportHeight: 800,
            target: null,
        });

        expect(intent).toEqual({
            type: 'zoom',
            factor: wheelZoomFactor(-120),
            focalX: 50,
            focalY: 60,
            source: 'wheel',
        });
    });

    it('classifies dominant horizontal swipes only after threshold', () => {
        expect(classifySwipe({ deltaX: 79, deltaY: 0 })).toBeNull();
        expect(classifySwipe({ deltaX: 100, deltaY: 80 })).toBeNull();
        expect(classifySwipe({ deltaX: 100, deltaY: 20 })).toBe('previous');
        expect(classifySwipe({ deltaX: -100, deltaY: 20 })).toBe('next');
    });

    it('suppresses gestures from editable and modal targets', () => {
        const input = target({ tagName: 'input' });
        expect(shouldIgnoreGestureTarget(input)).toBe(true);

        const editable = target({ tagName: 'div', isContentEditable: true });
        expect(shouldIgnoreGestureTarget(editable)).toBe(true);

        const modalChild = target({ tagName: 'button', closestResult: {} });
        expect(shouldIgnoreGestureTarget(modalChild)).toBe(true);

        const normal = target({ tagName: 'div' });
        expect(shouldIgnoreGestureTarget(normal)).toBe(false);
        expect(shouldIgnoreGestureTarget(normal, { modalOpen: true })).toBe(true);
    });
});

function target(opts: { tagName: string; isContentEditable?: boolean; closestResult?: unknown }): EventTarget {
    return {
        tagName: opts.tagName,
        isContentEditable: opts.isContentEditable ?? false,
        closest: () => opts.closestResult ?? null,
    } as unknown as EventTarget;
}
