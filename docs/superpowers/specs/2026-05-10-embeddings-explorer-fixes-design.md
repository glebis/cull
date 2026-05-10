# Embeddings Explorer Fixes — Design Spec

**Date:** 2026-05-10
**Status:** Reviewed / ready for implementation
**Scope:** 4 related fixes to the EmbeddingExplorer component and app state persistence

---

## Problem Statement

The Embeddings tab has four issues that prevent it from matching the original vision of an interactive, zoomable thumbnail scatter plot:

1. **Most images render as dots, not thumbnails** — only the currently filtered subset shows thumbnails
2. **Thumbnails cap at 48px** — users cannot zoom in close enough to inspect images
3. **View state resets on tab switch** — pan, zoom, and selection are lost when switching to Loupe and back
4. **No state persistence across relaunches** — all app preferences (view, folder, zoom, filters) are lost on restart

These fixes are related. Fix 1 and Fix 2 can be developed independently, but both modify `EmbeddingExplorer.svelte` and must be merged carefully. Fix 3 also modifies `EmbeddingExplorer.svelte` and should land after Fix 1 and Fix 2. Fix 4 depends on Fix 3's `embeddingViewState` store.

---

## Fix 1: Render All Embedded Images as Thumbnails

### Root Cause

`loadProjection()` in `EmbeddingExplorer.svelte:336-340` builds `imageMap` from `$images`, which is the currently filtered/folder-scoped subset. `getAllEmbeddings()` returns IDs for all embedded images (e.g. 310), but `$images` may contain only 8. Any embedding ID absent from `imageMap` falls through to the colored-dot fallback in the `draw()` function.

Additionally, click handlers (`handlePointClick`, `handleFocusInGrid`, `handleCanvasDblClick`) search `$images` by index via `focusedIndex`, which cannot represent images outside the filtered set.

### Design

**Data flow change:**

```
Before:
  getAllEmbeddings() → ids
  $images (filtered) → imageMap
  draw() → most points have no image → colored dot

After:
  getAllEmbeddings() → ids
  getImagesByIds(ids) → embeddingImages (full corpus)
  embeddingImages → imageMap
  draw() → all points have image data → thumbnails
```

**Implementation:**

1. In `loadProjection()`, replace lines 336-340:

```typescript
// Before:
const map = new Map<string, ImageWithFile>();
for (const img of $images) {
    map.set(img.image.id, img);
}
imageMap = map;

// After:
const embeddingIds = embeddings.map(([id]) => id);
const embeddingImages = await getImagesByIds(embeddingIds);
const map = new Map<string, ImageWithFile>();
for (const img of embeddingImages) {
    map.set(img.image.id, img);
}
imageMap = map;
```

2. Fix Loupe navigation for images outside the filtered set. Do **not** append out-of-filter images into the global `images` store from `EmbeddingExplorer.svelte`.

```typescript
export const focusedImageOverride = writable<ImageWithFile | null>(null);

export const focusedImage = derived(
    [images, focusedIndex, focusedImageOverride],
    ([$images, $idx, $override]) => $override ?? $images[$idx] ?? null
);
```

Add a helper in `EmbeddingExplorer.svelte`:

```typescript
function focusImageForLoupe(imageId: string): boolean {
    const idx = $images.findIndex(img => img.image.id === imageId);
    if (idx >= 0) {
        focusedImageOverride.set(null);
        focusedIndex.set(idx);
        return true;
    }

    const img = imageMap.get(imageId);
    if (!img) return false;
    focusedImageOverride.set(img);
    return true;
}
```

Use this helper in `handlePointClick()`, `handleFocusInGrid()`, and `handleCanvasDblClick()`. `handlePointClick()` should still set `selectedPoint` and zoom even if the image is not in `$images`; it should set `focusedIndex` only when the point is in the current list.

Update `Loupe.svelte` to read `focusedImage` instead of deriving directly from `$images[$focusedIndex]`.

### Files Modified

