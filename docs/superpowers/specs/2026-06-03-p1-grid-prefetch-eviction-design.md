# P1 — Grid prefetch + eviction

**Date:** 2026-06-03
**Status:** Approved (via /goal)
**Depends on:** P0 (variant selection) — prefetch warms the *correct* variant URLs.

## Problem

`Grid.svelte` is virtualized (`computeVisibleItems`, `src/lib/view-utils.ts`) so off-screen
rows unmount. But:

- Forward overscan = `preloadRows` (~`ceil(IMAGE_PAGE_SIZE / cols)`), backward overscan = **2
  rows** (`Grid.svelte:28`). Scrolling **up** shows blank cells.
- Overscan is symmetric/static — it ignores scroll direction and speed.
- When a row enters the window the browser only *then* fetches + decodes the `asset://`
  thumbnail → visible pop-in during fast scroll.
- There is no active warming of the next screen and no explicit bound on warmed images.

Goal: next screen pre-filled so scroll feels instant; previously-loaded images released so
memory stays flat.

## Proposed Solution

Add direction-aware overscan plus a bounded decode-warming prefetch cache. Keep all decision
logic in pure functions (unit-tested); the only side effect is constructing detached `Image`
objects to warm the webview decode cache.

### Components

1. **`computeScrollDirection(prevScrollTop, scrollTop, prevDir)` — `view-utils.ts` (pure)**
   - Returns `'down' | 'up' | 'none'` with a small hysteresis threshold so jitter near zero
     delta does not flip direction.

2. **`computeOverscan(direction, baseRows, options)` — `view-utils.ts` (pure)**
   - Asymmetric overscan: more rows ahead in the travel direction, fewer behind.
   - e.g. down → `{ before: 1, after: baseRows * 2 }`; up → `{ before: baseRows * 2, after: 1 }`;
     none → `{ before: 2, after: baseRows }`.
   - Existing `computeVisibleItems(overscanRowsBefore, overscanRowsAfter)` consumes this.

3. **`computePrefetchIndices(scrollTop, containerHeight, cols, cellSize, total, direction, rows)`
   — `view-utils.ts` (pure)**
   - Returns the flat item indices for ~N rows beyond the rendered window in the travel
     direction (the "next screen") to warm — distinct from what is mounted.

4. **`createPrefetchCache(maxEntries)` — `src/lib/prefetch-cache.ts` (new)**
   - LRU keyed by resolved variant URL. `warm(url)` creates a detached `Image`, sets
     `decoding='async'` and `src=url`, inserts into the LRU; on overflow it evicts the
     oldest entry and drops its reference (and sets `img.src=''`) so the webview can reclaim.
   - `has(url)`, `size()`, `clear()` for tests and teardown.
   - The Image constructor is injectable (default `() => new Image()`) so the LRU eviction
     logic is unit-testable headless without a DOM.

5. **`Grid.svelte` wiring**
   - Track `prevScrollTop`, derive `direction` via `computeScrollDirection`.
   - Feed `computeOverscan` into `computeVisibleItems`.
   - On scroll/resize, compute `computePrefetchIndices`, resolve each to a variant URL
     (P0 `pickThumbnailVariant` + `convertFileSrc`), and `cache.warm(url)`.
   - `maxEntries` sized to a few screens (e.g. `max(60, cols * visibleRows * 3)`); cache
     cleared on scope change and component destroy.

### Data flow

scroll → direction → asymmetric overscan (mount window) + prefetch indices (warm-ahead) →
LRU warms next-screen variant URLs → when those rows mount, images are already decoded →
instant paint. LRU eviction releases the trailing screens.

### Non-goals

- No GPU/WebGL/WebGPU rendering. Stays on DOM `<img>`.
- No windowing of the `images` metadata array (separate concern; cheap per item).
- No change to pagination (`loadMoreImagesForCurrentScope`).

## Testing (TDD)

`src/lib/view-utils.test.ts`:
- `computeScrollDirection` returns down/up and resists jitter under threshold.
- `computeOverscan` is asymmetric and matches direction.
- `computePrefetchIndices` targets rows ahead of the window, clamps at list bounds, returns
  none when nothing more to warm.

`src/lib/prefetch-cache.test.ts`:
- warming inserts; duplicate warm is a no-op move-to-front.
- exceeding `maxEntries` evicts the oldest and calls teardown on it (injected fake Image).
- `clear()` empties and tears down all.

## Acceptance Criteria

- [ ] Scrolling up no longer shows blank cells under normal speed (asymmetric overscan).
- [ ] Next-screen thumbnails are warmed before they mount (prefetch indices + LRU).
- [ ] Prefetch cache never exceeds `maxEntries`; evicted entries are torn down (no leak).
- [ ] Cache is cleared on scope change and component destroy.
- [ ] All pure decision logic is unit-tested; `npm test` and `npm run check` green.
