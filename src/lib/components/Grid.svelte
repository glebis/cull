<script lang="ts">
    import { onDestroy } from 'svelte';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { images, selectedIds, selectionAnchorIndex, focusedIndex, thumbnailSize, viewMode, gridGap, gridScrollTop, navigateTo, imageLoadState } from '$lib/stores';
    import { IMAGE_PAGE_SIZE, loadMoreImagesForCurrentScope } from '$lib/image-loading';
    import {
        computeGridClickSelection,
        computeGridLayout,
        computeVisibleItems,
        computeScrollDirection,
        computeOverscan,
        computePrefetchIndices,
        safeAssetPreviewPath,
        type ScrollDirection,
    } from '$lib/view-utils';
    import { createPrefetchCache } from '$lib/prefetch-cache';
    import Thumbnail from './Thumbnail.svelte';

    let containerEl: HTMLDivElement | undefined = $state(undefined);
    let containerWidth = $state(800);
    let containerHeight = $state(600);
    let scrollTop = $state(0);
    let scrollRestoreSeq = 0;

    let size = $state(160);
    thumbnailSize.subscribe(v => size = v);

    let gap = $state(4);
    gridGap.subscribe(v => gap = v);

    let layout = $derived(computeGridLayout(containerWidth, size, gap, $images.length));
    let cols = $derived(layout.cols);
    let cellSize = $derived(layout.cellSize);
    let totalHeight = $derived(layout.totalHeight);
    let preloadRows = $derived(Math.max(2, Math.ceil(IMAGE_PAGE_SIZE / Math.max(cols, 1))));

    // Scroll-direction-aware prefetch + bounded decode-warming cache (P1).
    const dpr = typeof window !== 'undefined' ? (window.devicePixelRatio || 1) : 1;
    let prevScrollTop = 0;
    let scrollDir = $state<ScrollDirection>('none');
    // Bound warmed images to a few screens' worth; evicted entries release their decode.
    const prefetch = createPrefetchCache(300);

    let overscan = $derived(computeOverscan(scrollDir, preloadRows));

    let visibleItems = $derived.by(() => {
        const imgs = $images;
        return computeVisibleItems(scrollTop, containerHeight, layout.cols, layout.cellSize, imgs.length, {
            overscanRowsBefore: overscan.before,
            overscanRowsAfter: overscan.after,
        })
            .map(({ index, x, y }) => ({ index, item: imgs[index], x, y }));
    });

    function warmPrefetch() {
        if (cellSize <= 0 || cols <= 0) return;
        const imgs = $images;
        const indices = computePrefetchIndices(
            scrollTop,
            containerHeight,
            cols,
            cellSize,
            imgs.length,
            scrollDir,
            Math.max(2, preloadRows),
        );
        for (const i of indices) {
            const item = imgs[i];
            if (!item) continue;
            const previewPath = safeAssetPreviewPath(item, { displayPx: size, dpr });
            if (previewPath) prefetch.warm(convertFileSrc(previewPath));
        }
    }

    // Release warmed images when the scope changes (first item identity changes).
    let prevScopeKey: string | null = null;
    $effect(() => {
        const imgs = $images;
        const scopeKey = imgs.length > 0 ? imgs[0].image.id : null;
        if (scopeKey !== prevScopeKey) {
            prevScopeKey = scopeKey;
            prefetch.clear();
        }
    });

    onDestroy(() => prefetch.clear());

    function maybeLoadMore() {
        if (!$imageLoadState.hasMore || $imageLoadState.loading || $imageLoadState.loadingMore) return;
        if (cellSize <= 0) return;
        const remainingPx = totalHeight - (scrollTop + containerHeight);
        if (remainingPx < cellSize * preloadRows) {
            void loadMoreImagesForCurrentScope();
        }
    }

    function onScroll(e: Event) {
        const nextScrollTop = (e.target as HTMLDivElement).scrollTop;
        scrollDir = computeScrollDirection(prevScrollTop, nextScrollTop, scrollDir);
        prevScrollTop = nextScrollTop;
        scrollTop = nextScrollTop;
        gridScrollTop.set(scrollTop);
        maybeLoadMore();
        warmPrefetch();
    }

    function handleClick(index: number, event: MouseEvent | KeyboardEvent) {
        const result = computeGridClickSelection({
            items: $images,
            selectedIds: $selectedIds,
            focusedIndex: $focusedIndex,
            anchorIndex: $selectionAnchorIndex,
            targetIndex: index,
            shiftKey: event.shiftKey,
            toggleKey: event.altKey || event.metaKey,
            getId: item => item.image.id,
        });

        if (result.selectedIds) selectedIds.set(result.selectedIds);
        selectionAnchorIndex.set(result.anchorIndex);
        focusedIndex.set(index);
    }

    function handleDblClick(index: number) {
        focusedIndex.set(index);
        navigateTo('loupe');
    }

    $effect(() => {
        if (!containerEl) return;
        const ro = new ResizeObserver((entries) => {
            for (const entry of entries) {
                containerWidth = entry.contentRect.width;
                containerHeight = entry.contentRect.height;
            }
        });
        ro.observe(containerEl);
        return () => ro.disconnect();
    });

    $effect(() => {
        if (!containerEl) return;
        totalHeight;
        containerHeight;
        cellSize;
        preloadRows;
        $images.length;
        $imageLoadState.hasMore;
        $imageLoadState.loading;
        $imageLoadState.loadingMore;
        maybeLoadMore();
        warmPrefetch();
    });

    $effect(() => {
        if (!containerEl) return;
        const target = $gridScrollTop;
        $images.length;
        if (Math.abs(containerEl.scrollTop - target) <= 1) {
            scrollTop = containerEl.scrollTop;
            return;
        }
        const seq = ++scrollRestoreSeq;
        requestAnimationFrame(() => {
            if (!containerEl) return;
            if (seq !== scrollRestoreSeq) return;
            if (Math.abs(containerEl.scrollTop - target) > 1) {
                containerEl.scrollTop = target;
                scrollTop = containerEl.scrollTop;
            }
        });
    });

    let prevFocusedIndex = $state<number | null>(null);
    $effect(() => {
        const idx = $focusedIndex;
        if (idx === prevFocusedIndex) return;
        prevFocusedIndex = idx;
        const el = containerEl?.querySelector('[tabindex="0"]') as HTMLElement | null;
        el?.focus();
        if (idx >= $images.length - cols * 4) {
            maybeLoadMore();
        }
    });