| File | Change |
|------|--------|
| `src/lib/components/EmbeddingExplorer.svelte` | Replace imageMap construction in `loadProjection()`. Fix `handlePointClick`, `handleFocusInGrid`, `handleCanvasDblClick` to handle images outside filtered set. Import `focusedImageOverride`. |
| `src/lib/stores.ts` | Add `focusedImageOverride` and update `focusedImage` derived store. |
| `src/lib/components/Loupe.svelte` | Use `focusedImage` derived store instead of `$images[$focusedIndex]`. |

### Edge Cases

- **Image deleted from DB but embedding still exists:** `getImagesByIds` will return fewer results than requested. The embedding point will still render as a colored dot. This is acceptable — stale embeddings are cleaned up during regeneration.
- **Large embedding sets (1000+):** `getImagesByIds` accepts an array of IDs. The Rust side does a batch `SELECT ... WHERE id IN (...)` which SQLite handles fine up to ~1000 IDs. For larger sets, chunk into batches of 500.
- **Out-of-filter Loupe navigation:** Use `focusedImageOverride`; do not mutate `images`. Mutating `images` from `EmbeddingExplorer` pollutes the filtered/folder-scoped global list, can create duplicates, changes next/previous Loupe behavior, and can race with `loadImages()` replacing the store.
- **Override lifetime:** Clear `focusedImageOverride` whenever the user navigates to a normal grid image, changes folder/collection/filter, or `loadImages()` replaces `images`.

### Testing

- Verify: with a folder filter active (showing 8 images), switch to Embeddings tab → all 310+ points should render as thumbnails, not dots.
- Verify: double-click a point whose image is NOT in the current folder filter → Loupe opens showing that image.
- Verify: after returning from Loupe, the embedding view still shows all thumbnails.

---

## Fix 2: Remove Thumbnail Size Cap, Support Deep Zoom

### Root Cause

`EmbeddingExplorer.svelte:474-475`:
```typescript
const pointDensityFactor = Math.max(1, Math.sqrt(points.length / 10));
const baseThumbSize = Math.max(4, Math.min(48, (8 * Math.sqrt(scale)) / pointDensityFactor));
```

With 310 points, `pointDensityFactor ≈ 5.6`. Even at `scale=100`, `baseThumbSize` = `min(48, 8*10/5.6)` = `min(48, 14.3)` = 14px. The 48px hard cap and the constant density penalty make deep zoom useless.

The frontend `THUMB_SIZES` array at line 55 is `[64, 128, 256]`, but the Rust backend generates thumbnails at `[64, 128, 256, 800]` (`thumbnails.rs:8`). The 800px tier is never used.

### Design

**New formula:** density penalty fades as zoom increases, allowing thumbnails to grow large when zoomed in while keeping the overview clean:

```typescript
export function computeScatterThumbSize(
    scale: number,
    pointCount: number
): { size: number; useThumb: boolean } {
    const density = Math.max(1, Math.sqrt(pointCount / 80));
    const densityWeight = 1 / Math.sqrt(Math.max(scale, 1));
    const penalty = 1 + (density - 1) * densityWeight;
    const size = Math.max(4, Math.min(192, 10 * Math.pow(scale, 0.45) / penalty));
    return { size, useThumb: size >= 8 };
}
```

**Behavior at key zoom levels (310 points, density ≈ 1.97):**

| Scale | densityWeight | penalty | Raw size | Clamped | Thumbnail tier |
|-------|--------------|---------|----------|---------|----------------|
| 1     | 1.00         | 1.97    | 5.1      | 5       | dot            |
| 5     | 0.45         | 1.43    | 14.4     | 14      | 64px           |
| 20    | 0.22         | 1.22    | 31.6     | 32      | 64px           |
| 100   | 0.10         | 1.10    | 72.4     | 72      | 128px          |
| 500   | 0.04         | 1.04    | 157.1    | 157     | 256px          |
| 800   | 0.04         | 1.03    | 195.8    | 192     | 256px          |

**THUMB_SIZES update:** add 800 to the array: `[64, 128, 256, 800]`.

### Files Modified

