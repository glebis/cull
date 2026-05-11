<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { listen } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { getImagesByIds, type ImageWithFile } from '$lib/api';

    interface Props {
        oncompare: (imageIds: string[]) => void;
        onselect: (imageId: string) => void;
    }

    let { oncompare, onselect }: Props = $props();

    let images = $state<ImageWithFile[]>([]);
    let visible = $state(false);
    let jobId = $state<string | null>(null);
    let unlistener: (() => void) | null = null;

    onMount(async () => {
        try {
            unlistener = await listen<any>('generation-complete', async (e) => {
                const ids: string[] = e.payload.image_ids ?? [];
                jobId = e.payload.job_id ?? null;
                if (ids.length > 0) {
                    images = await getImagesByIds(ids);
                    visible = true;
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
        images = [];
    }

    function openCompare() {
        oncompare(images.map(i => i.image.id));
        dismiss();
    }

    function thumbnailUrl(img: ImageWithFile): string {
        return convertFileSrc(img.thumbnail_path ?? img.path);
    }
</script>

{#if visible && images.length > 0}
    <div class="results-strip">
        <div class="strip-header">
            <span class="strip-title">Generated {images.length} image{images.length > 1 ? 's' : ''}</span>
            <div class="strip-actions">
                {#if images.length > 1}
                    <button class="strip-btn" onclick={openCompare}>Compare</button>
                {/if}
                <button class="strip-btn dismiss" onclick={dismiss}>&times;</button>
            </div>
        </div>
        <div class="strip-images">
            {#each images as img}
                <button class="strip-thumb" onclick={() => onselect(img.image.id)}>
                    <img src={thumbnailUrl(img)} alt="" />
                </button>
            {/each}
        </div>
    </div>
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
    .strip-thumb img {
        width: 100%;
        height: 100%;
        object-fit: contain;
    }
</style>
