<script lang="ts">
    import { images, selectedIds, focusedIndex, thumbnailSize, viewMode, gridGap } from '$lib/stores';
    import Thumbnail from './Thumbnail.svelte';

    let containerEl: HTMLDivElement | undefined = $state(undefined);
    let containerWidth = $state(800);
    let containerHeight = $state(600);
    let scrollTop = $state(0);

    let size = $state(160);
    thumbnailSize.subscribe(v => size = v);

    let gap = $state(4);
    gridGap.subscribe(v => gap = v);

    let cols = $derived(Math.max(1, Math.floor((containerWidth + gap) / (size + gap))));
    let cellSize = $derived(size + gap);
    let rows = $derived(Math.ceil($images.length / cols));
    let totalHeight = $derived(rows * cellSize);

    let firstVisibleRow = $derived(Math.floor(scrollTop / cellSize));
    let visibleRows = $derived(Math.ceil(containerHeight / cellSize) + 2);
    let lastVisibleRow = $derived(Math.min(firstVisibleRow + visibleRows, rows));

    interface VisibleItem {
        index: number;
        item: typeof $images[0];
        x: number;
        y: number;
    }

    let visibleItems = $derived.by(() => {
        const items: VisibleItem[] = [];
        const imgs = $images;
        for (let row = firstVisibleRow; row < lastVisibleRow; row++) {
            for (let col = 0; col < cols; col++) {
                const index = row * cols + col;
                if (index >= imgs.length) break;
                items.push({
                    index,
                    item: imgs[index],
                    x: col * cellSize,
                    y: row * cellSize,
                });
            }
        }
        return items;
    });

    function onScroll(e: Event) {
        scrollTop = (e.target as HTMLDivElement).scrollTop;
    }

    function handleClick(index: number) {
        focusedIndex.set(index);
    }

    function handleDblClick(index: number) {
        focusedIndex.set(index);
        viewMode.set('loupe');
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
                >
                    <Thumbnail
                        item={vi.item}
                        {size}
                        focused={$focusedIndex === vi.index}
                        selected={$selectedIds.has(vi.item.image.id)}
                        onclick={() => handleClick(vi.index)}
                        ondblclick={() => handleDblClick(vi.index)}
                    />
                </div>
            {/each}
        </div>
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
</style>