| File | Change |
|------|--------|
| `src/lib/embedding-utils.ts` | Replace `computeScatterThumbSize()` with new formula |
| `src/lib/embedding-utils.test.ts` | Update tests for new formula behavior |
| `src/lib/components/EmbeddingExplorer.svelte` | Change `THUMB_SIZES` from `[64, 128, 256]` to `[64, 128, 256, 800]`. Import `computeScatterThumbSize` from `$lib/embedding-utils`. Replace inline thumb size calculation at line 474-475 with `const { size: baseThumbSize, useThumb } = computeScatterThumbSize(scale, points.length)`. Use the same helper for hover hit testing so visual size and clickable area stay aligned. |

### Edge Cases

- **Performance at extreme zoom:** at `scale > 1000`, thumbnails will be ~160-192px. With 310 points, only ~20-40 will be visible on screen (viewport culling at line 482 already skips off-screen points). Canvas performance is fine.
- **Missing 800px thumbnails:** if images were imported before the thumbnail pyramid was introduced, the 800px variant may not exist. `pickThumbnail()` already falls back to the next available size. No issue.
- **Minimum zoom:** no change needed — at `scale < 1`, points are 4px dots, which is correct for the overview.

### Testing

- Verify: at default zoom, points appear as small thumbnails (~8-12px), not dots.
- Verify: scroll-zoom in on a cluster → thumbnails grow to 60-120px, clearly visible.
- Verify: zoom in further → thumbnails reach 150-190px, showing image detail.
- Verify: zoom back out → thumbnails shrink smoothly, no artifacts.

---

## Fix 3: Persist Embedding View State Across Tab Switches

### Root Cause

All embedding view state is component-local `$state()` variables (lines 56-67):

```typescript
let panX = $state(0);
let panY = $state(0);
let scale = $state(1);
let selectedPoint = $state<Point | null>(null);
let highlightedCluster = $state<number | null>(null);
```

The `{#if $viewMode === 'embeddings'}` block in `+page.svelte:140-141` destroys and recreates the component on every tab switch, resetting all state. Additionally, `handleResize()` at line 711-712 unconditionally calls `fitView()`, overwriting any user pan/zoom when the window resizes.

### Design

**New store in `stores.ts`:**

```typescript
export interface EmbeddingViewState {
    panX: number;
    panY: number;
    scale: number;
    selectedPointId: string | null;
    highlightedCluster: number | null;
    provider: 'clip' | 'gemini';
    projectionKey: string | null;
    hasUserView: boolean;  // false until user pans/zooms/selects
}

export const embeddingViewState = writable<EmbeddingViewState>({
    panX: 0,
    panY: 0,
    scale: 1,
    selectedPointId: null,
    highlightedCluster: null,
    provider: 'clip',
    projectionKey: null,
    hasUserView: false,
});
```

**Component integration:**

1. On mount, read from `$embeddingViewState` and restore `selectedProvider`. Do not restore `panX`, `panY`, or `scale` until after `loadProjection()` has produced points and computed the current `projectionKey`.

2. Make the projection deterministic enough to reuse saved view coordinates:
   - Sort `embeddings` by image ID before building `vectors`.
   - Pass a deterministic `random` function to `new UMAP(...)`.
   - Make k-means deterministic too, or replace the local random `kMeans()` with a seeded/deterministic helper.
   - Compute `projectionKey` from `selectedProvider`, sorted IDs, and embedding count. A simple stable key such as `${provider}:${ids.length}:${ids.join('|')}` is fine for this feature.

3. After `loadProjection()` builds the `points` array, restore `panX`, `panY`, `scale`, `highlightedCluster`, and `selectedPoint` only if the saved state matches the current projection:
   ```typescript
   const savedState = get(embeddingViewState);
   if (savedState.hasUserView && savedState.provider === selectedProvider && savedState.projectionKey === projectionKey) {
       panX = savedState.panX;
       panY = savedState.panY;
       scale = savedState.scale;
       highlightedCluster = savedState.highlightedCluster;
       selectedPoint = savedState.selectedPointId
           ? points.find(p => p.id === savedState.selectedPointId) ?? null
           : null;
   } else {
       highlightedCluster = null;
       selectedPoint = null;
       fitView();
   }
   ```

