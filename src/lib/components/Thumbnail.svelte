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

    let borderClass = $derived(
        focused ? 'focused' : selected ? 'selected' : ''
    );
</script>

<div
    class="thumb {borderClass}"
    style="width: {size}px; height: {size}px;"
    role="button"
    tabindex="-1"
    {onclick}
    {ondblclick}
    onkeydown={(e) => { if (e.key === 'Enter') onclick(); }}
>
    <img {src} alt="" loading="lazy" draggable="false" />

    {#if rating > 0}
        <div class="rating">
            {#each Array(rating) as _}
                <span class="star">&#9733;</span>
            {/each}
        </div>
    {/if}

    {#if decision === 'accepted'}
        <div class="badge accept">&#10003;</div>
    {:else if decision === 'rejected'}
        <div class="badge reject">&#10007;</div>
    {/if}
</div>

<style>
    .thumb {
        position: relative;
        border: 2px solid transparent;
        border-radius: var(--radius);
        overflow: hidden;
        cursor: pointer;
        background: var(--surface);
        transition: border-color 0.1s;
        flex-shrink: 0;
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
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
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