</script>

<div
    class="grid-container"
    bind:this={containerEl}
    onscroll={onScroll}
    role="grid"
    aria-label={"Image grid, " + $images.length + " images"}
>
    {#if $images.length === 0}
        <div class="empty">
            <div class="empty-icon">&#9776;</div>
            <div class="empty-text">No images loaded</div>
            <div class="empty-hint">Use the sidebar to import a folder</div>
        </div>
    {:else}
        <div class="grid-scroll" style="height: {totalHeight}px; position: relative;">
            {#each visibleItems.filter(vi => vi.item) as vi (vi.item.image.id)}
                <div
                    class="grid-cell"
                    style="position: absolute; left: {vi.x}px; top: {vi.y}px; width: {size}px; height: {size}px;"
                    data-agent-image-id={vi.item.image.id}
                    data-agent-filename={vi.item.path.split('/').filter(Boolean).pop() ?? vi.item.image.id}
                    data-agent-path={vi.item.path}
                    data-agent-thumbnail-path={vi.item.thumbnail_path ?? ''}
                    data-agent-rating={vi.item.selection?.star_rating ?? ''}
                    data-agent-decision={vi.item.selection?.decision ?? 'undecided'}
                    data-agent-selected={$selectedIds.has(vi.item.image.id)}
                    data-agent-focused={$focusedIndex === vi.index}
                    data-agent-view-role="grid-cell"
                >
                    <Thumbnail
                        item={vi.item}
                        {size}
                        focused={$focusedIndex === vi.index}
                        selected={$selectedIds.has(vi.item.image.id)}
                        onclick={(event) => handleClick(vi.index, event)}
                        ondblclick={() => handleDblClick(vi.index)}
                        loading="eager"
                    />
                </div>
            {/each}
        </div>
        {#if $imageLoadState.loadingMore}
            <div class="load-indicator" aria-live="polite">Loading</div>
        {/if}
    {/if}
</div>

<style>
    .grid-container {
        grid-area: main;
        overflow-y: auto;
        overflow-x: hidden;
        background: var(--bg);
        padding: 4px;
    }
    .grid-scroll {
        width: 100%;
    }
    .empty {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100%;
        gap: 8px;
    }
    .empty-icon {
        font-size: 48px;
        color: var(--border);
    }
    .empty-text {
        font-size: 16px;
        color: var(--text-secondary);
    }
    .empty-hint {
        font-size: 12px;
        color: var(--text-secondary);
        opacity: 0.6;
    }
    .load-indicator {
        position: sticky;
        bottom: 8px;
        left: 50%;
        width: max-content;
        margin: 0 auto;
        padding: 4px 8px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--text-secondary);
        font-size: 11px;
        pointer-events: none;
    }
</style>
