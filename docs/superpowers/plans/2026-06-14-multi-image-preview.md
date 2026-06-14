# Multi-image Preview Display Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a live selected-set Preview Display mode that recomposes up to 30 selected Grid images into masonry, magazine, hero strip, or dense overview layouts.

**Architecture:** Extend the existing Preview Display state rather than creating a new view. Keep layout math in a pure frontend utility, keep canonical presentation state in Rust `PreviewState`, and let the main window sync ordered selected IDs to the preview window through the existing `update_preview_state` event path.

**Tech Stack:** Svelte 5 runes, TypeScript/Vitest, Tauri 2 IPC, Rust/serde, existing Preview Display commands and stores.

---

## File Structure

- Create `src/lib/preview-layout.ts`: pure layout recipe types, selected-set ordering helper, cap helper, and deterministic layout item computation.
- Create `src/lib/preview-layout.test.ts`: Vitest coverage for caps, ordering, deterministic bounds, and recipe differences.
- Modify `src/lib/api.ts`: add `PreviewFeed`, `PreviewLayoutRecipe`, `selected_image_ids`, `feed`, and `layout_recipe` fields to Preview Display types and `updatePreviewState`.
- Modify `src-tauri/src/preview/state.rs`: add Rust equivalents for feed/layout/selected IDs, defaults, cap enforcement, and unit tests.
- Modify `src-tauri/src/commands/preview.rs`: accept the new state fields in `update_preview_state`.
- Modify `src/routes/+page.svelte`: sync ordered selected IDs when Preview Display feed is `selected_set`.
- Modify `src/lib/preview-display.ts`: add feed/layout helpers and selected-set sync payload helpers.
- Modify `src/lib/preview-display-store.ts`: add stores and parsers for feed/layout settings.
- Modify `src/lib/components/PreviewDisplay.svelte`: render multi-image layouts while preserving single-image behavior.
- Modify `src/lib/keys.ts`, `src/lib/menu.ts`, and `src-tauri/src/menu.rs`: add feed toggle and layout-cycle commands.
- Modify `docs/preview-display.md` and `docs/USER_GUIDE.md`: document selected-set preview mode and its cap.

## Task 1: Pure Layout Utility

**Files:**
- Create: `src/lib/preview-layout.ts`
- Create: `src/lib/preview-layout.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/lib/preview-layout.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
    PREVIEW_SELECTED_SET_LIMIT,
    capPreviewSelection,
    computePreviewLayout,
    orderPreviewSelection,
    type PreviewLayoutInputImage,
} from './preview-layout';

const images: PreviewLayoutInputImage[] = [
    { id: 'a', width: 1200, height: 800 },
    { id: 'b', width: 800, height: 1200 },
    { id: 'c', width: 1000, height: 1000 },
    { id: 'd', width: 1600, height: 900 },
];

describe('preview-layout', () => {
    it('caps selected-set preview input at 30 ids', () => {
        const ids = Array.from({ length: 34 }, (_, index) => `img-${index}`);
        expect(capPreviewSelection(ids)).toHaveLength(PREVIEW_SELECTED_SET_LIMIT);
        expect(capPreviewSelection(ids).at(-1)).toBe('img-29');
    });

    it('orders selected ids by visible grid order and keeps off-slice ids after visible ids', () => {
        const selected = new Set(['c', 'x', 'a', 'b']);
        const ordered = orderPreviewSelection(selected, ['b', 'a', 'c', 'd']);
        expect(ordered).toEqual(['b', 'a', 'c', 'x']);
    });

    it('keeps computed masonry layout inside the viewport', () => {
        const layout = computePreviewLayout({
            images,
            recipe: 'masonry',
            width: 1000,
            height: 700,
            gap: 8,
        });
        expect(layout.items).toHaveLength(4);
        for (const item of layout.items) {
            expect(item.x).toBeGreaterThanOrEqual(0);
            expect(item.y).toBeGreaterThanOrEqual(0);
            expect(item.x + item.width).toBeLessThanOrEqual(1000);
            expect(item.y + item.height).toBeLessThanOrEqual(700);
        }
    });

    it('makes magazine layout assign the first image as the largest item', () => {
        const layout = computePreviewLayout({
            images,
            recipe: 'magazine',
            width: 1000,
            height: 700,
            gap: 8,
        });
        const [hero, ...supporting] = layout.items;
        const heroArea = hero.width * hero.height;
        expect(supporting.every((item) => heroArea > item.width * item.height)).toBe(true);
    });
});
```

- [ ] **Step 2: Run the tests to verify failure**

Run:

```bash
npx vitest run src/lib/preview-layout.test.ts
```

Expected: fail because `src/lib/preview-layout.ts` does not exist.

