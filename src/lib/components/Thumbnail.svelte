<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
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

    function handleImgError() {
        imgError = true;
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
        <div class="badge accept">&#10003;</div>
    {:else if decision === 'reject'}
        <div class="badge reject">&#10007;</div>
    {/if}
</div>

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
        top: 4px;
        right: 4px;
        width: 18px;
        height: 18px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 11px;
        font-weight: 700;
    }
    .badge.accept {
        background: var(--green);
        color: var(--bg);
    }
    .badge.reject {
        background: var(--red);
        color: var(--bg);
    }
</style>
