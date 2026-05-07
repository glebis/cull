<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { revealItemInDir } from '@tauri-apps/plugin-opener';
    import { images, selectedIds, focusedIndex, statusHint, compareActiveSide } from '$lib/stores';
    import type { ImageWithFile } from '$lib/api';

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

    let leftSrc = $derived(leftImage ? convertFileSrc(leftImage.path) : '');
    let rightSrc = $derived(rightImage ? convertFileSrc(rightImage.path) : '');

    let leftName = $derived(leftImage?.path.split('/').pop() ?? '');
    let rightName = $derived(rightImage?.path.split('/').pop() ?? '');

    $effect(() => {
        statusHint.set(`${leftName} vs ${rightName}`);
        return () => statusHint.set(null);
    });

    function ratingStars(img: ImageWithFile | null): number {
        return img?.selection?.star_rating ?? 0;
    }

    function decisionLabel(img: ImageWithFile | null): string {
        return img?.selection?.decision ?? 'undecided';
    }

    let contextMenuVisible = $state(false);
    let contextMenuX = $state(0);
    let contextMenuY = $state(0);
    let contextMenuPath = $state('');

    function handleContextMenu(e: MouseEvent, img: ImageWithFile | null) {
        if (!img) return;
        e.preventDefault();
        contextMenuX = e.clientX;
        contextMenuY = e.clientY;
        contextMenuPath = img.path;
        contextMenuVisible = true;

        function closeMenu() {
            contextMenuVisible = false;
            window.removeEventListener('click', closeMenu);
            window.removeEventListener('contextmenu', closeMenu);
        }
        setTimeout(() => {
            window.addEventListener('click', closeMenu);
            window.addEventListener('contextmenu', closeMenu);
        });
    }

    function revealInFinder() {
        contextMenuVisible = false;
        if (contextMenuPath) revealItemInDir(contextMenuPath);
    }
</script>

<div class="compare-container">
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
            <div class="label">{leftName}</div>
            <div class="img-wrap">
                <img src={leftSrc} alt={leftName} draggable="false" />
            </div>
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
        {:else}
            <div class="empty-panel">No image</div>
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
            <div class="label">{rightName}</div>
            <div class="img-wrap">
                <img src={rightSrc} alt={rightName} draggable="false" />
            </div>
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
        {:else}
            <div class="empty-panel">No image</div>
        {/if}
    </div>

    {#if contextMenuVisible}
        <div
            class="context-menu"
            style="left: {contextMenuX}px; top: {contextMenuY}px;"
            role="menu"
        >
            <button class="context-menu-item" onclick={revealInFinder} role="menuitem">
                Reveal in Finder
            </button>
        </div>
    {/if}
</div>

<style>
    .compare-container {
        grid-area: main;
        display: flex;
        background: var(--bg);
        overflow: hidden;
    }
    .panel {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        padding: 8px;
        border: 2px solid transparent;
        transition: border-color 0.15s;
        overflow: hidden;
        cursor: pointer;
    }
    .panel.active {
        border-color: var(--blue);
    }
    .divider {
        width: 1px;
        background: var(--border);
        flex-shrink: 0;
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
        overflow: hidden;
        width: 100%;
    }
    .img-wrap img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
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
    .context-menu {
        position: fixed;
        z-index: 9999;
        background: var(--surface, #2a2a2e);
        border: 1px solid var(--border, #444);
        border-radius: 4px;
        padding: 4px 0;
        min-width: 160px;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    }
    .context-menu-item {
        display: block;
        width: 100%;
        padding: 6px 12px;
        background: none;
        border: none;
        color: var(--text, #eee);
        font-size: 12px;
        text-align: left;
        cursor: pointer;
    }
    .context-menu-item:hover {
        background: var(--blue, #3b82f6);
        color: #fff;
    }
</style>