- [ ] **Step 3: Implement the utility**

Create `src/lib/preview-layout.ts`:

```ts
export const PREVIEW_SELECTED_SET_LIMIT = 30;

export type PreviewLayoutRecipe = 'single' | 'masonry' | 'magazine' | 'hero_strip' | 'dense_overview';

export interface PreviewLayoutInputImage {
    id: string;
    width: number;
    height: number;
}

export interface PreviewLayoutItem {
    id: string;
    x: number;
    y: number;
    width: number;
    height: number;
}

export interface PreviewLayoutResult {
    recipe: PreviewLayoutRecipe;
    width: number;
    height: number;
    items: PreviewLayoutItem[];
}

export function capPreviewSelection(ids: readonly string[]): string[] {
    return ids.slice(0, PREVIEW_SELECTED_SET_LIMIT);
}

export function orderPreviewSelection(selectedIds: ReadonlySet<string>, visibleGridOrder: readonly string[]): string[] {
    const visible = visibleGridOrder.filter((id) => selectedIds.has(id));
    const visibleSet = new Set(visible);
    const remainder = Array.from(selectedIds).filter((id) => !visibleSet.has(id));
    return capPreviewSelection([...visible, ...remainder]);
}

export function computePreviewLayout(options: {
    images: PreviewLayoutInputImage[];
    recipe: PreviewLayoutRecipe;
    width: number;
    height: number;
    gap?: number;
}): PreviewLayoutResult {
    const gap = options.gap ?? 8;
    const width = Math.max(1, options.width);
    const height = Math.max(1, options.height);
    const images = options.images.slice(0, PREVIEW_SELECTED_SET_LIMIT);
    const recipe = options.recipe;

    if (images.length === 0) return { recipe, width, height, items: [] };
    if (recipe === 'single' || images.length === 1) {
        return { recipe, width, height, items: [fitItem(images[0], 0, 0, width, height)] };
    }
    if (recipe === 'magazine') return { recipe, width, height, items: magazineLayout(images, width, height, gap) };
    if (recipe === 'hero_strip') return { recipe, width, height, items: heroStripLayout(images, width, height, gap) };
    if (recipe === 'dense_overview') return { recipe, width, height, items: denseOverviewLayout(images, width, height, gap) };
    return { recipe, width, height, items: masonryLayout(images, width, height, gap) };
}

function fitItem(image: PreviewLayoutInputImage, x: number, y: number, boxWidth: number, boxHeight: number): PreviewLayoutItem {
    const ratio = Math.min(boxWidth / Math.max(1, image.width), boxHeight / Math.max(1, image.height));
    const width = Math.max(1, Math.round(image.width * ratio));
    const height = Math.max(1, Math.round(image.height * ratio));
    return {
        id: image.id,
        x: Math.round(x + (boxWidth - width) / 2),
        y: Math.round(y + (boxHeight - height) / 2),
        width,
        height,
    };
}

function masonryLayout(images: PreviewLayoutInputImage[], width: number, height: number, gap: number): PreviewLayoutItem[] {
    const columns = Math.min(5, Math.max(2, Math.ceil(Math.sqrt(images.length))));
    const columnWidth = Math.floor((width - gap * (columns - 1)) / columns);
    const columnHeights = Array.from({ length: columns }, () => 0);
    const items: PreviewLayoutItem[] = [];

    for (const image of images) {
        const column = columnHeights.indexOf(Math.min(...columnHeights));
        const boxHeight = Math.max(80, Math.round(columnWidth * image.height / Math.max(1, image.width)));
        const x = column * (columnWidth + gap);
        const y = columnHeights[column];
        items.push({ id: image.id, x, y, width: columnWidth, height: boxHeight });
        columnHeights[column] += boxHeight + gap;
    }

    return scaleToViewport(items, width, height);
}

function magazineLayout(images: PreviewLayoutInputImage[], width: number, height: number, gap: number): PreviewLayoutItem[] {
    const heroWidth = Math.floor(width * 0.58);
    const sideWidth = width - heroWidth - gap;
    const hero = fitItem(images[0], 0, 0, heroWidth, height);
    const rest = denseOverviewLayout(images.slice(1), sideWidth, height, gap)
        .map((item) => ({ ...item, x: item.x + heroWidth + gap }));
    return [hero, ...rest];
}

function heroStripLayout(images: PreviewLayoutInputImage[], width: number, height: number, gap: number): PreviewLayoutItem[] {
    const heroHeight = Math.floor(height * 0.68);
    const hero = fitItem(images[0], 0, 0, width, heroHeight);
    const stripHeight = height - heroHeight - gap;
    const count = Math.max(1, images.length - 1);
    const cellWidth = Math.floor((width - gap * (count - 1)) / count);
    const strip = images.slice(1).map((image, index) =>
        fitItem(image, index * (cellWidth + gap), heroHeight + gap, cellWidth, stripHeight)
    );
    return [hero, ...strip];
}

function denseOverviewLayout(images: PreviewLayoutInputImage[], width: number, height: number, gap: number): PreviewLayoutItem[] {
    const columns = Math.ceil(Math.sqrt(images.length * width / Math.max(1, height)));
    const rows = Math.ceil(images.length / columns);
    const cellWidth = Math.floor((width - gap * (columns - 1)) / columns);
    const cellHeight = Math.floor((height - gap * (rows - 1)) / rows);
    return images.map((image, index) => {
        const column = index % columns;
        const row = Math.floor(index / columns);
        return fitItem(image, column * (cellWidth + gap), row * (cellHeight + gap), cellWidth, cellHeight);
    });
}

function scaleToViewport(items: PreviewLayoutItem[], width: number, height: number): PreviewLayoutItem[] {
    const maxY = Math.max(...items.map((item) => item.y + item.height), 1);
    const scale = Math.min(1, height / maxY);
    return items.map((item) => ({
        ...item,
        x: Math.round(item.x * scale),
        y: Math.round(item.y * scale),
        width: Math.max(1, Math.round(item.width * scale)),
        height: Math.max(1, Math.round(item.height * scale)),
    })).filter((item) => item.x + item.width <= width && item.y + item.height <= height);
}
```

