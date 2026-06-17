# Gesture Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Apple-style gesture support for Cull's Grid, Loupe, Canvas, and Compare views while preserving explicit controls and avoiding macOS system gesture conflicts.

**Architecture:** Build pure gesture and transform helpers first, then wire them into Svelte components as semantic intents. Keep unmodified wheel/scroll as scroll or pan, reserve zoom for pinch/native magnify/modifier-wheel, and defer native AppKit bridging until real hardware testing proves it is needed.

**Tech Stack:** Svelte 5 runes, TypeScript, Vitest, Tauri 2 WebView events, existing Cull stores and component event handlers.

## Global Constraints

- Trackpad is primary; Magic Mouse, mouse wheel, pointer drag, and touch-like input get equivalent behaviour where available.
- Gestures are accelerators, not exclusive controls.
- Do not apply crops with a gesture alone.
- Do not make three- or four-finger gestures required.
- Do not make unmodified wheel events mean zoom.
- Do not treat ordinary mouse double-click as Apple smart zoom.
- Do not add a native macOS bridge in the first implementation.
- Disable gesture shortcuts while text inputs, dialogs, command palette, context menus, or modal layers own focus.
- Keep all UI colors and styling within existing Cull tokens if any visible UI is added.

---

## File Structure

- Create `src/lib/gesture-interactions.ts`: pure wheel normalization, gesture suppression, swipe classification, semantic intent types, and surface routing helpers.
- Create `src/lib/gesture-interactions.test.ts`: Vitest coverage for thresholds, wheel delta modes, suppression, crop policy, and source routing.
- Create `src/lib/loupe-transform.ts`: pure Loupe zoom/pan/smart-zoom helpers.
- Create `src/lib/loupe-transform.test.ts`: Vitest coverage for focal zoom, actual-size scale, pan clamp, and smart zoom.
- Modify `src/lib/canvas-interactions.ts`: add factor-based canvas zoom helper while preserving current wheel helper compatibility.
- Modify `src/lib/canvas-interactions.test.ts`: coverage for factor-based zoom.
- Modify `src/lib/components/Grid.svelte`: route modifier-wheel/native gesture zoom into a thumbnail zoom command that preserves focus and preset consistency.
- Modify `src/lib/components/Loupe.svelte`: route wheel/pan/swipe/zoom through helpers, preserve crop safety, and keep double-click separate from smart zoom.
- Modify `src/lib/components/Canvas.svelte`: route pan/zoom through helpers and debounce viewport persistence.
- Modify `src/lib/components/Compare.svelte`: add navigation gesture routing only.
- Create `src/lib/compare-gestures.ts`: pure Compare swipe navigation helper.
- Create `src/lib/compare-gestures.test.ts`: focused tests for adjacent-pair navigation.

## Task 1: Gesture Interpreter Utilities

**Files:**
- Create: `src/lib/gesture-interactions.ts`
- Create: `src/lib/gesture-interactions.test.ts`

**Interfaces:**
- Produces:
  - `normalizeWheelDelta(eventLike, viewportHeight): { deltaX: number; deltaY: number }`
  - `wheelZoomFactor(deltaY: number): number`
  - `classifySwipe(input: SwipeInput, options?: SwipeOptions): 'previous' | 'next' | null`
  - `shouldIgnoreGestureTarget(target: EventTarget | null, state?: GestureSuppressionState): boolean`
  - `wheelGestureIntent(input: WheelGestureInput): GestureIntent | null`
  - Types `GestureIntent`, `GestureSource`, `GestureSurface`, `GestureSuppressionState`
- Consumes: no app stores; pure TypeScript only.

- [ ] **Step 1: Write failing utility tests**

Add `src/lib/gesture-interactions.test.ts`:

```ts
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
        const input = document.createElement('input');
        expect(shouldIgnoreGestureTarget(input)).toBe(true);

        const editable = document.createElement('div');
        editable.contentEditable = 'true';
        expect(shouldIgnoreGestureTarget(editable)).toBe(true);

        const modalChild = document.createElement('button');
        const modal = document.createElement('div');
        modal.setAttribute('role', 'dialog');
        modal.appendChild(modalChild);
        expect(shouldIgnoreGestureTarget(modalChild)).toBe(true);

        const normal = document.createElement('div');
        expect(shouldIgnoreGestureTarget(normal)).toBe(false);
        expect(shouldIgnoreGestureTarget(normal, { modalOpen: true })).toBe(true);
    });
});
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
npm test -- src/lib/gesture-interactions.test.ts
```