4. On every user interaction that changes view state (pan, zoom, selection, cluster highlight), save back to the store. Use a `saveViewState()` helper called from `handleWheel`, `handleMouseUp` (after pan), `handlePointClick`, `zoomToPoint` when its animation completes, and `focusCluster`:
   ```typescript
   function saveViewState() {
       embeddingViewState.set({
           panX, panY, scale,
           selectedPointId: selectedPoint?.id ?? null,
           highlightedCluster,
           provider: selectedProvider,
           projectionKey,
           hasUserView: true,
       });
   }
   ```

5. In `handleResize()`, replace unconditional `fitView()`:
   ```typescript
   // Before:
   if (points.length > 0) {
       fitView();
       requestDraw();
   }

   // After:
   if (points.length > 0) {
       const savedState = get(embeddingViewState);
       if (!savedState.hasUserView || savedState.provider !== selectedProvider || savedState.projectionKey !== projectionKey) {
           fitView();
       }
       requestDraw();
   }
   ```

6. When the provider changes or the embedding set changes (different IDs), do not blindly reset the store during every `loadProjection()` call. Instead, compute a new `projectionKey`; if it differs from the saved key, `fitView()` runs and the next user interaction writes the new key.

### Files Modified

| File | Change |
|------|--------|
| `src/lib/stores.ts` | Add `EmbeddingViewState` interface and `embeddingViewState` writable store |
| `src/lib/components/EmbeddingExplorer.svelte` | Import and use `embeddingViewState`. Restore on mount, save on interaction, conditional `fitView()`. |

### Edge Cases

- **UMAP produces different projections each run:** UMAP is stochastic by default. Resetting `hasUserView` whenever `loadProjection()` runs is not sufficient because `loadProjection()` runs on every Embeddings remount, including normal tab switches. Use sorted IDs plus deterministic UMAP/k-means random sources, and validate saved view state with `projectionKey`.
- **Provider switch:** when `selectedProvider` changes, `projectionKey` changes. The saved view is ignored and `fitView()` runs.
- **selectedPointId references deleted image:** `points.find()` returns null. Selection is cleared. Correct.
- **Concurrent loads:** guard `loadProjection()` with a monotonically increasing request token so a slower previous provider load cannot overwrite `points`, `imageMap`, or restored view state after a newer provider selection.

### Testing

- Verify: pan and zoom the embedding view → switch to Loupe → switch back to Embeddings → same pan/zoom/position.
- Verify: select a point → switch to Grid → switch back → same point still selected and highlighted.
- Verify: click "Regenerate All" → view resets to fitView (new UMAP projection).
- Verify: resize window → if user had panned/zoomed, position is preserved (not reset to fitView).

---

## Fix 4: App State Autosave via localStorage

### Root Cause

No persistence mechanism exists anywhere in the frontend. All store values are in-memory writables that reset to defaults on page load.

### Design

**New module: `src/lib/persistence.ts`**

A small typed persistence layer using `localStorage`:

```typescript
const STORAGE_KEY = 'imageview-app-state';
const SCHEMA_VERSION = 1;

interface PersistedState {
    _version: number;
    viewMode: ViewMode;
    thumbnailSize: number;
    gridPreset: number;
    gridGap: number;
    sidebarVisible: boolean;
    zenMode: boolean;
    activeFolder: string | null;
    activeCollection: string | null;
    activeSmartCollectionId: string | null;
    minSizeFilter: number;
    loupeScale: number;
    loupePanX: number;
    loupePanY: number;
    lineageLayout: LineageLayout;
    showDetectionBoxes: boolean;
    nsfwMode: NsfwMode;
    embeddingViewState: EmbeddingViewState;
}
```

**Three functions:**