- [ ] **Step 4: Run the tests to verify pass**

Run:

```bash
npx vitest run src/lib/preview-layout.test.ts
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/preview-layout.ts src/lib/preview-layout.test.ts
git commit -m "feat: add preview layout utilities"
```

## Task 2: Extend Preview State Types

**Files:**
- Modify: `src-tauri/src/preview/state.rs`
- Modify: `src-tauri/src/commands/preview.rs`
- Modify: `src/lib/api.ts`
- Test: `src-tauri/tests/preview_state.rs`

- [ ] **Step 1: Write failing Rust state tests**

Append to `src-tauri/tests/preview_state.rs`:

```rust
use cull::preview::state::{PreviewFeed, PreviewLayoutRecipe};

#[test]
fn preview_state_defaults_to_focused_single_layout() {
    let state = cull::preview::state::PreviewState::default();
    assert_eq!(state.feed, PreviewFeed::FocusedImage);
    assert_eq!(state.layout_recipe, PreviewLayoutRecipe::Single);
    assert!(state.selected_image_ids.is_empty());
}

#[test]
fn preview_state_caps_selected_image_ids() {
    let store = cull::preview::state::PreviewStateStore::default();
    let ids: Vec<String> = (0..34).map(|index| format!("img-{index}")).collect();
    let next = store.update(
        Some("focused".to_string()),
        cull::preview::state::PreviewDisplayMode::ImageOnly,
        cull::preview::state::PreviewOverlayConfig::default(),
        Some(false),
        Some(false),
        Some(PreviewFeed::SelectedSet),
        Some(PreviewLayoutRecipe::Masonry),
        Some(ids),
    );
    assert_eq!(next.feed, PreviewFeed::SelectedSet);
    assert_eq!(next.layout_recipe, PreviewLayoutRecipe::Masonry);
    assert_eq!(next.selected_image_ids.len(), 30);
    assert_eq!(next.selected_image_ids[29], "img-29");
}
```

- [ ] **Step 2: Run Rust tests to verify failure**

Run:

```bash
cd src-tauri && cargo test --test preview_state
```

Expected: fail because `PreviewFeed`, `PreviewLayoutRecipe`, and the extended `update` signature do not exist.

- [ ] **Step 3: Extend Rust preview state**

Modify `src-tauri/src/preview/state.rs` with these additions:

```rust
pub const PREVIEW_SELECTED_SET_LIMIT: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewFeed {
    FocusedImage,
    SelectedSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewLayoutRecipe {
    Single,
    Masonry,
    Magazine,
    HeroStrip,
    DenseOverview,
}
```

Add fields to `PreviewState`:

```rust
pub feed: PreviewFeed,
pub layout_recipe: PreviewLayoutRecipe,
pub selected_image_ids: Vec<String>,
```

Set defaults in `impl Default for PreviewState`:

```rust
feed: PreviewFeed::FocusedImage,
layout_recipe: PreviewLayoutRecipe::Single,
selected_image_ids: Vec::new(),
```

Change `PreviewStateStore::update` to accept and apply the optional fields:

```rust
pub fn update(
    &self,
    image_id: Option<String>,
    display_mode: PreviewDisplayMode,
    overlay: PreviewOverlayConfig,
    frozen: Option<bool>,
    blanked: Option<bool>,
    feed: Option<PreviewFeed>,
    layout_recipe: Option<PreviewLayoutRecipe>,
    selected_image_ids: Option<Vec<String>>,
) -> PreviewState {
    let mut state = self.state.lock();
    state.image_id = image_id;
    state.display_mode = display_mode;
    state.overlay = overlay;
    if let Some(frozen) = frozen {
        state.frozen = frozen;
    }
    if let Some(blanked) = blanked {
        state.blanked = blanked;
    }
    if let Some(feed) = feed {
        state.feed = feed;
    }
    if let Some(layout_recipe) = layout_recipe {
        state.layout_recipe = layout_recipe;
    }
    if let Some(selected_image_ids) = selected_image_ids {
        state.selected_image_ids = selected_image_ids
            .into_iter()
            .take(PREVIEW_SELECTED_SET_LIMIT)
            .collect();
    }
    state.version += 1;
    state.updated_at_ms = current_time_ms();
    state.clone()
}
```

- [ ] **Step 4: Extend preview command arguments**

Modify imports in `src-tauri/src/commands/preview.rs`:

```rust
use crate::preview::state::{
    PreviewDisplayMode, PreviewFeed, PreviewLayoutRecipe, PreviewOverlayConfig, PreviewState,
};
```

Extend `update_preview_state` parameters and call:

```rust
feed: Option<PreviewFeed>,
layout_recipe: Option<PreviewLayoutRecipe>,
selected_image_ids: Option<Vec<String>>,
```

```rust
.update(
    image_id,
    display_mode,
    overlay,
    frozen,
    blanked,
    feed,
    layout_recipe,
    selected_image_ids,
);
```

- [ ] **Step 5: Extend TypeScript API types**

Modify `src/lib/api.ts`:

```ts
export type PreviewFeed = 'focused_image' | 'selected_set';
export type PreviewLayoutRecipe = 'single' | 'masonry' | 'magazine' | 'hero_strip' | 'dense_overview';
```

Add to `PreviewState`:

```ts
    feed: PreviewFeed;
    layout_recipe: PreviewLayoutRecipe;
    selected_image_ids: string[];
```

Extend `updatePreviewState`:

```ts
export async function updatePreviewState(
    imageId: string | null,
    displayMode: PreviewDisplayMode,
    overlay: PreviewOverlayConfig,
    frozen?: boolean,
    blanked?: boolean,
    feed?: PreviewFeed,
    layoutRecipe?: PreviewLayoutRecipe,
    selectedImageIds?: string[]
): Promise<PreviewState> {
    return invoke<PreviewState>('update_preview_state', {
        imageId,
        displayMode,
        overlay,
        frozen,
        blanked,
        feed,
        layoutRecipe,
        selectedImageIds,
    });
}
```

- [ ] **Step 6: Run tests**

Run:

```bash
cd src-tauri && cargo test --test preview_state
npx vitest run src/lib/preview-display-utils.test.ts src/lib/preview-display-controls.test.ts
```

If TypeScript fixtures fail because `PreviewState` needs the new fields, add these fields to every local `PreviewState` literal in the failing test file:

```ts
feed: 'focused_image',
layout_recipe: 'single',
selected_image_ids: [],
```

Expected: all focused tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/preview/state.rs src-tauri/src/commands/preview.rs src-tauri/tests/preview_state.rs src/lib/api.ts src/lib/preview-display-utils.test.ts src/lib/preview-display-controls.test.ts
git commit -m "feat: extend preview display state for selected sets"
```

## Task 3: Sync Selected Sets From Main Window

**Files:**
- Modify: `src/lib/preview-display.ts`
- Modify: `src/lib/preview-display-store.ts`
- Modify: `src/routes/+page.svelte`
- Test: `src/lib/preview-display-utils.test.ts`

- [ ] **Step 1: Write failing helper tests**

Append to `src/lib/preview-display-utils.test.ts`:

```ts
import { orderPreviewSelection } from './preview-layout';
import {
    nextPreviewSelectedSetPayload,
    previewSyncSelectedImageIds,
} from './preview-display';

it('builds selected-set payload from visible grid order', () => {
    const selected = new Set(['c', 'a']);
    const visible = ['a', 'b', 'c'];
    expect(previewSyncSelectedImageIds(selected, visible)).toEqual(['a', 'c']);
    expect(orderPreviewSelection(selected, visible)).toEqual(['a', 'c']);
});