Expected: FAIL because `src/lib/gesture-interactions.ts` does not exist.

- [ ] **Step 3: Implement utility module**

Create `src/lib/gesture-interactions.ts`:

```ts
export type GestureSurface = 'grid' | 'loupe' | 'compare' | 'canvas';
export type GestureSource = 'trackpad' | 'magic_mouse' | 'wheel' | 'pointer' | 'touch' | 'native_macos';

export type GestureIntent =
    | { type: 'zoom'; factor: number; focalX: number; focalY: number; source: GestureSource }
    | { type: 'pan'; deltaX: number; deltaY: number; source: GestureSource }
    | { type: 'navigate'; direction: 'previous' | 'next'; source: GestureSource }
    | { type: 'smart_zoom'; focalX: number; focalY: number; source: GestureSource }
    | { type: 'crop_adjust'; deltaX: number; deltaY: number; source: GestureSource };

export interface GestureSuppressionState {
    modalOpen?: boolean;
    contextMenuOpen?: boolean;
    commandPaletteOpen?: boolean;
    textEntryOpen?: boolean;
    cropModeActive?: boolean;
}

export interface WheelLikeInput {
    deltaX: number;
    deltaY: number;
    deltaMode: number;
}

export interface SwipeInput {
    deltaX: number;
    deltaY: number;
}

export interface SwipeOptions {
    minDistance?: number;
    dominanceRatio?: number;
}

export interface WheelGestureInput extends WheelLikeInput {
    surface: GestureSurface;
    clientX: number;
    clientY: number;
    ctrlKey?: boolean;
    metaKey?: boolean;
    altKey?: boolean;
    shiftKey?: boolean;
    viewportHeight: number;
    target: EventTarget | null;
    suppression?: GestureSuppressionState;
}

const LINE_DELTA_PX = 16;
const DEFAULT_SWIPE_DISTANCE = 80;
const DEFAULT_SWIPE_DOMINANCE = 1.5;
const WHEEL_ZOOM_BASE = 1.0015;

export function normalizeWheelDelta(input: WheelLikeInput, viewportHeight: number): { deltaX: number; deltaY: number } {
    const multiplier = input.deltaMode === 1
        ? LINE_DELTA_PX
        : input.deltaMode === 2
            ? viewportHeight
            : 1;
    return {
        deltaX: input.deltaX * multiplier,
        deltaY: input.deltaY * multiplier,
    };
}

export function wheelZoomFactor(deltaY: number): number {
    return Math.pow(WHEEL_ZOOM_BASE, -deltaY);
}

export function classifySwipe(input: SwipeInput, options: SwipeOptions = {}): 'previous' | 'next' | null {
    const minDistance = options.minDistance ?? DEFAULT_SWIPE_DISTANCE;
    const dominanceRatio = options.dominanceRatio ?? DEFAULT_SWIPE_DOMINANCE;
    const absX = Math.abs(input.deltaX);
    const absY = Math.abs(input.deltaY);
    if (absX < minDistance) return null;
    if (absX < absY * dominanceRatio) return null;
    return input.deltaX > 0 ? 'previous' : 'next';
}

export function shouldIgnoreGestureTarget(target: EventTarget | null, state: GestureSuppressionState = {}): boolean {
    if (state.modalOpen || state.contextMenuOpen || state.commandPaletteOpen || state.textEntryOpen) return true;
    if (!(target instanceof HTMLElement)) return false;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
    if (target.isContentEditable) return true;
    return target.closest('[role="dialog"], .modal-dialog, .command-palette, .context-menu') !== null;
}

export function wheelGestureIntent(input: WheelGestureInput): GestureIntent | null {
    if (shouldIgnoreGestureTarget(input.target, input.suppression)) return null;
    const delta = normalizeWheelDelta(input, input.viewportHeight);
    if (input.ctrlKey || input.metaKey || input.altKey) {
        return {
            type: 'zoom',
            factor: wheelZoomFactor(delta.deltaY),
            focalX: input.clientX,
            focalY: input.clientY,
            source: 'wheel',
        };
    }
    return {
        type: 'pan',
        deltaX: delta.deltaX,
        deltaY: delta.deltaY,
        source: 'wheel',
    };
}
```

- [ ] **Step 4: Run tests and verify pass**

Run:

```bash
npm test -- src/lib/gesture-interactions.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/gesture-interactions.ts src/lib/gesture-interactions.test.ts
git commit -m "feat: add gesture intent utilities"
```

