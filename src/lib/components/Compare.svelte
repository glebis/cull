<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { images, selectedIds, focusedIndex, compareActiveSide, compareImageOnly, zenMode } from '$lib/stores';
    import type { ImageWithFile } from '$lib/api';
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

    function ratingStars(img: ImageWithFile | null): number {
        return img?.selection?.star_rating ?? 0;
    }

    function decisionLabel(img: ImageWithFile | null): string {
        return img?.selection?.decision ?? 'undecided';
    }

    let ctxMenu = $state<{ visible: boolean; x: number; y: number; image: ImageWithFile | null }>({ visible: false, x: 0, y: 0, image: null });

    function handleContextMenu(e: MouseEvent, img: ImageWithFile | null) {
        if (!img) return;
        e.preventDefault();
        ctxMenu = { visible: true, x: e.clientX, y: e.clientY, image: img };
    }
</script>

<div class="compare-container" class:images-only={imageOnly}>
    <div
        class="panel"
        class:active={$compareActiveSide === 0}
        onclick={() => compareActiveSide.set(0)}
        oncontextmenu={(e) => handleContextMenu(e, leftImage)}
        role="button"
        tabindex="0"
        onkeydown={() => {}}
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
        onkeydown={() => {}}
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
