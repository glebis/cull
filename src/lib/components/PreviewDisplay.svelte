<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { onMount } from 'svelte';
    import {
        getGenerationRun,
        getImageHistogram,
        getImagesByIds,
        getPreviewState,
        isRawFormat,
        listImageTags,
        setAppSetting,
        updatePreviewState,
        type GenerationRun,
        type ImageHistogram,
        type ImageTag,
        type ImageWithFile,
        type PreviewDisplayMode,
        type PreviewState,
    } from '$lib/api';
    import { histogramPolyline } from '$lib/histogram-utils';
    import {
        isPreviewDisplayPresetCycleShortcut,
        nextPreviewDisplayPresetMode,
        overlayForPreviewDisplayMode,
        previewDisplayImageSourcePath,
        previewDisplayRailVisible,
    } from '$lib/preview-display';
    import { PREVIEW_DISPLAY_MODE_SETTING, PREVIEW_DISPLAY_OVERLAY_SETTING } from '$lib/preview-display-store';

    type DisplayLoadState = 'loading' | 'empty' | 'ready' | 'missing' | 'error' | 'blanked';

    let previewState = $state<PreviewState | null>(null);
    let image = $state<ImageWithFile | null>(null);
    let generationRun = $state<GenerationRun | null>(null);
    let tags = $state<ImageTag[]>([]);
    let histogram = $state<ImageHistogram | null>(null);
    let loadState = $state<DisplayLoadState>('loading');
    let sourceLoadFailed = $state(false);
    let requestSeq = 0;
    let headerVisible = $state(true);
    let pointerOverHeader = $state(false);
    let blankMessageVisible = $state(false);
    let headerHideTimer: ReturnType<typeof setTimeout> | null = null;
    let blankMessageTimer: ReturnType<typeof setTimeout> | null = null;

    let imageSrc = $derived(image ? convertFileSrc(previewDisplayImageSourcePath(image, sourceLoadFailed)) : '');
    let filename = $derived(image?.path.split('/').pop() ?? '');
    let rating = $derived(image?.selection?.star_rating ?? 0);
    let decision = $derived(image?.selection?.decision ?? 'undecided');
    let dimensions = $derived(image ? `${image.image.width}x${image.image.height}` : '');
    let promptPreview = $derived(generationRun?.prompt ?? image?.image.ai_prompt ?? '');
    let sourceSummary = $derived([
        image?.source_label,
        generationRun?.provider,
        generationRun?.model,
    ].filter(Boolean).join(' / '));
    let tagSummary = $derived(tags.map((tag) => tag.name).join(', '));
    let railVisible = $derived(previewState ? previewDisplayRailVisible(previewState.overlay) : false);
    let lumaPoints = $derived(histogram ? histogramPolyline(histogram.luma, 64) : '');
    let redPoints = $derived(histogram ? histogramPolyline(histogram.red, 64) : '');
    let greenPoints = $derived(histogram ? histogramPolyline(histogram.green, 64) : '');
    let bluePoints = $derived(histogram ? histogramPolyline(histogram.blue, 64) : '');

    function resetDetails() {
        generationRun = null;
        tags = [];
        histogram = null;
    }

    function clearHeaderHideTimer() {
        if (!headerHideTimer) return;
        clearTimeout(headerHideTimer);
        headerHideTimer = null;
    }

    function scheduleHeaderHide() {
        clearHeaderHideTimer();
        headerHideTimer = setTimeout(() => {
            if (!pointerOverHeader) headerVisible = false;
        }, 1500);
    }

    function showPreviewHeader() {
        headerVisible = true;
        if (!pointerOverHeader) scheduleHeaderHide();
    }

    function handlePreviewPointerMove() {
        showPreviewHeader();
    }

    function handleHeaderPointerEnter() {
        pointerOverHeader = true;
        headerVisible = true;
        clearHeaderHideTimer();
    }

    function handleHeaderPointerLeave() {
        pointerOverHeader = false;
        scheduleHeaderHide();
    }

    function clearBlankMessageTimer() {
        if (!blankMessageTimer) return;
        clearTimeout(blankMessageTimer);
        blankMessageTimer = null;
    }

    function hideBlankMessage() {
        clearBlankMessageTimer();
        blankMessageVisible = false;
    }

    function showBlankMessageTemporarily() {
        clearBlankMessageTimer();
        blankMessageVisible = true;
        blankMessageTimer = setTimeout(() => {
            blankMessageVisible = false;
            blankMessageTimer = null;
        }, 3000);
    }

    async function loadDetails(next: PreviewState, imageId: string, seq: number) {
        const runPromise = next.overlay.showPrompt || next.overlay.showSource
            ? getGenerationRun(imageId).catch(() => null)
            : Promise.resolve(null);
        const tagsPromise = next.overlay.showTags
            ? listImageTags(imageId).catch(() => [])
            : Promise.resolve([]);
        const histogramPromise = next.overlay.showHistogram
            ? getImageHistogram(imageId).catch(() => null)
            : Promise.resolve(null);

        const [run, nextTags, nextHistogram] = await Promise.all([runPromise, tagsPromise, histogramPromise]);
        if (seq !== requestSeq) return;
        generationRun = run;
        tags = nextTags;
        histogram = nextHistogram;
    }

    async function applyPreviewState(next: PreviewState) {
        previewState = next;
        sourceLoadFailed = false;
        resetDetails();

        if (next.blanked) {
            requestSeq++;
            image = null;
            loadState = 'blanked';
            showBlankMessageTemporarily();
            return;
        }

        hideBlankMessage();

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
            if (image) {
                await loadDetails(next, image.image.id, seq);
            }
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

    async function applyPreviewPreset(displayMode: PreviewDisplayMode) {
        if (!previewState) return;
        const overlay = overlayForPreviewDisplayMode(displayMode);
        const next = await updatePreviewState(
            previewState.image_id,
            displayMode,
            overlay,
            previewState.frozen,
            previewState.blanked
        );
        await applyPreviewState(next);
        try {
            await setAppSetting(PREVIEW_DISPLAY_MODE_SETTING, displayMode);
            await setAppSetting(PREVIEW_DISPLAY_OVERLAY_SETTING, JSON.stringify(overlay));
        } catch (e) {
            console.warn('Failed to persist Preview Display preset:', e);
        }
    }

    function handlePreviewKeydown(event: KeyboardEvent) {
        if (!isPreviewDisplayPresetCycleShortcut(event)) return;
        event.preventDefault();
        if (!previewState) return;
        const displayMode = nextPreviewDisplayPresetMode(previewState.display_mode);
        applyPreviewPreset(displayMode).catch((e) => {
            console.error('Failed to cycle Preview Display preset:', e);
        });
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
        scheduleHeaderHide();

        return () => {
            stateUnlisten.then((fn) => fn());
            clearHeaderHideTimer();
            clearBlankMessageTimer();
        };
    });