## Task 2: Loupe Transform Helpers

**Files:**
- Create: `src/lib/loupe-transform.ts`
- Create: `src/lib/loupe-transform.test.ts`

**Interfaces:**
- Consumes: no Svelte stores.
- Produces:
  - `computeLoupeFocalZoom(transform, viewport, image, focalPoint, factor, minScale?, maxScale?): LoupeTransform`
  - `computeLoupeActualSizeScale(viewport, image): number`
  - `clampLoupePan(transform, viewport, image): LoupeTransform`
  - `computeLoupeSmartZoom(transform, viewport, image, lastInspectionScale?): LoupeTransform`

- [ ] **Step 1: Write failing transform tests**

Add `src/lib/loupe-transform.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
    clampLoupePan,
    computeLoupeActualSizeScale,
    computeLoupeFocalZoom,
    computeLoupeSmartZoom,
} from './loupe-transform';

describe('loupe transform helpers', () => {
    const viewport = { width: 1000, height: 800 };
    const image = { width: 500, height: 400 };

    it('computes actual size relative to fit scale', () => {
        expect(computeLoupeActualSizeScale(viewport, image)).toBeCloseTo(0.5);
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
        expect(clampLoupePan({ scale: 1, panX: 200, panY: -200 }, viewport, image)).toEqual({
            scale: 1,
            panX: 0,
            panY: 0,
        });
    });

    it('smart zoom toggles from fit to actual size and back', () => {
        const actual = computeLoupeSmartZoom({ scale: 1, panX: 0, panY: 0 }, viewport, image);
        expect(actual.scale).toBeCloseTo(0.5);

        const fit = computeLoupeSmartZoom(actual, viewport, image);
        expect(fit).toEqual({ scale: 1, panX: 0, panY: 0 });
    });
});
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
npm test -- src/lib/loupe-transform.test.ts
```

Expected: FAIL because `src/lib/loupe-transform.ts` does not exist.

- [ ] **Step 3: Implement transform helpers**

Create `src/lib/loupe-transform.ts`:

```ts
export interface LoupeTransform {
    scale: number;
    panX: number;
    panY: number;
}

export interface LoupeViewport {
    width: number;
    height: number;
}

export interface LoupeImageSize {
    width: number;
    height: number;
}

export interface LoupePoint {
    x: number;
    y: number;
}

const DEFAULT_MIN_SCALE = 0.1;
const DEFAULT_MAX_SCALE = 20;
const FIT_EPSILON = 0.02;

export function computeLoupeActualSizeScale(viewport: LoupeViewport, image: LoupeImageSize): number {
    const fitScale = Math.min(viewport.width / image.width, viewport.height / image.height);
    if (!Number.isFinite(fitScale) || fitScale <= 0) return 1;
    return clamp(1 / fitScale, DEFAULT_MIN_SCALE, DEFAULT_MAX_SCALE);
}

export function computeLoupeFocalZoom(
    transform: LoupeTransform,
    viewport: LoupeViewport,
    image: LoupeImageSize,
    focalPoint: LoupePoint,
    factor: number,
    minScale = DEFAULT_MIN_SCALE,
    maxScale = DEFAULT_MAX_SCALE,
): LoupeTransform {
    const scale = clamp(transform.scale * factor, minScale, maxScale);
    if (transform.scale <= 0) return clampLoupePan({ ...transform, scale }, viewport, image);
    const ratio = scale / transform.scale;
    return clampLoupePan({
        scale,
        panX: focalPoint.x - (focalPoint.x - transform.panX) * ratio,
        panY: focalPoint.y - (focalPoint.y - transform.panY) * ratio,
    }, viewport, image);
}

export function clampLoupePan(transform: LoupeTransform, viewport: LoupeViewport, image: LoupeImageSize): LoupeTransform {
    const renderedWidth = image.width * fitScale(viewport, image) * transform.scale;
    const renderedHeight = image.height * fitScale(viewport, image) * transform.scale;
    return {
        scale: transform.scale,
        panX: clampAxis(transform.panX, renderedWidth, viewport.width),
        panY: clampAxis(transform.panY, renderedHeight, viewport.height),
    };
}

export function computeLoupeSmartZoom(
    transform: LoupeTransform,
    viewport: LoupeViewport,
    image: LoupeImageSize,
    lastInspectionScale?: number,
): LoupeTransform {
    if (Math.abs(transform.scale - 1) <= FIT_EPSILON) {
        const scale = lastInspectionScale && lastInspectionScale > 1
            ? lastInspectionScale
            : computeLoupeActualSizeScale(viewport, image);
        return clampLoupePan({ scale, panX: 0, panY: 0 }, viewport, image);
    }
    return { scale: 1, panX: 0, panY: 0 };
}

function fitScale(viewport: LoupeViewport, image: LoupeImageSize): number {
    const scale = Math.min(viewport.width / image.width, viewport.height / image.height);
    return Number.isFinite(scale) && scale > 0 ? scale : 1;
}

function clampAxis(pan: number, rendered: number, viewport: number): number {
    if (rendered <= viewport) return 0;
    const limit = (rendered - viewport) / 2;
    return clamp(pan, -limit, limit);
}

function clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, value));
}
```

