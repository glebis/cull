# P0 — Thumbnail variant selection

**Date:** 2026-06-03
**Status:** Approved (via /goal)
**Theme:** Fast, memory-bounded image rendering on Mac.

## Problem

The backend generates **four JPEG thumbnail variants** per image — `64, 128, 256, 800` px
(`src-tauri/src/db_core/thumbnails.rs:8`, Lanczos3, quality 90). On disk:

- `{id}.jpg` — largest (800px)
- `{id}_64.jpg`, `{id}_128.jpg`, `{id}_256.jpg` — smaller variants

But the frontend stores and uses a **single** `thumbnail_path` (`src/lib/api.ts:42`), and
`safeAssetPreviewPath` (`src/lib/view-utils.ts`) returns it verbatim — which is the **800px**
file. So the grid decodes an 800px JPEG to paint a 64–160px tile (up to ~150× the needed
pixels). This is the dominant cause of slow, memory-heavy dense grids and blocks the goal of
"smaller thumbnails, more items."

## Proposed Solution

Select the smallest existing variant whose pixel size is `>=` the on-screen display size
(times a clamped device-pixel-ratio), falling back to the base path. This is a pure,
unit-testable derivation plus a small `<img>` fallback chain. No Rust changes; no
regeneration; we just start reading the variants that already exist.

### Components

1. **`pickThumbnailVariant(basePath, displayPx, opts)` — `src/lib/view-utils.ts` (new, pure)**
   - Input: base thumbnail path (`.../{id}.jpg`), the display edge in CSS px, and
     `{ dpr, availableSizes = THUMBNAIL_SIZES, maxDprMultiplier = 2 }`.
   - Target = `displayPx * min(dpr, maxDprMultiplier)`.
   - Choose the smallest `size` in `availableSizes` (sorted asc) with `size >= target`.
   - If the chosen size is the largest (800) → return the **base** path (`{id}.jpg`),
     since that is the 800px file. Otherwise return the sibling `{id}_{size}.jpg`,
     derived by inserting `_{size}` before the `.jpg` extension.
   - If `target` exceeds all sizes → return base path.
   - Path derivation must be robust to query/percent-encoded segments and only rewrite the
     final `{id}.jpg` filename.
   - `THUMBNAIL_SIZES = [64, 128, 256, 800]` exported as a shared frontend constant mirroring
     the Rust array.

2. **`safeAssetPreviewPath` integration**
   - Add an optional `displayPx` argument. When provided and the base path is asset-safe,
     run it through `pickThumbnailVariant` before returning. When omitted, behaviour is
     unchanged (back-compat for Loupe/Compare/other callers).

3. **`Thumbnail.svelte` wiring**
   - `Thumbnail` already receives `size`. Compute `previewPath` from
     `safeAssetPreviewPath(item, { displayPx: size })` using `window.devicePixelRatio`.
   - Fallback chain in `onerror`: variant path → base `{id}.jpg` → existing
     `regenerateSingleThumbnail`. This protects images generated before multi-size existed
     (only `{id}.jpg` present) and any missing variant.

### Data flow

Grid passes `size` → Thumbnail computes target px → `pickThumbnailVariant` → `convertFileSrc`
→ `<img src>`. On 404/decode error the `<img onerror>` walks the fallback chain.

### Non-goals

- No new thumbnail sizes (e.g. 512) in this phase — only selection among existing variants.
- No change to thumbnail generation or storage.
- No DB schema change.

## Testing (TDD)

Unit tests in `src/lib/view-utils.test.ts`:

- 64px tile @ dpr 1 → `{id}_64.jpg`
- 64px tile @ dpr 2 → `{id}_128.jpg`
- 160px tile @ dpr 2 (target 320) → base `{id}.jpg` (no 512 available)
- 256px tile @ dpr 1 → `{id}_256.jpg`
- target above all sizes → base path
- non-`.jpg` / odd base path → returns base unchanged (no corruption)
- `safeAssetPreviewPath` without `displayPx` → unchanged output (back-compat)
- unsafe path → still returns null

## Acceptance Criteria

- [ ] `pickThumbnailVariant` selects the smallest variant `>= displayPx * clampedDpr`, else base.
- [ ] Variant path derivation only rewrites the trailing `{id}.jpg` and never corrupts other paths.
- [ ] `safeAssetPreviewPath` is back-compatible when `displayPx` is omitted.
- [ ] `Thumbnail.svelte` requests size-appropriate variants and falls back variant→base→regenerate.
- [ ] Unit tests above pass; `npm test` and `npm run check` are green.
