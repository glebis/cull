<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import type { ImageWithFile } from '$lib/api';
    import { regenerateSingleThumbnail } from '$lib/api';
    import { recordImageLoadFailure } from '$lib/diagnostics';
    import ContextMenu from './ContextMenu.svelte';

    interface Props {
        item: ImageWithFile;
        size: number;
        focused: boolean;
        selected: boolean;
        onclick: (event: MouseEvent | KeyboardEvent) => void;
        ondblclick: () => void;
        loading?: 'lazy' | 'eager';
    }

    let { item, size, focused, selected, onclick, ondblclick, loading = 'lazy' }: Props = $props();

    let src = $derived(
        item.thumbnail_path
            ? convertFileSrc(item.thumbnail_path)
            : convertFileSrc(item.path)
    );

    let rating = $derived(item.selection?.star_rating ?? 0);
    let decision = $derived(item.selection?.decision ?? 'undecided');
    let filename = $derived(item.path.split('/').pop() ?? 'image');

    const SOURCE_LABELS: Record<string, string> = {
        gpt_image_2: 'GPT',
        dalle_3: 'DALL-E',
        dalle: 'DALL-E',
        openai: 'OpenAI',
        stable_diffusion: 'SD',
        comfyui: 'ComfyUI',
        midjourney: 'MJ',
        nanobanana: 'NB',
    };
    let sourceTag = $derived(item.source_label ? SOURCE_LABELS[item.source_label] ?? item.source_label : null);
    let imgError = $state(false);
    let regenerating = $state(false);

    let borderClass = $derived(
        focused ? 'focused' : selected ? 'selected' : ''
    );

    let ctxMenu = $state({ visible: false, x: 0, y: 0 });

    async function handleImgError() {
        if (regenerating) return;
        recordImageLoadFailure({
            view: 'thumbnail',
            image: item,
            assetKind: 'thumbnail',
            errorKind: 'img_onerror',
            fallbackUsed: false,
            fallbackSucceeded: null,
            phase: 'thumbnail',
        });
        regenerating = true;
        try {
            const newPath = await regenerateSingleThumbnail(item.image.id);
            item.thumbnail_path = newPath;
        } catch {
            recordImageLoadFailure({
                view: 'thumbnail',
                image: item,
                assetKind: 'thumbnail',
                errorKind: 'thumbnail_regeneration_failed',
                fallbackUsed: false,
                fallbackSucceeded: false,
                phase: 'regenerate',
            });
            imgError = true;
        } finally {
            regenerating = false;
        }
    }

    function handleContextMenu(e: MouseEvent) {
        e.preventDefault();
        e.stopPropagation();
        ctxMenu = { visible: true, x: e.clientX, y: e.clientY };
    }
</script>

<div
    class="thumb {borderClass}"
    style="width: {size}px; height: {size}px;"
    role="gridcell"
    tabindex={focused ? 0 : -1}
    aria-label={filename}
    aria-selected={selected}
    {onclick}
    {ondblclick}
    oncontextmenu={handleContextMenu}
    onkeydown={(e) => { if (e.key === 'Enter') onclick(e); }}
>
    {#if imgError}
        <div class="fallback-text">{filename}</div>
    {:else if regenerating}
        <div class="regenerating"></div>
    {:else}
        <img {src} alt={filename} {loading} decoding="async" draggable="false" onerror={handleImgError} />
    {/if}

    {#if item.missing_at}
        <div class="missing-overlay">
            <span class="missing-badge">Missing</span>
        </div>
    {/if}

    {#if rating > 0}
        <div class="rating">
            {#each Array(rating) as _}
                <span class="star">&#9733;</span>
            {/each}
        </div>
    {/if}

    {#if sourceTag}
        <div class="source-tag">{sourceTag}</div>
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

{#if ctxMenu.visible}
    <ContextMenu
        image={item}
        x={ctxMenu.x}
        y={ctxMenu.y}
        onclose={() => ctxMenu.visible = false}
    />
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
    .regenerating {
        width: 24px;
        height: 24px;
        border: 2px solid var(--border);
        border-top-color: var(--blue);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
    }
    @keyframes spin {
        to { transform: rotate(360deg); }
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
    .source-tag {
        position: absolute;
        top: 4px;
        left: 4px;
        font-size: 9px;
        font-weight: 600;
        letter-spacing: 0.03em;
        padding: 1px 5px;
        border-radius: 3px;
        background: rgba(0, 0, 0, 0.65);
        color: var(--purple, #bb9af7);
        backdrop-filter: blur(4px);
        line-height: 1.4;
    }
    .badge {
        position: absolute;
        top: 0;
        right: 0;
        width: 24px;
        height: 22px;
        border: 1px solid var(--border);
        border-top: 0;
        border-right: 0;
        border-radius: 0 0 0 var(--radius);
        background: var(--surface);
        color: var(--text-secondary);
        display: flex;
        align-items: center;
        justify-content: center;
        box-shadow: 0 0 0 1px var(--bg);
        opacity: 0.96;
        pointer-events: none;
    }
    .badge svg {
        width: 12px;
        height: 12px;
        stroke-width: 2.4;
    }
    .badge.accept {
        border-left-color: var(--green);
        border-bottom-color: var(--green);
        color: var(--green);
    }
    .badge.reject {
        border-left-color: var(--red);
        border-bottom-color: var(--red);
        color: var(--red);
    }
    .missing-overlay {
        position: absolute;
        inset: 0;
        background: rgba(0, 0, 0, 0.5);
        display: flex;
        align-items: center;
        justify-content: center;
        pointer-events: none;
    }
    .missing-badge {
        font-size: 9px;
        font-weight: 600;
        color: #f87171;
        background: rgba(127, 29, 29, 0.6);
        padding: 1px 6px;
        border-radius: 3px;
    }
</style>
