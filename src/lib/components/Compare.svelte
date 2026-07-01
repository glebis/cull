<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { images, selectedIds, focusedIndex, compareActiveSide, compareImageOnly, zenMode } from '$lib/stores';
    import type { ImageWithFile } from '$lib/api';
    import { nextCompareFocusedIndex } from '$lib/compare-gestures';
    import { classifySwipe, wheelGestureIntent } from '$lib/gesture-interactions';
    import { computeCompareSwap } from '$lib/compare-utils';
    import { safeAssetPreviewPath } from '$lib/view-utils';
    import ContextMenu from './ContextMenu.svelte';

    // Resolve the two images to compare
    let pair = $derived.by(() => {
        const imgs = $images;
        const sel = $selectedIds;
        const idx = $focusedIndex;

        if (sel.size >= 2) {
            const selArr = Array.from(sel);
            const a = imgs.find(i => i.image.id === selArr[0]);
            const b = imgs.find(i => i.image.id === selArr[1]);
            if (a && b) return [a, b] as const;
        }

        // Fallback: focused + next
        const a = imgs[idx];
        const b = imgs[idx + 1];
        if (a && b) return [a, b] as const;
        if (a) return [a, null] as const;
        return [null, null] as const;
    });

    let leftImage = $derived(pair[0]);
    let rightImage = $derived(pair[1]);

    let leftPreviewPath = $derived(leftImage ? safeAssetPreviewPath(leftImage) : null);
    let rightPreviewPath = $derived(rightImage ? safeAssetPreviewPath(rightImage) : null);
    let leftSrc = $derived(leftPreviewPath ? convertFileSrc(leftPreviewPath) : '');
    let rightSrc = $derived(rightPreviewPath ? convertFileSrc(rightPreviewPath) : '');

    let leftName = $derived(leftImage?.path.split('/').pop() ?? '');
    let rightName = $derived(rightImage?.path.split('/').pop() ?? '');
    let imageOnly = $derived($zenMode && $compareImageOnly);
    let focusedImageId = $derived($images[$focusedIndex]?.image.id ?? null);
    let compareEl: HTMLDivElement | undefined = $state();
    let wheelSwipeX = 0;
    let wheelSwipeY = 0;
    let wheelSwipeResetTimer: ReturnType<typeof setTimeout> | null = null;
    let lastWheelNavigationAt = 0;

    function ratingStars(img: ImageWithFile | null): number {
        return img?.selection?.star_rating ?? 0;
    }

    function decisionLabel(img: ImageWithFile | null): string {
        return img?.selection?.decision ?? 'undecided';
    }

    function comparePanelLabel(side: 'Left' | 'Right', img: ImageWithFile | null): string {
        const name = img?.path.split('/').pop() ?? 'empty slot';
        return `${side} compare panel: ${name}`;
    }

    function handlePanelKeydown(e: KeyboardEvent, side: 0 | 1) {
        if (e.key !== 'Enter' && e.key !== ' ') return;
        e.preventDefault();
        compareActiveSide.set(side);
    }

    let ctxMenu = $state<{ visible: boolean; x: number; y: number; image: ImageWithFile | null }>({ visible: false, x: 0, y: 0, image: null });

    function handleContextMenu(e: MouseEvent, img: ImageWithFile | null) {
        if (!img) return;
        e.preventDefault();
        ctxMenu = { visible: true, x: e.clientX, y: e.clientY, image: img };
    }

    function handleWheel(e: WheelEvent) {
        const rect = compareEl?.getBoundingClientRect();
        if (!rect) return;
        const intent = wheelGestureIntent({
            surface: 'compare',
            deltaX: e.deltaX,
            deltaY: e.deltaY,
            deltaMode: e.deltaMode,
            clientX: e.clientX,
            clientY: e.clientY,
            ctrlKey: e.ctrlKey,
            metaKey: e.metaKey,
            altKey: e.altKey,
            shiftKey: e.shiftKey,
            viewportHeight: rect.height,
            target: e.target,
        });
        if (!intent || intent.type !== 'pan') return;

        wheelSwipeX += intent.deltaX;
        wheelSwipeY += intent.deltaY;
        scheduleWheelSwipeReset();
        const direction = classifySwipe({ deltaX: wheelSwipeX, deltaY: wheelSwipeY });
        const now = Date.now();
        if (!direction || now - lastWheelNavigationAt < 250) return;

        e.preventDefault();
        lastWheelNavigationAt = now;
        resetWheelSwipe();
        applyCompareSwipe(direction);
    }

    function applyCompareSwipe(direction: 'previous' | 'next') {
        const imgs = $images;
        if (imgs.length === 0) return;

        if ($selectedIds.size >= 2) {
            const result = computeCompareSwap(
                imgs.map(item => item.image.id),
                $selectedIds,
                $focusedIndex,
                $compareActiveSide,
                direction === 'next' ? 1 : -1,
            );
            if (result) selectedIds.set(result.newSelectedIds);
            return;
        }

        selectedIds.set(new Set());
        compareActiveSide.set(0);
        focusedIndex.set(nextCompareFocusedIndex($focusedIndex, imgs.length, direction));
    }

    function scheduleWheelSwipeReset() {
        if (wheelSwipeResetTimer) clearTimeout(wheelSwipeResetTimer);
        wheelSwipeResetTimer = setTimeout(resetWheelSwipe, 180);
    }

    function resetWheelSwipe() {
        wheelSwipeX = 0;
        wheelSwipeY = 0;
        if (wheelSwipeResetTimer) {
            clearTimeout(wheelSwipeResetTimer);
            wheelSwipeResetTimer = null;
        }
    }