</script>

<svelte:window onkeydown={handlePreviewKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
    class="preview-display"
    data-state={loadState}
    onpointermove={handlePreviewPointerMove}
>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header
        class="preview-header"
        class:hidden={!headerVisible}
        data-tauri-drag-region="deep"
        aria-label="Preview Display window header"
        onpointerenter={handleHeaderPointerEnter}
        onpointerleave={handleHeaderPointerLeave}
    >
        <span class="preview-title" data-tauri-drag-region="deep">Preview Display</span>
    </header>
    <main class="preview-stage">
        {#if loadState === 'ready' && image}
            <img
                class="preview-image"
                src={imageSrc}
                alt={filename}
                draggable="false"
                onerror={handleImageError}
            />
            {#if previewState?.overlay.showFilename || previewState?.overlay.showRating || previewState?.overlay.showDecision || railVisible}
                <aside
                    class="preview-info"
                    aria-label="Preview image details"
                    data-side={previewState?.overlay.railSide}
                    data-width={previewState?.overlay.railWidth}
                    data-text={previewState?.overlay.railTextSize}
                >
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
                    {#if railVisible}
                        <div class="info-grid">
                            {#if previewState?.overlay.showDimensions}
                                <div class="label">Dimensions</div>
                                <div class="value">{dimensions}</div>
                            {/if}
                            {#if previewState?.overlay.showFormat}
                                <div class="label">Format</div>
                                <div class="value">{image.image.format}</div>
                            {/if}
                            {#if previewState?.overlay.showSource}
                                <div class="label">Source</div>
                                <div class="value line-clamp">{sourceSummary || 'Unknown'}</div>
                            {/if}
                            {#if previewState?.overlay.showPrompt}
                                <div class="label">Prompt</div>
                                <div class="value prompt-preview line-clamp">{promptPreview || 'No prompt'}</div>
                            {/if}
                            {#if previewState?.overlay.showTags}
                                <div class="label">Tags</div>
                                <div class="value tag-list line-clamp">{tagSummary || 'No tags'}</div>
                            {/if}
                        </div>
                        {#if previewState?.overlay.showHistogram}
                            <div class="histogram-panel" aria-label="RGB histogram">
                                {#if histogram}
                                    <svg class="histogram-svg" viewBox="0 0 255 64" preserveAspectRatio="none">
                                        <polyline class="histogram-line luma" points={lumaPoints} />
                                        <polyline class="histogram-line red" points={redPoints} />
                                        <polyline class="histogram-line green" points={greenPoints} />
                                        <polyline class="histogram-line blue" points={bluePoints} />
                                    </svg>
                                    <div class="histogram-source">{histogram.source}</div>
                                {:else}
                                    <div class="value">Histogram unavailable</div>
                                {/if}
                            </div>
                        {/if}
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
            {#if blankMessageVisible}
                <div class="preview-message">Preview is Blanked</div>
            {/if}
        {:else}
            <div class="preview-message">No image selected</div>
        {/if}
    </main>
</div>

<style>
    .preview-display {
        width: 100vw;
        height: 100vh;
        background: var(--bg);
        color: var(--text);
        display: grid;
        grid-template-rows: minmax(0, 1fr);
        overflow: hidden;
        position: relative;
    }

    .preview-header {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        z-index: 20;
        height: var(--macos-titlebar-safe-area);
        padding-left: var(--macos-window-controls-width);
        padding-right: var(--spacing);
        border-bottom: 1px solid var(--border);
        background: var(--surface);
        display: flex;
        align-items: center;
        justify-content: center;
        user-select: none;
        -webkit-user-select: none;
        transition: opacity 150ms ease, transform 150ms ease;
    }

    .preview-header.hidden {
        opacity: 0;
        pointer-events: none;
        transform: translateY(calc(-1 * var(--macos-titlebar-safe-area)));
    }

    .preview-title {
        color: var(--text-secondary);
        font-size: 11px;
        line-height: 1;
        text-transform: uppercase;
        letter-spacing: 0;
    }

    .preview-stage {
        width: 100%;
        height: 100%;
        min-height: 0;
        display: grid;
        place-items: center;
        overflow: hidden;
        position: relative;
    }

    .preview-image {
        max-width: 100%;
        max-height: 100%;
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
        max-height: calc(100% - 32px);
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
        overflow: hidden;
    }

    .preview-info[data-side="left"] {
        left: 16px;
        right: auto;
    }

    .preview-info[data-width="narrow"] {
        width: min(280px, calc(100vw - 32px));
    }

    .preview-info[data-width="wide"] {
        width: min(460px, calc(100vw - 32px));
    }

    .preview-info[data-text="small"] {
        font-size: 11px;
    }

    .preview-info[data-text="large"] {
        font-size: 13px;
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

    .info-grid {
        display: grid;
        grid-template-columns: auto 1fr;
        gap: 4px 10px;
        min-width: 0;
    }

    .label {
        color: var(--text-secondary);
        text-transform: uppercase;
        font-size: 10px;
    }

    .value {
        min-width: 0;
        color: var(--text);
        overflow-wrap: anywhere;
    }

    .line-clamp {
        display: -webkit-box;
        line-clamp: 3;
        -webkit-line-clamp: 3;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }

    .histogram-panel {
        border-top: 1px solid var(--border);
        padding-top: 8px;
    }

    .histogram-svg {
        width: 100%;
        height: 72px;
        display: block;
        background: var(--bg);
        border: 1px solid var(--border);
    }

    .histogram-line {
        fill: none;
        stroke-width: 1.5;
        vector-effect: non-scaling-stroke;
    }

    .histogram-line.luma {
        stroke: var(--text-secondary);
    }

    .histogram-line.red {
        stroke: var(--red);
    }

    .histogram-line.green {
        stroke: var(--green);
    }

    .histogram-line.blue {
        stroke: var(--blue);
    }

    .histogram-source {
        color: var(--text-secondary);
        font-size: 10px;
        margin-top: 4px;
        text-transform: uppercase;
    }
</style>
