# P2 — Canvas viewport culling + render cap

**Date:** 2026-06-03
**Status:** Approved (via /goal)
**Depends on:** P0 (variant selection) for the per-item image URLs it renders.

## Problem

`Canvas.svelte` (freeform pan/zoom moodboard) renders **every** item —
`{#each canvasItems}` (`src/lib/components/Canvas.svelte:479`) — as DOM `<img>` inside a
CSS-transformed layer (`transform: translate(panX,panY) scale(zoom)`). There is **no
virtualization, no viewport culling, and no cap**. At hundreds of items it strains; at
thousands it is unusable. (It is DOM + CSS transforms, not a `<canvas>` element — no GPU.)

## Proposed Solution

Render only the items whose transformed bounding box intersects the visible viewport (plus a
margin), and impose a hard render cap with a visible warning when a layout contains more
items than we can render smoothly. Culling math is a pure, unit-testable function; the
component consumes it.

### Components

1. **`computeVisibleCanvasItems(items, viewport, opts)` — `src/lib/canvas-utils.ts` (extend
   or new, pure)**
   - `viewport = { panX, panY, zoom, width, height }` (CSS px of the canvas viewport).
   - For each item with `{ x, y, width, height, rotation }`, compute its **screen-space**
     bbox: `screenX = x * zoom + panX`, etc. For rotated items use the rotated AABB
     (rotate corners, take min/max) so rotation never clips an on-screen item.
   - Keep items whose screen bbox intersects `[0,width] × [0,height]` expanded by
     `opts.margin` (default ~one viewport of overscan so panning feels instant).
   - Returns the kept items in original order (stable keys).

2. **`capCanvasItems(items, max)` — `canvas-utils.ts` (pure)**
   - Returns `{ rendered, droppedCount }`; `rendered = items.slice(0, max)` when over cap.
   - `max` default e.g. 1500 (tunable constant `CANVAS_RENDER_CAP`).

3. **`Canvas.svelte` wiring**
   - Bind viewport size via `ResizeObserver` (mirror Grid's pattern), track `panX/panY/zoom`
     (already present).
   - `visibleItems = capCanvasItems(computeVisibleCanvasItems(canvasItems, viewport, {margin}), CAP)`.
   - Render `{#each visibleItems.rendered}`; show a small non-blocking banner when
     `droppedCount > 0` ("Showing N of M — zoom in to see more"), styled with theme tokens.
   - Item `<img>` uses P0 variant selection sized to the on-screen item size
     (`width * zoom`).

### Data flow

pan/zoom/resize → recompute screen-space bboxes → cull to viewport+margin → cap →
render subset. Off-screen items are not mounted, so DOM node count tracks the viewport, not
the library size.

### Non-goals

- No GPU/WebGL rendering, no texture atlas (deferred; revisit only if DOM culling proves
  insufficient at target densities).
- No change to the canvas document model, persistence, or layout algorithm.
- No change to selection/context-menu behaviour beyond operating on rendered items.

## Testing (TDD)

`src/lib/canvas-utils.test.ts`:
- item fully inside viewport → kept; fully outside (beyond margin) → dropped.
- item straddling an edge → kept.
- pan/zoom changes flip an item in/out correctly.
- rotated item near an edge → kept (rotated AABB), not clipped.
- `capCanvasItems`: under cap → all rendered, `droppedCount 0`; over cap → first `max`
  rendered, correct `droppedCount`.

## Acceptance Criteria

- [ ] Only items intersecting viewport+margin are rendered; off-screen items are unmounted.
- [ ] Rotated items near edges are not clipped (rotated AABB used).
- [ ] A hard cap limits rendered nodes; a non-blocking banner reports dropped count.
- [ ] Canvas image elements use size-appropriate thumbnail variants (P0).
- [ ] Culling/cap logic is unit-tested; `npm test` and `npm run check` green.