- [ ] **Step 4: Run tests and verify pass**

Run:

```bash
npm test -- src/lib/loupe-transform.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/loupe-transform.ts src/lib/loupe-transform.test.ts
git commit -m "feat: add loupe gesture transforms"
```

## Task 3: Canvas Factor Zoom Helper

**Files:**
- Modify: `src/lib/canvas-interactions.ts`
- Modify: `src/lib/canvas-interactions.test.ts`

**Interfaces:**
- Consumes: `CanvasViewportTransform`, `CanvasPoint`
- Produces: `computeCanvasZoomAtPoint(viewport, pointer, factor, minZoom?, maxZoom?)`

- [ ] **Step 1: Write failing canvas zoom test**

Add to `src/lib/canvas-interactions.test.ts`:

```ts
import { computeCanvasZoomAtPoint } from './canvas-interactions';

it('zooms canvas around a pointer using a factor', () => {
    const next = computeCanvasZoomAtPoint(
        { panX: 10, panY: 20, zoom: 1 },
        { x: 100, y: 80 },
        2,
    );

    expect(next).toEqual({ panX: -80, panY: -40, zoom: 2 });
});
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
npm test -- src/lib/canvas-interactions.test.ts
```

Expected: FAIL because `computeCanvasZoomAtPoint` is not exported.

- [ ] **Step 3: Implement factor helper and reuse it**

Update `src/lib/canvas-interactions.ts`:

```ts
export function computeCanvasZoomAtPoint(
    viewport: CanvasViewportTransform,
    pointer: CanvasPoint,
    factor: number,
    minZoom = DEFAULT_MIN_ZOOM,
    maxZoom = DEFAULT_MAX_ZOOM,
): CanvasViewportTransform {
    const newZoom = clamp(viewport.zoom * factor, minZoom, maxZoom);
    if (viewport.zoom === 0) {
        return { ...viewport, zoom: newZoom };
    }

    return {
        panX: pointer.x - (pointer.x - viewport.panX) * (newZoom / viewport.zoom),
        panY: pointer.y - (pointer.y - viewport.panY) * (newZoom / viewport.zoom),
        zoom: newZoom,
    };
}
```

Then change `computeCanvasWheelZoom` to:

```ts
export function computeCanvasWheelZoom(
    viewport: CanvasViewportTransform,
    pointer: CanvasPoint,
    deltaY: number,
    minZoom = DEFAULT_MIN_ZOOM,
    maxZoom = DEFAULT_MAX_ZOOM,
): CanvasViewportTransform {
    const factor = deltaY > 0 ? 0.9 : 1.1;
    return computeCanvasZoomAtPoint(viewport, pointer, factor, minZoom, maxZoom);
}
```

- [ ] **Step 4: Run test and verify pass**

Run:

```bash
npm test -- src/lib/canvas-interactions.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/canvas-interactions.ts src/lib/canvas-interactions.test.ts
git commit -m "feat: add canvas factor zoom helper"
```

## Task 4: Grid Gesture Zoom

**Files:**
- Modify: `src/lib/components/Grid.svelte`
- Test: `src/lib/gesture-interactions.test.ts` or new `src/lib/grid-gesture-utils.test.ts` if helper extraction is needed

**Interfaces:**
- Consumes: `wheelGestureIntent`
- Produces: component-local `applyThumbnailZoom(factor, clientY)` or extracted helper if testing needs pure scroll math.

- [ ] **Step 1: Add or extend tests for grid zoom routing**

If extracting a pure helper, create tests asserting:

