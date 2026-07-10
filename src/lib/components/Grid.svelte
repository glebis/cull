<script lang="ts">
    import { onDestroy } from 'svelte';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { open } from '@tauri-apps/plugin-dialog';
    import { images, selectedIds, selectionAnchorIndex, focusedIndex, thumbnailSize, viewMode, gridGap, gridScrollTop, navigateTo, imageLoadState, showToast, totalCount, folders, gridPreset, GRID_PRESETS, activeSmartCollection, activeCollection, activeDetectedClass, activeFolder, minSizeFilter, clipboardMonitorStatus } from '$lib/stores';
    import { importFolder as apiImportFolder, getImageCount, listFolders } from '$lib/api';
    import { IMAGE_PAGE_SIZE, loadImagesForCurrentScope, loadMoreImagesForCurrentScope } from '$lib/image-loading';
    import { wheelGestureIntent } from '$lib/gesture-interactions';
    import { gridGestureZoom } from '$lib/grid-gesture-zoom';
    import { resolveLibraryViewState, scopeEmptyCopy, type LibraryScopeKind } from '$lib/library-view-state';
    import {
        computeGridClickSelection,
        computeAnchoredGridScrollTop,
        computeGridLayout,
        computeVisibleItems,
        computeScrollDirection,
        computeOverscan,
        computePrefetchIndices,
        safeAssetPreviewPath,
        type ScrollDirection,
    } from '$lib/view-utils';
    import { createPrefetchCache } from '$lib/prefetch-cache';
    import clipboardMonitorEmptySrc from '$lib/assets/clipboard-monitor-empty.png';
    import Thumbnail from './Thumbnail.svelte';

    let containerEl: HTMLDivElement | undefined = $state(undefined);
    let containerWidth = $state(800);
    let containerHeight = $state(600);
    let scrollTop = $state(0);
    let scrollRestoreSeq = 0;
    let pendingGridAnchor: { x: number; y: number } | null = null;
    let previousGridLayout: {
        size: number;
        gap: number;
        cols: number;
        cellSize: number;
        scrollTop: number;
        containerWidth: number;
        containerHeight: number;
    } | null = null;

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

    function handleWheel(e: WheelEvent) {
        if (!containerEl) return;
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
            viewportHeight: containerHeight || containerEl.clientHeight,
            target: e.target,
        });
        if (!intent || intent.type !== 'zoom') return;
        e.preventDefault();
        applyThumbnailZoom(intent.factor, e.clientX, e.clientY);
    }

    function applyThumbnailZoom(factor: number, clientX: number, clientY: number) {
        if (!containerEl) return;
        const rect = containerEl.getBoundingClientRect();
        const pointerX = clientX - rect.left;
        const pointerY = clientY - rect.top;
        const next = gridGestureZoom({ size, gap, preset: $gridPreset }, factor, GRID_PRESETS);
        if (next.size === size && next.gap === gap && next.preset === $gridPreset) return;

        pendingGridAnchor = { x: pointerX, y: pointerY };
        thumbnailSize.set(next.size);
        gridPreset.set(next.preset);
        gridGap.set(next.gap);
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
        const current = {
            size,
            gap,
            cols,
            cellSize,
            scrollTop,
            containerWidth,
            containerHeight,
        };
        const previous = previousGridLayout;
        previousGridLayout = current;
        if (!previous) {
            pendingGridAnchor = null;
            return;
        }

        const layoutChanged =
            previous.size !== size ||
            previous.gap !== gap ||
            previous.cols !== cols ||
            previous.cellSize !== cellSize;
        if (!layoutChanged) {
            pendingGridAnchor = null;
            return;
        }

        const anchor = pendingGridAnchor ?? {
            x: previous.containerWidth / 2,
            y: previous.containerHeight / 2,
        };
        pendingGridAnchor = null;
        const nextScrollTop = computeAnchoredGridScrollTop({
            oldScrollTop: previous.scrollTop,
            viewportWidth: previous.containerWidth,
            viewportHeight: previous.containerHeight,
            anchorX: anchor.x,
            anchorY: anchor.y,
            oldCols: previous.cols,
            oldCellSize: previous.cellSize,
            newCols: cols,
            newCellSize: cellSize,
            totalItems: $images.length,
        });

        gridScrollTop.set(nextScrollTop);
        requestAnimationFrame(() => {
            if (!containerEl) return;
            containerEl.scrollTop = nextScrollTop;
            scrollTop = containerEl.scrollTop;
            prevScrollTop = scrollTop;
        });
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

    // Same precedence as currentScope() in image-loading.ts, so the empty
    // state describes what the user is actually looking at.
    let scopeKind = $derived.by<LibraryScopeKind>(() => {
        if ($activeSmartCollection?.filter_json) return 'smart';
        if ($activeCollection) return 'collection';
        if ($activeDetectedClass) return 'detected-class';
        if ($activeFolder) return 'folder';
        if ($minSizeFilter > 0) return 'filtered';
        return 'all';
    });

    let libraryViewState = $derived(resolveLibraryViewState({
        loading: $imageLoadState.loading,
        error: $imageLoadState.error,
        loaded: $imageLoadState.loaded,
        imageCount: $images.length,
        scopeKind,
    }));
    let isClipboardMonitorEmpty = $derived(Boolean(
        $activeCollection && $clipboardMonitorStatus?.collection_id === $activeCollection,
    ));

    let scopeCopy = $derived(scopeEmptyCopy(scopeKind));

    function clearFilters() {
        activeDetectedClass.set(null);
        minSizeFilter.set(0);
        loadImagesForCurrentScope({ force: true })
            .catch(e => console.error('Failed to reload after clearing filters:', e));
    }

    function retryLoad() {
        loadImagesForCurrentScope({ force: true, invalidateCache: true })
            .catch(e => console.error('Retry library load failed:', e));
    }

    // First-run onboarding: the empty state IS the onboarding, so it gets a
    // working Import Folder action instead of only pointing at the sidebar.
    let importingFromEmptyState = $state(false);
    async function importFolderFromEmptyState() {
        if (importingFromEmptyState) return;
        const selected = await open({ directory: true, multiple: false });
        if (!selected) return;
        importingFromEmptyState = true;
        try {
            const result = await apiImportFolder(selected as string);
            const folderName = (selected as string).split('/').filter(Boolean).pop() ?? selected;
            let detail = `+${result.imported} imported, ${result.skipped} skipped`;
            if (result.errors.length > 0) detail += `, ${result.errors.length} errors`;
            showToast(`Imported "${folderName}"`, { detail, type: 'success', duration: 8000 });
            totalCount.set(await getImageCount());
            try {
                folders.set(await listFolders());
            } catch (_) {}
            await loadImagesForCurrentScope({ force: true, invalidateCache: true });
        } catch (e) {
            showToast('Import failed', { detail: String(e), type: 'error', duration: 10000 });
        } finally {
            importingFromEmptyState = false;
        }
    }

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
    onwheel={handleWheel}
    role="grid"
    aria-label={"Image grid, " + $images.length + " images"}
>
    {#if libraryViewState === 'error'}
        <div class="load-error" role="alert" data-testid="library-error-banner">
            <div class="error-icon">&#9888;</div>
            <div class="error-text">Library failed to load</div>
            <div class="error-detail">{$imageLoadState.error}</div>
            <button class="retry-btn" onclick={retryLoad}>Retry</button>
        </div>
    {:else if libraryViewState === 'loading'}
        <div class="empty" aria-live="polite">
            <div class="empty-text">Loading library&hellip;</div>
        </div>
    {:else if (libraryViewState === 'scope-empty' || libraryViewState === 'empty') && isClipboardMonitorEmpty}
        <div class="empty clipboard-empty">
            <img
                class="clipboard-empty-image"
                src={clipboardMonitorEmptySrc}
                alt=""
                aria-hidden="true"
            />
            <div class="empty-text">Clipboard monitor is waiting</div>
            <div class="empty-hint">Copied images will appear here as they arrive.</div>
            {#if $clipboardMonitorStatus?.collection_name}
                <div class="empty-hint">Saving into {$clipboardMonitorStatus.collection_name}</div>
            {/if}
        </div>
    {:else if libraryViewState === 'scope-empty'}
        <div class="empty" data-testid="scope-empty-state">
            <div class="empty-icon">&#9776;</div>
            <div class="empty-text">{scopeCopy.title}</div>
            {#if scopeCopy.clearFilters}
                <button class="empty-import-btn" onclick={clearFilters}>Clear Filters</button>
            {/if}
            <div class="empty-hint">{scopeCopy.hint}</div>
        </div>
    {:else if libraryViewState === 'empty'}
        <div class="empty">
            <div class="empty-icon">&#9776;</div>
            <div class="empty-text">Your library is empty</div>
            <button
                class="empty-import-btn"
                onclick={importFolderFromEmptyState}
                disabled={importingFromEmptyState}
            >
                {importingFromEmptyState ? 'Importing…' : '+ Import Folder'}
            </button>
            <div class="empty-hint">or drag &amp; drop a folder of images anywhere in this window</div>
            <div class="empty-hint">Agents can import for you too — connect via the Cull MCP server</div>
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
    .load-error {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100%;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
        text-align: center;
    }
    .error-icon {
        font-size: 48px;
        color: var(--red);
    }
    .error-text {
        font-size: 16px;
        color: var(--red);
    }
    .error-detail {
        max-width: 480px;
        font-size: 12px;
        color: var(--text-secondary);
        padding: var(--spacing);
        background: color-mix(in srgb, var(--red) 10%, transparent);
        border: 1px solid color-mix(in srgb, var(--red) 40%, transparent);
        border-radius: var(--radius);
        word-break: break-word;
    }
    .retry-btn {
        padding: var(--spacing) calc(var(--spacing) * 2);
        background: var(--surface);
        border: 1px solid var(--red);
        border-radius: var(--radius);
        color: var(--red);
        font-family: var(--font);
        font-size: 13px;
        cursor: pointer;
    }
    .retry-btn:hover {
        background: color-mix(in srgb, var(--red) 15%, var(--surface));
    }
    .empty-icon {
        font-size: 48px;
        color: var(--border);
    }
    .clipboard-empty {
        padding: calc(var(--spacing) * 3);
        text-align: center;
    }
    .clipboard-empty-image {
        width: min(54vw, 520px);
        max-height: 42vh;
        aspect-ratio: 1040 / 650;
        object-fit: contain;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        box-shadow: 0 0 0 1px color-mix(in srgb, var(--blue) 8%, transparent);
    }
    .empty-import-btn {
        padding: var(--spacing) calc(var(--spacing) * 2);
        background: color-mix(in srgb, var(--blue) 14%, transparent);
        border: 1px solid var(--blue);
        border-radius: var(--radius);
        color: var(--blue);
        font-family: var(--font);
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        transition: background 120ms, transform 80ms;
    }
    .empty-import-btn:hover:not(:disabled) {
        background: color-mix(in srgb, var(--blue) 22%, transparent);
    }
    .empty-import-btn:active:not(:disabled) {
        transform: translateY(1px);
    }
    .empty-import-btn:disabled {
        opacity: 0.5;
        cursor: default;
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