it('does not update selected ids while preview is frozen', () => {
    const current = {
        image_id: 'a',
        display_mode: 'image_only',
        overlay: DEFAULT_PREVIEW_OVERLAY,
        frozen: true,
        blanked: false,
        version: 2,
        updated_at_ms: 1,
        feed: 'selected_set',
        layout_recipe: 'masonry',
        selected_image_ids: ['old'],
    } as const;
    expect(nextPreviewSelectedSetPayload(new Set(['new']), ['new'], current, true, false).selectedImageIds)
        .toEqual(['old']);
});
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
npx vitest run src/lib/preview-display-utils.test.ts
```

Expected: fail because helpers do not exist.

- [ ] **Step 3: Add stores and parsers**

Modify `src/lib/preview-display-store.ts`:

```ts
import type { PreviewFeed, PreviewLayoutRecipe } from './api';

export const PREVIEW_DISPLAY_FEED_SETTING = 'preview_display_feed';
export const PREVIEW_DISPLAY_LAYOUT_RECIPE_SETTING = 'preview_display_layout_recipe';

export const previewDisplayFeed = writable<PreviewFeed>('focused_image');
export const previewDisplayLayoutRecipe = writable<PreviewLayoutRecipe>('single');

export function setPreviewDisplayFeed(feed: PreviewFeed) {
    previewDisplayFeed.set(feed);
}

export function setPreviewDisplayLayoutRecipe(recipe: PreviewLayoutRecipe) {
    previewDisplayLayoutRecipe.set(recipe);
}

export function parsePreviewFeed(value: string | null): PreviewFeed {
    return value === 'selected_set' ? 'selected_set' : 'focused_image';
}

export function parsePreviewLayoutRecipe(value: string | null): PreviewLayoutRecipe {
    if (value === 'masonry' || value === 'magazine' || value === 'hero_strip' || value === 'dense_overview') return value;
    return 'single';
}
```

- [ ] **Step 4: Add sync helpers**

Modify `src/lib/preview-display.ts`:

```ts
import { orderPreviewSelection } from './preview-layout';
import type { PreviewFeed, PreviewLayoutRecipe } from './api';

export function previewSyncSelectedImageIds(
    selectedIds: ReadonlySet<string>,
    visibleGridOrder: readonly string[],
): string[] {
    return orderPreviewSelection(selectedIds, visibleGridOrder);
}