```ts
expect(nextThumbnailZoom({ size: 160, gap: 4, preset: 1 }, 1.2).size).toBeGreaterThan(160);
expect(nextThumbnailZoom({ size: 160, gap: 4, preset: 1 }, 1).preset).toBe(1);
```

Run:

```bash
npm test -- src/lib/gesture-interactions.test.ts
```

Expected: FAIL until the helper exists.

- [ ] **Step 2: Wire Grid wheel handler without blocking native scroll**

In `Grid.svelte`, add `onwheel={handleWheel}` to `.grid-container` and implement:

```ts
function handleWheel(e: WheelEvent) {
    const intent = wheelGestureIntent({
        surface: 'grid',
        deltaX: e.deltaX,
        deltaY: e.deltaY,
        deltaMode: e.deltaMode,
        clientX: e.clientX,
        clientY: e.clientY,
        ctrlKey: e.ctrlKey,
        metaKey: e.metaKey,
        altKey: e.altKey,
        shiftKey: e.shiftKey,
        viewportHeight: containerHeight,
        target: e.target,
    });
    if (!intent || intent.type !== 'zoom') return;
    e.preventDefault();
    applyThumbnailZoom(intent.factor, e.clientY);
}
```

`applyThumbnailZoom` updates `thumbnailSize`, `gridPreset`, `gridGap`, and `gridScrollTop`.

- [ ] **Step 3: Run focused frontend tests**

Run:

```bash
npm test -- src/lib/gesture-interactions.test.ts
npm run check
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/Grid.svelte src/lib/gesture-interactions.ts src/lib/gesture-interactions.test.ts
git commit -m "feat: add grid gesture zoom"
```

## Task 5: Loupe Gesture Integration

**Files:**
- Modify: `src/lib/components/Loupe.svelte`
- Modify or create tests around `src/lib/loupe-transform.test.ts` and `src/lib/gesture-interactions.test.ts`

**Interfaces:**
- Consumes: `wheelGestureIntent`, `classifySwipe`, `computeLoupeFocalZoom`, `clampLoupePan`
- Produces: Loupe wheel pan, modifier-wheel zoom, and thresholded navigation.

- [ ] **Step 1: Add tests for Loupe routing helpers**

Extend pure tests to assert:

```ts
expect(wheelGestureIntent({ surface: 'loupe', deltaX: 20, deltaY: 0, deltaMode: 0, clientX: 0, clientY: 0, viewportHeight: 800, target: null })?.type).toBe('pan');
expect(classifySwipe({ deltaX: -120, deltaY: 10 })).toBe('next');
```

- [ ] **Step 2: Replace Loupe unconditional wheel zoom**

Change `handleWheel` in `Loupe.svelte` so:

- unmodified wheel pans only when `$loupeScale > 1`
- modifier-wheel zooms around the pointer using `computeLoupeFocalZoom`
- horizontal pan at fit scale accumulates for swipe navigation
- crop mode ignores navigation gestures

- [ ] **Step 3: Keep crop application explicit**

Verify no gesture intent calls `applyCrop()`. Crop mode remains controlled by existing buttons/events and Enter.

- [ ] **Step 4: Run focused tests**

Run:

```bash
npm test -- src/lib/gesture-interactions.test.ts src/lib/loupe-transform.test.ts
npm run check
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/Loupe.svelte src/lib/gesture-interactions.ts src/lib/loupe-transform.ts src/lib/gesture-interactions.test.ts src/lib/loupe-transform.test.ts
git commit -m "feat: add loupe gesture navigation"
```

## Task 6: Canvas Gesture Integration

**Files:**
- Modify: `src/lib/components/Canvas.svelte`
- Modify: `src/lib/canvas-interactions.ts`
- Test: `src/lib/canvas-interactions.test.ts`

**Interfaces:**
- Consumes: `wheelGestureIntent`, `computeCanvasPanDrag`, `computeCanvasZoomAtPoint`
- Produces: Canvas two-finger/wheel pan, modifier-wheel zoom, and debounced persistence.

- [ ] **Step 1: Add test coverage for factor zoom**

Use the Task 3 test if not already committed.

- [ ] **Step 2: Replace Canvas unconditional wheel zoom**

Change `handleWheel`:

```ts
const intent = wheelGestureIntent({ surface: 'canvas', ... });
if (!intent) return;
if (intent.type === 'pan') {
    e.preventDefault();
    panX -= intent.deltaX;
    panY -= intent.deltaY;
    queueCanvasSave();
}
if (intent.type === 'zoom') {
    e.preventDefault();
    const next = computeCanvasZoomAtPoint({ panX, panY, zoom }, pointer, intent.factor);
    panX = next.panX;
    panY = next.panY;
    zoom = next.zoom;
    queueCanvasSave();
}
```

