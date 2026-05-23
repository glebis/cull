<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { listen } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { getImagesByIds, type ImageWithFile } from '$lib/api';
    import { focusedImage, openImageInLoupe } from '$lib/stores';
    import ContextMenu from './ContextMenu.svelte';

    let resultImages = $state<ImageWithFile[]>([]);
    let visible = $state(false);
    let jobId = $state<string | null>(null);
    let unlistener: (() => void) | null = null;
    let ctxMenu = $state<{ visible: boolean; x: number; y: number; image: ImageWithFile | null }>({
        visible: false,
        x: 0,
        y: 0,
        image: null,
    });

    onMount(async () => {
        try {
            unlistener = await listen<any>('generation-complete', async (e) => {
                const ids: string[] = e.payload.image_ids ?? [];
                jobId = e.payload.job_id ?? null;
                if (ids.length > 0) {
                    resultImages = await getImagesByIds(ids);
                    if (resultImages.length > 0) {
                        visible = true;
                        openImageInLoupe(resultImages[0]);
                    }
                }
            });
        } catch {
            // Not in Tauri
        }
    });

    onDestroy(() => {
        unlistener?.();
    });

    function dismiss() {
        visible = false;
        resultImages = [];
        ctxMenu = { visible: false, x: 0, y: 0, image: null };
    }

    function thumbnailUrl(img: ImageWithFile): string {
        return convertFileSrc(img.thumbnail_path ?? img.path);
    }

    function openResult(img: ImageWithFile) {
        openImageInLoupe(img);
    }

    function handleContextMenu(e: MouseEvent, img: ImageWithFile) {
        e.preventDefault();
        e.stopPropagation();
        openImageInLoupe(img);
        ctxMenu = { visible: true, x: e.clientX, y: e.clientY, image: img };
    }
</script>

{#if visible && resultImages.length > 0}
    <div class="results-strip">
        <div class="strip-header">
            <span class="strip-title">Generated {resultImages.length} image{resultImages.length > 1 ? 's' : ''}</span>
            <div class="strip-actions">
                <button class="strip-btn dismiss" onclick={dismiss}>&times;</button>
            </div>
        </div>
        <div class="strip-images">
            {#each resultImages as img}
                <button
                    class="strip-thumb"
                    class:active={$focusedImage?.image.id === img.image.id}
                    onclick={() => openResult(img)}
                    oncontextmenu={(e) => handleContextMenu(e, img)}
                    title={img.path.split('/').pop() ?? 'Generated image'}
                >
                    <img src={thumbnailUrl(img)} alt="" />
                </button>
            {/each}
        </div>
    </div>

    {#if ctxMenu.visible && ctxMenu.image}
        <ContextMenu
            image={ctxMenu.image}
            x={ctxMenu.x}
            y={ctxMenu.y}
            onclose={() => ctxMenu.visible = false}
        />
    {/if}
{/if}

<style>
    .results-strip {
        position: fixed;
        bottom: 48px;
        left: 50%;
        transform: translateX(-50%);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        padding: var(--spacing);
        z-index: 900;
        min-width: 200px;
        max-width: 90vw;
        box-shadow: 0 0 0 1px var(--bg);
    }
    .strip-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: var(--spacing);
    }
    .strip-title {
        font-size: 11px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 0.04em;
    }
    .strip-actions {
        display: flex;
        gap: 4px;
    }
    .strip-btn {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--blue);
        font-size: 11px;
        font-family: var(--font);
        padding: 2px 8px;
        cursor: pointer;
    }
    .strip-btn.dismiss {
        color: var(--text-secondary);
        border: none;
        background: none;
        font-size: 14px;
    }
    .strip-images {
        display: flex;
        gap: var(--spacing);
        overflow-x: auto;
    }
    .strip-thumb {
        flex-shrink: 0;
        width: 80px;
        height: 80px;
        border-radius: var(--radius);
        overflow: hidden;
        border: 1px solid var(--border);
        padding: 0;
        background: var(--bg);
        cursor: pointer;
    }
    .strip-thumb:hover {
        border-color: var(--blue);
    }
    .strip-thumb.active {
        border-color: var(--blue);
        box-shadow: 0 0 0 1px var(--blue);
    }
    .strip-thumb:focus-visible {
        outline: 1px solid var(--blue);
        outline-offset: 2px;
    }
    .strip-thumb img {
        width: 100%;
        height: 100%;
        object-fit: contain;
    }
</style>
