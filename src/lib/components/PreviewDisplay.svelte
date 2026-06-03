<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { onMount } from 'svelte';
    import { getImagesByIds, getPreviewState, isRawFormat, type ImageWithFile, type PreviewState } from '$lib/api';
    import { previewDisplayImageSourcePath } from '$lib/preview-display';

    type DisplayLoadState = 'loading' | 'empty' | 'ready' | 'missing' | 'error' | 'blanked';

    let previewState = $state<PreviewState | null>(null);
    let image = $state<ImageWithFile | null>(null);
    let loadState = $state<DisplayLoadState>('loading');
    let sourceLoadFailed = $state(false);
    let requestSeq = 0;

    let imageSrc = $derived(image ? convertFileSrc(previewDisplayImageSourcePath(image, sourceLoadFailed)) : '');
    let filename = $derived(image?.path.split('/').pop() ?? '');
    let rating = $derived(image?.selection?.star_rating ?? 0);
    let decision = $derived(image?.selection?.decision ?? 'undecided');
    let dimensions = $derived(image ? `${image.image.width}x${image.image.height}` : '');

    async function applyPreviewState(next: PreviewState) {
        previewState = next;
        sourceLoadFailed = false;

        if (next.blanked) {
            requestSeq++;
            image = null;
            loadState = 'blanked';
            return;
        }

        if (!next.image_id) {
            requestSeq++;
            image = null;
            loadState = 'empty';
            return;
        }

        const seq = ++requestSeq;
        loadState = 'loading';

        try {
            const records = await getImagesByIds([next.image_id]);
            if (seq !== requestSeq) return;
            image = records[0] ?? null;
            loadState = image ? 'ready' : 'missing';
        } catch (e) {
            if (seq !== requestSeq) return;
            console.error('Failed to load Preview Display image:', e);
            image = null;
            loadState = 'error';
        }
    }

    function handleImageError() {
        if (!image) return;
        const canFallback = !sourceLoadFailed && !isRawFormat(image.image.format) && !!image.thumbnail_path;
        if (canFallback) {
            sourceLoadFailed = true;
            return;
        }
        loadState = 'error';
    }

    onMount(() => {
        getPreviewState()
            .then(applyPreviewState)
            .catch((e) => {
                console.error('Failed to load Preview Display state:', e);
                loadState = 'error';
            });

        const stateUnlisten = listen<PreviewState>('preview:state-changed', (event) => {
            applyPreviewState(event.payload).catch((e) => {
                console.error('Failed to apply Preview Display state:', e);
                loadState = 'error';
            });
        });

        return () => {
            stateUnlisten.then((fn) => fn());
        };
    });
</script>

<div class="preview-display" data-state={loadState}>
    {#if loadState === 'ready' && image}
        <img
            class="preview-image"
            src={imageSrc}
            alt={filename}
            draggable="false"
            onerror={handleImageError}
        />
        {#if previewState?.overlay.showFilename || previewState?.overlay.showRating || previewState?.overlay.showDecision || previewState?.overlay.showMetadataRail}
            <aside class="preview-info" aria-label="Preview image details">
                {#if previewState?.overlay.showFilename}
                    <div class="info-primary">{filename}</div>
                {/if}
                {#if previewState?.overlay.showRating || previewState?.overlay.showDecision}
                    <div class="info-row">
                        {#if previewState?.overlay.showRating}
                            <span>{rating ? `${rating} stars` : 'Unrated'}</span>
                        {/if}
                        {#if previewState?.overlay.showDecision}
                            <span>{decision}</span>
                        {/if}
                    </div>
                {/if}
                {#if previewState?.overlay.showMetadataRail}
                    <div class="info-row">
                        <span>{dimensions}</span>
                        <span>{image.image.format}</span>
                    </div>
                {/if}
            </aside>
        {/if}
    {:else if loadState === 'loading'}
        <div class="preview-message">Loading</div>
    {:else if loadState === 'missing'}
        <div class="preview-message">Image unavailable</div>
    {:else if loadState === 'error'}
        <div class="preview-message">Preview unavailable</div>
    {:else if loadState === 'blanked'}
        <div class="preview-message">Preview blanked</div>
    {:else}
        <div class="preview-message">No image selected</div>
    {/if}
</div>

<style>
    .preview-display {
        width: 100vw;
        height: 100vh;
        background: var(--bg);
        color: var(--text);
        display: grid;
        place-items: center;
        overflow: hidden;
        position: relative;
    }

    .preview-image {
        max-width: 100vw;
        max-height: 100vh;
        width: auto;
        height: auto;
        object-fit: contain;
        user-select: none;
    }

    .preview-message {
        color: var(--text-secondary);
        font-size: 13px;
        text-transform: uppercase;
        letter-spacing: 0;
    }

    .preview-info {
        position: absolute;
        right: 16px;
        bottom: 16px;
        width: min(360px, calc(100vw - 32px));
        padding: 12px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--text);
        display: flex;
        flex-direction: column;
        gap: 8px;
        font-size: 12px;
        line-height: 1.4;
    }

    .info-primary {
        font-weight: 700;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .info-row {
        color: var(--text-secondary);
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
    }
</style>