Suppress this while `dragItem`, `resizeItem`, or `cropDraft` is active.

- [ ] **Step 3: Reduce save churn**

Keep `queueCanvasSave()` debounced for viewport changes. Do not call `persistCanvasLayout()` directly from high-frequency gesture events.

- [ ] **Step 4: Run focused tests**

Run:

```bash
npm test -- src/lib/canvas-interactions.test.ts src/lib/gesture-interactions.test.ts
npm run check
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/Canvas.svelte src/lib/canvas-interactions.ts src/lib/canvas-interactions.test.ts
git commit -m "feat: add canvas gesture pan zoom"
```

## Task 7: Compare Gesture Navigation

**Files:**
- Modify: `src/lib/components/Compare.svelte`
- Create: `src/lib/compare-gestures.ts`
- Create: `src/lib/compare-gestures.test.ts`

**Interfaces:**
- Consumes: `classifySwipe`, existing `focusedIndex`, `selectedIds`, `compareActiveSide`
- Produces: horizontal swipe navigation only.

- [ ] **Step 1: Write failing compare navigation helper tests**

Create `src/lib/compare-gestures.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { nextCompareFocusedIndex } from './compare-gestures';

describe('compare gesture navigation', () => {
    it('advances adjacent compare pairs by two images', () => {
        expect(nextCompareFocusedIndex(0, 10, 'next')).toBe(2);
        expect(nextCompareFocusedIndex(2, 10, 'previous')).toBe(0);
    });

    it('clamps compare pair navigation at collection bounds', () => {
        expect(nextCompareFocusedIndex(8, 10, 'next')).toBe(8);
        expect(nextCompareFocusedIndex(0, 10, 'previous')).toBe(0);
        expect(nextCompareFocusedIndex(0, 1, 'next')).toBe(0);
    });
});
```

Run:

```bash
npm test -- src/lib/compare-gestures.test.ts
```

Expected: FAIL because `src/lib/compare-gestures.ts` does not exist.

- [ ] **Step 2: Create compare navigation helper**

Create `src/lib/compare-gestures.ts`:

```ts
export function nextCompareFocusedIndex(current: number, total: number, direction: 'previous' | 'next'): number {
    const delta = direction === 'next' ? 2 : -2;
    return Math.max(0, Math.min(current + delta, Math.max(0, total - 2)));
}
```

- [ ] **Step 3: Verify helper tests pass**

Run:

```bash
npm test -- src/lib/compare-gestures.test.ts
```

Expected: PASS.

- [ ] **Step 4: Add component wheel/swipe routing**

In `Compare.svelte`, accumulate horizontal wheel deltas and call `focusedIndex.set(nextCompareFocusedIndex(...))` only when `classifySwipe` returns a direction. Do not add zoom.

- [ ] **Step 5: Run focused tests/check**

Run:

```bash
npm test -- src/lib/gesture-interactions.test.ts src/lib/compare-gestures.test.ts
npm run check
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/Compare.svelte src/lib/compare-gestures.ts src/lib/compare-gestures.test.ts src/lib/gesture-interactions.ts
git commit -m "feat: add compare swipe navigation"
```

## Task 8: Verification And Landing

**Files:**
- No planned source changes unless verification finds bugs.

- [ ] **Step 1: Run quick preflight**

Run:

```bash
npm run preflight -- quick
```

Expected: PASS.

- [ ] **Step 2: Run manual hardware smoke test**

Run the app and verify:

```bash
npm run dev
```

Manual checks:

- Trackpad Grid: two-finger scroll scrolls; modifier-wheel/pinch zooms thumbnails.
- Trackpad Loupe: unmodified scroll pans when zoomed; modifier-wheel/pinch zooms; horizontal swipe navigates only from rest.
- Trackpad Canvas: scroll pans; modifier-wheel/pinch zooms; item drag/crop still wins over viewport gestures.
- Magic Mouse: surface scroll does not accidentally zoom.

- [ ] **Step 3: Commit any verification fixes**

If fixes were required:

```bash
git add <changed-files>
git commit -m "fix: tune gesture interactions"
```

- [ ] **Step 4: Land session**

Run:

```bash
npm run land
```

Expected: branch rebases/pushes successfully and reports final git/bd status.