</script>

<div class="compare-container" class:images-only={imageOnly} bind:this={compareEl} onwheel={handleWheel}>
    <div
        class="panel"
        class:active={$compareActiveSide === 0}
        onclick={() => compareActiveSide.set(0)}
        oncontextmenu={(e) => handleContextMenu(e, leftImage)}
        role="button"
        tabindex="0"
        aria-label={comparePanelLabel('Left', leftImage)}
        aria-pressed={$compareActiveSide === 0}
        onkeydown={(e) => handlePanelKeydown(e, 0)}
        data-agent-image-id={leftImage?.image.id ?? ''}
        data-agent-filename={leftName}
        data-agent-path={leftImage?.path ?? ''}
        data-agent-thumbnail-path={leftImage?.thumbnail_path ?? ''}
        data-agent-ai-prompt={leftImage?.image.ai_prompt ?? ''}
        data-agent-rating={leftImage?.selection?.star_rating ?? ''}
        data-agent-decision={leftImage?.selection?.decision ?? 'undecided'}
        data-agent-selected={leftImage ? $selectedIds.has(leftImage.image.id) : false}
        data-agent-focused={leftImage?.image.id === focusedImageId}
        data-agent-view-role="compare-left"
    >
        {#if leftImage}
            {#if !imageOnly}
                <div class="label">{leftName}</div>
            {/if}
            <div class="img-wrap">
                {#if leftSrc}
                    <img src={leftSrc} alt={leftName} draggable="false" />
                {:else}
                    <div class="preview-unavailable">Preview unavailable</div>
                {/if}
            </div>
            {#if !imageOnly}
                <div class="meta">
                    {#if ratingStars(leftImage) > 0}
                        <span class="rating">
                            {#each Array(ratingStars(leftImage)) as _}
                                <span class="star">&#9733;</span>
                            {/each}
                        </span>
                    {/if}
                    <span class="decision" class:accept={decisionLabel(leftImage) === 'accept'} class:reject={decisionLabel(leftImage) === 'reject'}>
                        {decisionLabel(leftImage)}
                    </span>
                </div>
            {/if}
        {:else}
            {#if !imageOnly}
                <div class="empty-panel">No image</div>
            {/if}
        {/if}
    </div>

    <div class="divider"></div>

    <div
        class="panel"
        class:active={$compareActiveSide === 1}
        onclick={() => compareActiveSide.set(1)}
        oncontextmenu={(e) => handleContextMenu(e, rightImage)}
        role="button"
        tabindex="0"
        aria-label={comparePanelLabel('Right', rightImage)}
        aria-pressed={$compareActiveSide === 1}
        onkeydown={(e) => handlePanelKeydown(e, 1)}
        data-agent-image-id={rightImage?.image.id ?? ''}
        data-agent-filename={rightName}
        data-agent-path={rightImage?.path ?? ''}
        data-agent-thumbnail-path={rightImage?.thumbnail_path ?? ''}
        data-agent-ai-prompt={rightImage?.image.ai_prompt ?? ''}
        data-agent-rating={rightImage?.selection?.star_rating ?? ''}
        data-agent-decision={rightImage?.selection?.decision ?? 'undecided'}
        data-agent-selected={rightImage ? $selectedIds.has(rightImage.image.id) : false}
        data-agent-focused={rightImage?.image.id === focusedImageId}
        data-agent-view-role="compare-right"
    >
        {#if rightImage}
            {#if !imageOnly}
                <div class="label">{rightName}</div>
            {/if}
            <div class="img-wrap">
                {#if rightSrc}
                    <img src={rightSrc} alt={rightName} draggable="false" />
                {:else}
                    <div class="preview-unavailable">Preview unavailable</div>
                {/if}
            </div>
            {#if !imageOnly}
                <div class="meta">
                    {#if ratingStars(rightImage) > 0}
                        <span class="rating">
                            {#each Array(ratingStars(rightImage)) as _}
                                <span class="star">&#9733;</span>
                            {/each}
                        </span>
                    {/if}
                    <span class="decision" class:accept={decisionLabel(rightImage) === 'accept'} class:reject={decisionLabel(rightImage) === 'reject'}>
                        {decisionLabel(rightImage)}
                    </span>
                </div>
            {/if}
        {:else}
            {#if !imageOnly}
                <div class="empty-panel">No image</div>
            {/if}
        {/if}
    </div>

    {#if ctxMenu.visible && ctxMenu.image}
        <ContextMenu
            image={ctxMenu.image}
            x={ctxMenu.x}
            y={ctxMenu.y}
            onclose={() => ctxMenu.visible = false}
        />
    {/if}
</div>

<style>
    .compare-container {
        grid-area: main;
        display: grid;
        grid-template-columns: minmax(0, 1fr) 1px minmax(0, 1fr);
        background: var(--bg);
        overflow: hidden;
    }
    .panel {
        display: flex;
        flex-direction: column;
        align-items: center;
        min-width: 0;
        min-height: 0;
        padding: 8px;
        border: 2px solid transparent;
        box-sizing: border-box;
        transition: border-color 0.15s;
        overflow: hidden;
        cursor: pointer;
    }
    .panel.active {
        border-color: var(--blue);
    }
    .panel:focus-visible {
        outline: 2px solid var(--blue);
        outline-offset: -4px;
    }
    .compare-container.images-only {
        grid-template-columns: minmax(0, 1fr) 0 minmax(0, 1fr);
    }
    .compare-container.images-only .panel {
        padding: 0;
        border-width: 0;
    }
    .compare-container.images-only .panel.active {
        border-color: transparent;
    }
    .divider {
        width: 1px;
        background: var(--border);
    }
    .compare-container.images-only .divider {
        width: 0;
        background: transparent;
    }
    .label {
        font-size: 11px;
        color: var(--text-secondary);
        margin-bottom: 4px;
        max-width: 100%;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .img-wrap {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        min-width: 0;
        min-height: 0;
        overflow: hidden;
        width: 100%;
    }
    .img-wrap img {
        width: 100%;
        height: 100%;
        object-fit: contain;
        display: block;
    }
    .preview-unavailable {
        color: var(--text-secondary);
        font-size: 12px;
        text-align: center;
    }
    .meta {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-top: 4px;
        font-size: 11px;
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
        color: var(--text-secondary);
        text-transform: uppercase;
        font-size: 10px;
    }
    .decision.accept {
        color: var(--green);
    }
    .decision.reject {
        color: var(--red);
    }
    .empty-panel {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        color: var(--text-secondary);
        font-size: 12px;
    }
</style>