export function nextPreviewSelectedSetPayload(
    selectedIds: ReadonlySet<string>,
    visibleGridOrder: readonly string[],
    current: PreviewState | null,
    frozen: boolean,
    blanked: boolean,
): { selectedImageIds: string[]; feed: PreviewFeed; layoutRecipe: PreviewLayoutRecipe } {
    const feed = current?.feed ?? 'focused_image';
    const layoutRecipe = current?.layout_recipe ?? 'single';
    if (blanked) return { selectedImageIds: [], feed, layoutRecipe };
    if (frozen) return { selectedImageIds: current?.selected_image_ids ?? [], feed, layoutRecipe };
    return { selectedImageIds: previewSyncSelectedImageIds(selectedIds, visibleGridOrder), feed, layoutRecipe };
}
```

- [ ] **Step 5: Sync selected IDs in `+page.svelte`**

Modify imports in `src/routes/+page.svelte` to include new stores and helper:

```ts
previewDisplayFeed,
previewDisplayLayoutRecipe,
```

```ts
import { nextPreviewSelectedSetPayload, previewSyncImageId } from '$lib/preview-display';
```

In `syncFocusedImageToPreviewDisplay`, compute visible order and pass the new args:

```ts
const visibleGridOrder = $images.map((item) => item.image.id);
const selectedPayload = nextPreviewSelectedSetPayload(
    $selectedIds,
    visibleGridOrder,
    previewSyncState,
    $previewDisplayFrozen,
    $previewDisplayBlanked,
);
previewSyncState = await updatePreviewState(
    imageId,
    $previewDisplayMode ?? payload.displayMode,
    $previewDisplayOverlay ?? payload.overlay,
    $previewDisplayFrozen,
    $previewDisplayBlanked,
    $previewDisplayFeed,
    $previewDisplayLayoutRecipe,
    selectedPayload.selectedImageIds,
);
```

Add `$previewDisplayFeed`, `$previewDisplayLayoutRecipe`, and `$selectedIds` to the sync key.

Use this sync-key shape:

```ts
const syncKey = JSON.stringify({
    imageId,
    selectedImageIds: selectedPayload.selectedImageIds,
    displayMode: $previewDisplayMode,
    overlay: $previewDisplayOverlay,
    feed: $previewDisplayFeed,
    layoutRecipe: $previewDisplayLayoutRecipe,
    frozen: $previewDisplayFrozen,
    blanked: $previewDisplayBlanked,
    alwaysOnTop: $previewDisplayAlwaysOnTop,
});
```

- [ ] **Step 6: Run tests**

Run:

```bash
npx vitest run src/lib/preview-display-utils.test.ts
npm run check
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src/lib/preview-display.ts src/lib/preview-display-store.ts src/routes/+page.svelte src/lib/preview-display-utils.test.ts
git commit -m "feat: sync selected images to preview display"
```

## Task 4: Render Multi-image Preview Display

**Files:**
- Modify: `src/lib/components/PreviewDisplay.svelte`
- Test: `src/lib/preview-display-ui-contract.test.ts`

- [ ] **Step 1: Write failing UI contract tests**

Append to `src/lib/preview-display-ui-contract.test.ts`:

```ts
it('renders selected-set preview layouts through the layout utility', () => {
    const component = readFileSync(join(root, 'src/lib/components/PreviewDisplay.svelte'), 'utf8');
    expect(component).toContain('computePreviewLayout');
    expect(component).toContain('preview-layout-stage');
    expect(component).toContain('preview-layout-item');
    expect(component).toContain('selected_image_ids');
});
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
npx vitest run src/lib/preview-display-ui-contract.test.ts
```

Expected: fail until the component imports and uses the layout utility.

- [ ] **Step 3: Add multi-image state and loading**

Modify `src/lib/components/PreviewDisplay.svelte`:

```ts
import { computePreviewLayout, type PreviewLayoutItem } from '$lib/preview-layout';
```

Add state:

```ts
let selectedImages = $state<ImageWithFile[]>([]);
let stageEl: HTMLElement | undefined = $state(undefined);
let stageWidth = $state(1280);
let stageHeight = $state(720);
let layoutItems = $derived.by(() => {
    if (!previewState || previewState.feed !== 'selected_set') return [];
    return computePreviewLayout({
        images: selectedImages.map((item) => ({
            id: item.image.id,
            width: item.image.width,
            height: item.image.height,
        })),
        recipe: previewState.layout_recipe,
        width: stageWidth,
        height: stageHeight,
        gap: 8,
    }).items;
});
```

In `applyPreviewState`, before single-image load, load selected images when feed is selected-set:

```ts
if (next.feed === 'selected_set' && next.selected_image_ids.length > 0 && !next.blanked) {
    const seq = ++requestSeq;
    loadState = 'loading';
    try {
        const records = await getImagesByIds(next.selected_image_ids);
        if (seq !== requestSeq) return;
        const byId = new Map(records.map((record) => [record.image.id, record]));
        selectedImages = next.selected_image_ids.map((id) => byId.get(id)).filter(Boolean) as ImageWithFile[];
        image = selectedImages[0] ?? null;
        loadState = selectedImages.length > 0 ? 'ready' : 'missing';
    } catch (e) {
        if (seq !== requestSeq) return;
        console.error('Failed to load Preview Display selected set:', e);
        selectedImages = [];
        image = null;
        loadState = 'error';
    }
    return;
}
selectedImages = [];
```

Add a ResizeObserver effect for `stageEl`:

```ts
$effect(() => {
    if (!stageEl) return;
    const ro = new ResizeObserver((entries) => {
        for (const entry of entries) {
            stageWidth = Math.max(1, Math.round(entry.contentRect.width));
            stageHeight = Math.max(1, Math.round(entry.contentRect.height));
        }
    });
    ro.observe(stageEl);
    return () => ro.disconnect();
});
```

- [ ] **Step 4: Add selected-set markup**

Bind the stage:

```svelte
<main class="preview-stage" bind:this={stageEl}>
```

Before the single image branch:

```svelte
{#if loadState === 'ready' && previewState?.feed === 'selected_set' && selectedImages.length > 0}
    <div class="preview-layout-stage" data-recipe={previewState.layout_recipe}>
        {#each layoutItems as item (item.id)}
            {@const selected = selectedImages.find((record) => record.image.id === item.id)}
            {#if selected}
                <img
                    class="preview-layout-item"
                    src={convertFileSrc(previewDisplayImageSourcePath(selected, false))}
                    alt={selected.path.split('/').pop() ?? selected.image.id}
                    draggable="false"
                    style={`left:${item.x}px;top:${item.y}px;width:${item.width}px;height:${item.height}px;`}
                />
            {/if}
        {/each}
    </div>
{:else if loadState === 'ready' && image}
```

Add CSS:

```css
.preview-layout-stage {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
}
.preview-layout-item {
    position: absolute;
    display: block;
    object-fit: cover;
    background: var(--surface);
}
.preview-layout-stage[data-recipe="single"] .preview-layout-item {
    object-fit: contain;
}
```

- [ ] **Step 5: Run checks**

Run:

```bash
npx vitest run src/lib/preview-display-ui-contract.test.ts
npm run check
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/PreviewDisplay.svelte src/lib/preview-display-ui-contract.test.ts
git commit -m "feat: render selected-set preview layouts"
```

## Task 5: Commands, Menus, and Shortcuts

**Files:**
- Modify: `src/lib/keys.ts`
- Modify: `src/lib/menu.ts`
- Modify: `src/lib/preview-display-store.ts`
- Modify: `src-tauri/src/menu.rs`
- Modify: `src/routes/+page.svelte`
- Test: `src/lib/menu.test.ts`
- Test: `src/lib/keys-utils.test.ts`

- [ ] **Step 1: Write failing contract tests**

Add to `src/lib/menu.test.ts`:

```ts
it('handles selected-set preview display menu commands', () => {
    const source = readFileSync(join(root, 'src/lib/menu.ts'), 'utf8');
    expect(source).toContain('preview_display_feed_selected_set');
    expect(source).toContain('preview_display_layout_next');
});
```

Add to `src/lib/keys-utils.test.ts`:

```ts
it('documents preview display layout cycle shortcut in key handling source', () => {
    const source = readFileSync(join(root, 'src/lib/keys.ts'), 'utf8');
    expect(source).toContain('cycle-preview-layout');
    expect(source).toContain("event.key.toLowerCase() === 'l'");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
npx vitest run src/lib/menu.test.ts src/lib/keys-utils.test.ts
```

Expected: fail until menu and key handling are added.

- [ ] **Step 3: Add Rust menu items**

Modify `src-tauri/src/menu.rs` inside the Preview Display submenu:

```rust
let preview_display_feed_menu = Submenu::new(app, "Feed", true)?;
preview_display_feed_menu.append(&CheckMenuItem::with_id(
    app,
    "preview_display_feed_focused_image",
    "Focused Image",
    true,
    true,
    None::<&str>,
)?)?;
preview_display_feed_menu.append(&CheckMenuItem::with_id(
    app,
    "preview_display_feed_selected_set",
    "Selected Set",
    true,
    false,
    None::<&str>,
)?)?;
preview_display_menu.append(&preview_display_feed_menu)?;

let preview_display_layout_menu = Submenu::new(app, "Layout", true)?;
preview_display_layout_menu.append(&MenuItem::with_id(
    app,
    "preview_display_layout_next",
    "Next Layout",
    true,
    Some("CmdOrCtrl+Shift+L"),
)?)?;
preview_display_menu.append(&preview_display_layout_menu)?;
```

Extend the menu state payload and checked-state refresh for feed items:

```rust
preview_display_feed: String,
preview_display_layout_recipe: String,
```

```rust
set_menu_item_checked(&app, "preview_display_feed_focused_image", state.preview_display_feed == "focused_image")?;
set_menu_item_checked(&app, "preview_display_feed_selected_set", state.preview_display_feed == "selected_set")?;
```

- [ ] **Step 4: Add frontend menu handling**

Modify `src/lib/menu.ts`:

```ts
case 'preview_display_feed_focused_image':
    setPreviewDisplayFeed('focused_image');
    break;
case 'preview_display_feed_selected_set':
    setPreviewDisplayFeed('selected_set');
    break;
case 'preview_display_layout_next':
    cyclePreviewDisplayLayoutRecipe();
    break;
```

Add the layout cycle helper to `src/lib/preview-display-store.ts`:

```ts
const PREVIEW_LAYOUT_RECIPES: PreviewLayoutRecipe[] = ['masonry', 'magazine', 'hero_strip', 'dense_overview'];

export function cyclePreviewDisplayLayoutRecipe() {
    previewDisplayLayoutRecipe.update((current) => {
        const index = PREVIEW_LAYOUT_RECIPES.indexOf(current);
        return PREVIEW_LAYOUT_RECIPES[(index + 1) % PREVIEW_LAYOUT_RECIPES.length];
    });
}
```

Add imports to `src/lib/menu.ts`:

```ts
import {
    cyclePreviewDisplayLayoutRecipe,
    setPreviewDisplayFeed,
} from './preview-display-store';
```

Add the two symbols to the existing `./preview-display-store` import list if that import already exists in the file. Do not define a second local recipe list in `src/lib/menu.ts`; route and menu handling must share the store helper.

- [ ] **Step 5: Add keyboard handling**

Modify `src/lib/keys.ts`:

```ts
if (event.metaKey && event.shiftKey && event.key.toLowerCase() === 'l') {
    event.preventDefault();
    window.dispatchEvent(new CustomEvent('cycle-preview-layout'));
    return;
}
```

In `src/routes/+page.svelte`, listen for the event:

```ts
import { cyclePreviewDisplayLayoutRecipe } from '$lib/preview-display-store';

const handleCyclePreviewLayout = () => cyclePreviewDisplayLayoutRecipe();
window.addEventListener('cycle-preview-layout', handleCyclePreviewLayout);
```

Clean up in the existing `onMount` return:

```ts
window.removeEventListener('cycle-preview-layout', handleCyclePreviewLayout);
```

- [ ] **Step 6: Run checks**

Run:

```bash
npx vitest run src/lib/menu.test.ts src/lib/keys-utils.test.ts
npm run check
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/menu.rs src/lib/menu.ts src/lib/preview-display-store.ts src/lib/keys.ts src/routes/+page.svelte src/lib/menu.test.ts src/lib/keys-utils.test.ts
git commit -m "feat: add selected-set preview controls"
```

## Task 6: Persistence, Docs, and Final Verification

**Files:**
- Modify: `src/routes/+page.svelte`
- Modify: `docs/preview-display.md`
- Modify: `docs/USER_GUIDE.md`
- Test: `src/lib/preview-display-controls-contract.test.ts`

- [ ] **Step 1: Write failing docs/settings contract**

Append to `src/lib/preview-display-controls-contract.test.ts`:

```ts
it('documents selected-set preview display controls', () => {
    const docs = readFileSync(join(root, 'docs/preview-display.md'), 'utf8');
    expect(docs).toContain('Selected Set');
    expect(docs).toContain('30 images');
    expect(docs).toContain('Next Layout');
});
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
npx vitest run src/lib/preview-display-controls-contract.test.ts
```

Expected: fail until docs are updated.

- [ ] **Step 3: Restore and persist settings**

In `src/routes/+page.svelte`, extend `restorePreviewDisplaySettings`:

```ts
setPreviewDisplayFeed(parsePreviewFeed(await getAppSetting(PREVIEW_DISPLAY_FEED_SETTING)));
setPreviewDisplayLayoutRecipe(parsePreviewLayoutRecipe(await getAppSetting(PREVIEW_DISPLAY_LAYOUT_RECIPE_SETTING)));
```

Add effects near existing preview setting persistence:

```ts
$effect(() => {
    if (previewDisplayWindow) return;
    void setAppSetting(PREVIEW_DISPLAY_FEED_SETTING, $previewDisplayFeed);
});

$effect(() => {
    if (previewDisplayWindow) return;
    void setAppSetting(PREVIEW_DISPLAY_LAYOUT_RECIPE_SETTING, $previewDisplayLayoutRecipe);
});
```

- [ ] **Step 4: Update docs**

Add to `docs/preview-display.md` under Presentation Controls:

```md
- **Focused Image** and **Selected Set** feeds: choose whether Preview Display follows the focused image or recomposes the current Grid selection.
- **Next Layout** (`Cmd+Shift+L`): cycles Selected Set through masonry, magazine, hero strip, and dense overview layouts.

Selected Set displays at most 30 images. If more are selected, Cull uses the first 30 in deterministic Grid/selection order. The cap keeps the presentation readable and prevents heavy decode spikes on external displays.
```

Add to `docs/USER_GUIDE.md` in the Preview Display section:

```md
Use the Preview Display feed controls to switch from the focused image to the selected Grid set. Selected Set mode recomposes up to 30 selected images into live presentation layouts; use **Next Layout** or `Cmd+Shift+L` to cycle the arrangement.
```

- [ ] **Step 5: Run focused checks**

Run:

```bash
npx vitest run src/lib/preview-layout.test.ts src/lib/preview-display-utils.test.ts src/lib/preview-display-ui-contract.test.ts src/lib/preview-display-controls-contract.test.ts
npm run check
cd src-tauri && cargo fmt --all -- --check && cargo test --test preview_state
```

Expected: pass.

- [ ] **Step 6: Run full project gate**

Run:

```bash
npm run preflight -- full
```

Expected: pass. Existing clippy warnings may print because the project does not run clippy with `-D warnings`; new compile, format, or test failures must be fixed.

- [ ] **Step 7: Commit**

```bash
git add src/routes/+page.svelte docs/preview-display.md docs/USER_GUIDE.md src/lib/preview-display-controls-contract.test.ts
git commit -m "docs: document selected-set preview display"
```

## Execution Notes

- Keep the first implementation to one Preview Display surface. The plan intentionally does not implement bounded multi-surface preview windows.
- Keep Canvas out of the implementation. Add only the state shape that would allow a future "Save as Canvas" command.
- Do not add a Tauri mock import to `src/lib/api.ts` or any app component.
- Use `trash`, not `rm`, for any local cleanup during execution.
- Before landing, run `npm run land` from a clean worktree. If unrelated staged beads changes exist, do not include them in feature commits.