```typescript
export function saveAppState(): void {
    const state: PersistedState = {
        _version: SCHEMA_VERSION,
        viewMode: get(viewMode),
        thumbnailSize: get(thumbnailSize),
        gridPreset: get(gridPreset),
        gridGap: get(gridGap),
        sidebarVisible: get(sidebarVisible),
        zenMode: get(zenMode),
        activeFolder: get(activeFolder),
        activeCollection: get(activeCollection),
        activeSmartCollectionId: get(activeSmartCollection)?.id ?? null,
        minSizeFilter: get(minSizeFilter),
        loupeScale: get(loupeScale),
        loupePanX: get(loupePanX),
        loupePanY: get(loupePanY),
        lineageLayout: get(lineageLayout),
        showDetectionBoxes: get(showDetectionBoxes),
        nsfwMode: get(nsfwMode),
        embeddingViewState: get(embeddingViewState),
    };
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch {
        // localStorage full or unavailable — silent fail
    }
}

export function restoreAppStateBeforeImages(): PersistedState | null {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (!raw) return null;
        const state: PersistedState = JSON.parse(raw);
        if (state._version !== SCHEMA_VERSION) return null; // version mismatch — ignore

        // Set filters before loadImages(); apply viewMode after image restore.
        thumbnailSize.set(state.thumbnailSize);
        gridPreset.set(state.gridPreset);
        gridGap.set(state.gridGap);
        sidebarVisible.set(state.sidebarVisible);
        zenMode.set(state.zenMode);
        activeFolder.set(state.activeFolder);
        activeCollection.set(state.activeCollection);
        // activeSmartCollection restored by ID after collections are loaded
        minSizeFilter.set(state.minSizeFilter);
        loupeScale.set(state.loupeScale);
        loupePanX.set(state.loupePanX);
        loupePanY.set(state.loupePanY);
        lineageLayout.set(state.lineageLayout);
        showDetectionBoxes.set(state.showDetectionBoxes);
        nsfwMode.set(state.nsfwMode);
        embeddingViewState.set(state.embeddingViewState);
        return state;
    } catch {
        // Corrupted data — ignore
        return null;
    }
}

export function applyRestoredViewState(state: PersistedState | null): void {
    if (!state) return;
    viewMode.set(state.viewMode);
}
```

**Integration in `+page.svelte`:**

```typescript
import { saveAppState, restoreAppStateBeforeImages, applyRestoredViewState } from '$lib/persistence';

onMount(() => {
    const init = async () => {
        const restored = restoreAppStateBeforeImages();
        await loadImages();
        applyRestoredViewState(restored);

        // Keep the existing deep-link behavior after initial load so launch
        // params can override restored state and replace images when needed.
        await initDeepLink();
    };
    init().catch(e => console.error('Failed to initialize app state:', e));
    // ... existing init code ...

    // Debounced autosave every 5s
    let saveTimer: ReturnType<typeof setInterval>;
    saveTimer = setInterval(saveAppState, 5000);

    // Save on close
    const handleBeforeUnload = () => saveAppState();
    window.addEventListener('beforeunload', handleBeforeUnload);

    return () => {
        clearInterval(saveTimer);
        window.removeEventListener('beforeunload', handleBeforeUnload);
        saveAppState(); // Final save on component unmount
    };
});
```

**What is NOT persisted:**

- `focusedIndex` — depends on the loaded image set, which may differ across sessions
- API keys — handled by backend keyring commands
- `$images` — fetched fresh from DB on each launch
- `selectedIds` — transient selection state
- `compareImages` — transient compare state
- `importBatchFilter`, `importBatchImageIds` — ephemeral import state
- `toasts` — ephemeral notifications

### Restore Order

The restore must happen before `loadImages()` because `activeFolder` and `activeCollection` determine which images are fetched:

```
restoreAppStateBeforeImages() → sets activeFolder, activeCollection, minSizeFilter, etc.
loadImages()                  → reads restored filters → fetches correct image subset
applyRestoredViewState()      → sets viewMode and view-specific UI state
EmbeddingExplorer mount       → loads deterministic projection → restores pan/zoom if projectionKey matches
initDeepLink()                → launch URL can override restored state and replace images
```

Do not set `viewMode` before `loadImages()` completes. Restoring `viewMode = 'loupe'` or `'embeddings'` before image data is available can mount view components against an empty image list and produce transient empty UI or stale focused state.

### Schema Versioning

When `SCHEMA_VERSION` changes, old persisted state is silently discarded. This is intentional — UI preferences are not critical data and can be reset. Future versions can add migration logic if needed:

```typescript
if (state._version === 1) {
    // migrate v1 → v2
    state.newField = defaultValue;
    state._version = 2;
}
```

### Files Modified

| File | Change |
|------|--------|
| `src/lib/persistence.ts` | New file — `saveAppState()`, `restoreAppStateBeforeImages()`, `applyRestoredViewState()`, `PersistedState` interface |
| `src/routes/+page.svelte` | Import and call `restoreAppStateBeforeImages()` before `loadImages()` in `onMount()`, then call `applyRestoredViewState()`. Set up autosave interval and `beforeunload` handler. |

### Edge Cases

- **localStorage unavailable:** WebView should always support it, but the `try/catch` handles edge cases silently.
- **Stale folder reference:** if `activeFolder` was set to a folder that no longer exists, `listImagesByFolder()` will return an empty list. User sees an empty grid and can switch folders. This is acceptable — no crash.
- **Stale smart collection ID:** `activeSmartCollectionId` is persisted as a string. Existing `+page.svelte::loadImages()` does not read `activeSmartCollection`, so restoring smart collections requires additional integration: load smart collections before `loadImages()`, resolve the saved ID to an object, and call `evaluateSmartCollection()` when active. If this is not implemented, do not claim smart collection persistence.
- **Concurrent windows:** if multiple windows are open, the last one to save wins. This is acceptable for a desktop app.
- **Deep links:** `initDeepLink()` currently handles launch URLs and may also fetch/replace `images` itself. Keep it after initial persisted-state restore/load so launch params override saved preferences. If `initDeepLink()` is later refactored to return parsed launch params instead of applying them, then apply those params before `loadImages()`.

### Testing

- Verify: set folder filter, switch to Embeddings tab, zoom in → quit app → relaunch → same folder, same view mode (Embeddings), same zoom level.
- Verify: change grid size to XL → relaunch → grid is still XL.
- Verify: corrupt `localStorage` value → app starts with defaults, no crash.
- Verify: delete `localStorage` key → app starts with defaults.

---

## File Modification Summary

| File | Fix # | Type |
|------|-------|------|
| `src/lib/components/EmbeddingExplorer.svelte` | 1, 2, 3 | Modify |
| `src/lib/components/Loupe.svelte` | 1 | Modify |
| `src/lib/embedding-utils.ts` | 2 | Modify |
| `src/lib/embedding-utils.test.ts` | 2 | Modify |
| `src/lib/stores.ts` | 1, 3 | Modify |
| `src/lib/persistence.ts` | 4 | New |
| `src/routes/+page.svelte` | 4 | Modify |

## Implementation Order

These fixes are not fully independent:

- **Fix 1** (image rendering) and **Fix 2** (zoom formula) can be implemented in parallel, but both touch `EmbeddingExplorer.svelte`; expect a small merge conflict around imports and draw/hit-test helpers.
- **Fix 3** (view state store) adds to `stores.ts` and modifies mounting, `loadProjection()`, provider changes, resize handling, click handling, and animation completion in `EmbeddingExplorer.svelte`. It should be applied after Fix 1 and Fix 2.
- **Fix 4** (persistence) creates a new file and modifies `+page.svelte` — independent of the other three. Depends on Fix 3 for `embeddingViewState` being in the store, so should apply after Fix 3.

**Recommended parallel groups:**
- Group A (parallel): Fix 1 + Fix 2
- Group B (sequential after A): Fix 3, then Fix 4

Or if implementing with subagents:
- Agent 1: Fix 1 (image map + Loupe navigation)
- Agent 2: Fix 2 (zoom formula + THUMB_SIZES)
- Agent 3: Fix 3 (view state store) — after Agent 1 and 2 complete, since all touch EmbeddingExplorer
- Agent 4: Fix 4 (persistence) — after Agent 3 completes, since it depends on `embeddingViewState`

## Dependencies

- No new Rust commands needed — `getImagesByIds` already exists
- No new npm packages — `localStorage` is built-in
- No new Tauri plugins — `localStorage` works in Tauri WebView
- No database schema changes
