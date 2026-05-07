<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { revealItemInDir } from '@tauri-apps/plugin-opener';
    import type { ImageWithFile } from '$lib/api';

    interface Props {
        item: ImageWithFile;
        size: number;
        focused: boolean;
        selected: boolean;
        onclick: () => void;
        ondblclick: () => void;
    }

    let { item, size, focused, selected, onclick, ondblclick }: Props = $props();

    let src = $derived(
        item.thumbnail_path
            ? convertFileSrc(item.thumbnail_path)
            : convertFileSrc(item.path)
    );

    let rating = $derived(item.selection?.star_rating ?? 0);
    let decision = $derived(item.selection?.decision ?? 'undecided');
    let filename = $derived(item.path.split('/').pop() ?? 'image');
    let imgError = $state(false);

    let borderClass = $derived(
        focused ? 'focused' : selected ? 'selected' : ''
    );

    let contextMenuVisible = $state(false);
    let contextMenuX = $state(0);
    let contextMenuY = $state(0);

    function handleImgError() {
        imgError = true;
    }

    function handleContextMenu(e: MouseEvent) {
        e.preventDefault();
        e.stopPropagation();
        contextMenuX = e.clientX;
        contextMenuY = e.clientY;
        contextMenuVisible = true;

        function closeMenu() {
            contextMenuVisible = false;
            window.removeEventListener('click', closeMenu);
            window.removeEventListener('contextmenu', closeMenu);
        }
        // Close on next click or right-click anywhere
        setTimeout(() => {
            window.addEventListener('click', closeMenu);
            window.addEventListener('contextmenu', closeMenu);
        });
    }

    function revealInFinder() {
        contextMenuVisible = false;
        revealItemInDir(item.path);
    }
</script>

<div
    class="thumb {borderClass}"
    style="width: {size}px; height: {size}px;"
    role="gridcell"
    tabindex="-1"
    aria-label={filename}
    aria-selected={selected}
    {onclick}
    {ondblclick}
    oncontextmenu={handleContextMenu}
    onkeydown={(e) => { if (e.key === 'Enter') onclick(); }}
>
    {#if imgError}
        <div class="fallback-text">{filename}</div>
    {:else}
        <img {src} alt={filename} loading="lazy" draggable="false" onerror={handleImgError} />
    {/if}

    {#if rating > 0}
        <div class="rating">
            {#each Array(rating) as _}
                <span class="star">&#9733;</span>
            {/each}
        </div>
    {/if}

    {#if decision === 'accept'}
        <div class="badge accept">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 12 10 16 18 8" />
            </svg>
        </div>
    {:else if decision === 'reject'}
        <div class="badge reject">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                <line x1="7" y1="7" x2="17" y2="17" />
                <line x1="17" y1="7" x2="7" y2="17" />
            </svg>
        </div>
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

<style>
    .thumb {
        position: relative;
        border: 2px solid transparent;
        border-radius: 0;
        overflow: hidden;
        cursor: pointer;
        background: var(--surface);
        transition: border-color 0.1s;
        flex-shrink: 0;
        display: flex;
        align-items: center;
        justify-content: center;
    }
    .thumb.focused {
        border-color: var(--blue);
    }
    .thumb.selected {
        border-color: var(--green);
    }
    .thumb.focused.selected {
        border-color: var(--blue);
        box-shadow: 0 0 0 1px var(--green);
    }
    img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        display: block;
    }
    .fallback-text {
        font-size: 10px;
        color: var(--text-secondary);
        text-align: center;
        word-break: break-all;
        padding: 4px;
        overflow: hidden;
    }
    .rating {
        position: absolute;
        bottom: 4px;
        left: 4px;
        display: flex;
        gap: 1px;
    }
    .star {
        color: var(--orange);
        font-size: 10px;
        text-shadow: 0 1px 2px rgba(0,0,0,0.8);
    }
    .badge {
        position: absolute;
        top: 6px;
        right: 6px;
        width: 22px;
        height: 22px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5), 0 0 0 2px rgba(0, 0, 0, 0.2);
        backdrop-filter: blur(4px);
    }
    .badge svg {
        width: 14px;
        height: 14px;
    }
    .badge.accept {
        background: var(--green);
        color: var(--bg);
    }
    .badge.reject {
        background: var(--red);
        color: var(--bg);
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
