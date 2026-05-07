<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { images, focusedIndex, statusHint, loupeScale, loupePanX, loupePanY } from '$lib/stores';

    let dragging = $state(false);
    let dragStartX = $state(0);
    let dragStartY = $state(0);
    let panStartX = $state(0);
    let panStartY = $state(0);

    let image = $derived($images[$focusedIndex] ?? null);
    let src = $derived(image ? convertFileSrc(image.path) : '');
    let filename = $derived(image?.path.split('/').pop() ?? '');
    let dimensions = $derived(image ? `${image.image.width}x${image.image.height}` : '');
    let format = $derived(image?.image.format ?? '');
    let rating = $derived(image?.selection?.star_rating ?? 0);
    let decision = $derived(image?.selection?.decision ?? 'undecided');

    $effect(() => {
        const info = `${filename} | ${dimensions} | ${format}`;
        statusHint.set(info);
        return () => statusHint.set(null);
    });

    // Reset zoom/pan when image changes
    let prevImageId = $state('');
    $effect(() => {
        const id = image?.image.id ?? '';
        if (id !== prevImageId) {
            prevImageId = id;
            loupeScale.set(1);
            loupePanX.set(0);
            loupePanY.set(0);
        }
    });

    function handleWheel(e: WheelEvent) {
        e.preventDefault();
        const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
        loupeScale.update(s => {
            const next = Math.max(0.1, Math.min(20, s * factor));
            if (next <= 1) {
                loupePanX.set(0);
                loupePanY.set(0);
            }
            return next;
        });
    }

    function handleMouseDown(e: MouseEvent) {
        if ($loupeScale <= 1) return;
        dragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        panStartX = $loupePanX;
        panStartY = $loupePanY;
    }

    function handleMouseMove(e: MouseEvent) {
        if (!dragging) return;
        loupePanX.set(panStartX + (e.clientX - dragStartX));
        loupePanY.set(panStartY + (e.clientY - dragStartY));
    }

    function handleMouseUp() {
        dragging = false;
    }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
    class="loupe-container"
    onwheel={handleWheel}
    onmousedown={handleMouseDown}
    onmousemove={handleMouseMove}
    onmouseup={handleMouseUp}
    onmouseleave={handleMouseUp}
    class:dragging
    class:zoomed={$loupeScale > 1}
>
    {#if image}
        <img
            {src}
            alt={filename}
            draggable="false"
            style="transform: scale({$loupeScale}) translate({$loupePanX / $loupeScale}px, {$loupePanY / $loupeScale}px);"
        />
    {:else}
        <div class="empty">No image selected</div>
    {/if}

    <div class="overlay-bar">
        <span class="filename">{filename}</span>
        <span class="sep">|</span>
        <span class="dim">{dimensions}</span>
        <span class="sep">|</span>
        <span class="fmt">{format}</span>
        {#if rating > 0}
            <span class="sep">|</span>
            <span class="rating">
                {#each Array(rating) as _}
                    <span class="star">&#9733;</span>
                {/each}
            </span>
        {/if}
        {#if decision !== 'undecided'}
            <span class="sep">|</span>
            <span class="decision" class:accept={decision === 'accept'} class:reject={decision === 'reject'}>
                {decision}
            </span>
        {/if}
        {#if $loupeScale !== 1}
            <span class="sep">|</span>
            <span class="zoom">{Math.round($loupeScale * 100)}%</span>
        {/if}
    </div>
</div>

<style>
    .loupe-container {
        grid-area: main;
        display: flex;
        align-items: center;
        justify-content: center;
        background: var(--bg);
        overflow: hidden;
        position: relative;
        cursor: default;
    }
    .loupe-container.zoomed {
        cursor: grab;
    }
    .loupe-container.dragging {
        cursor: grabbing;
    }
    img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        transform-origin: center center;
        user-select: none;
        -webkit-user-drag: none;
    }
    .empty {
        color: var(--text-secondary);
        font-size: 14px;
    }
    .overlay-bar {
        position: absolute;
        bottom: 0;
        left: 0;
        right: 0;
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 6px 12px;
        background: rgba(8, 8, 12, 0.85);
        font-size: 11px;
    }
    .filename {
        color: var(--text);
    }
    .sep {
        color: var(--border);
    }
    .dim, .fmt {
        color: var(--text-secondary);
    }
    .rating {
        display: flex;
        gap: 1px;
    }
    .star {
        color: var(--orange);
        font-size: 12px;
    }
    .decision {
        text-transform: uppercase;
        font-size: 10px;
        color: var(--text-secondary);
    }
    .decision.accept {
        color: var(--green);
    }
    .decision.reject {
        color: var(--red);
    }
    .zoom {
        color: var(--blue);
    }
</style>
