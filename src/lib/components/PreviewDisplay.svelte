<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { save as saveDialog } from '@tauri-apps/plugin-dialog';
    import { onMount, tick } from 'svelte';
    import { toPng } from 'html-to-image';
    import {
        getGenerationRun,
        getImageHistogram,
        getImagesByIds,
        getPreviewState,
        isRawFormat,
        listImageTags,
        savePngToPath,
        setAppSetting,
        updatePreviewState,
        type GenerationRun,
        type ImageHistogram,
        type ImageTag,
        type ImageWithFile,
        type PreviewDisplayLayout,
        type PreviewDisplayMode,
        type PreviewState,
    } from '$lib/api';
    import { buildHtmlToImageOptions, formatExportError } from '$lib/export-renderer';
    import { histogramPolyline } from '$lib/histogram-utils';
    import {
        clampPreviewDisplayPan,
        clampPreviewDisplayZoom,
        isPreviewDisplayPresetCycleShortcut,
        nextPreviewDisplayPresetMode,
        overlayForPreviewDisplayMode,
        previewDisplayNormalizedFocus,
        previewDisplayPanForNormalizedFocus,
        previewDisplayImageSourcePath,
        previewDisplayRailVisible,
        previewDisplayZoomedSize,
        previewStateImageIds,
    } from '$lib/preview-display';
    import { PREVIEW_DISPLAY_MODE_SETTING, PREVIEW_DISPLAY_OVERLAY_SETTING } from '$lib/preview-display-store';

    type DisplayLoadState = 'loading' | 'empty' | 'ready' | 'missing' | 'error' | 'blanked';
    type CaptureDestination = 'clipboard' | 'png';

    let previewState = $state<PreviewState | null>(null);
    let image = $state<ImageWithFile | null>(null);
    let displayImages = $state<ImageWithFile[]>([]);
    let generationRun = $state<GenerationRun | null>(null);
    let tags = $state<ImageTag[]>([]);
    let histogram = $state<ImageHistogram | null>(null);
    let loadState = $state<DisplayLoadState>('loading');
    let sourceLoadFailures = $state<Record<string, boolean>>({});
    let requestSeq = 0;
    let headerVisible = $state(true);
    let pointerOverHeader = $state(false);
    let blankMessageVisible = $state(false);
    let capturing = $state(false);
    let captureStatus = $state('');
    let captureStatusTimer: ReturnType<typeof setTimeout> | null = null;
    let stageEl: HTMLElement | undefined = $state();
    let previewZoom = $state(1);
    let previewPanX = $state(0);
    let previewPanY = $state(0);
    let stageWidth = $state(0);
    let stageHeight = $state(0);
    let draggingPreview = $state(false);
    let dragPointerId = $state<number | null>(null);
    let dragStartX = $state(0);
    let dragStartY = $state(0);
    let dragStartPanX = $state(0);
    let dragStartPanY = $state(0);
    let savedPreviewFocus = { x: 0.5, y: 0.5 };
    let savedPreviewZoom = 1;
    let headerHideTimer: ReturnType<typeof setTimeout> | null = null;
    let blankMessageTimer: ReturnType<typeof setTimeout> | null = null;
    let captureRoot: HTMLDivElement;

    let imageSrc = $derived(image ? displayImageSrc(image) : '');
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
    let previewTransform = $derived(
        `translate(${previewPanX}px, ${previewPanY}px) scale(${previewZoom})`
    );
    let previewZoomLabel = $derived(`${Math.round(previewZoom * 100)}%`);

    function displayImageSrc(img: ImageWithFile): string {
        return convertFileSrc(previewDisplayImageSourcePath(img, sourceLoadFailures[img.image.id] === true));
    }

    function displayFilename(img: ImageWithFile): string {
        return img.path.split('/').pop() ?? img.path;
    }

    function layoutLabel(layout: PreviewDisplayLayout): string {
        if (layout === 'compare') return 'Compare';
        if (layout === 'grid') return 'Grid';
        return 'Single';
    }

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

    function zoomableSinglePreview() {
        return loadState === 'ready' && !!image && (previewState?.layout === 'single' || displayImages.length <= 1);
    }

    function currentImageSize() {
        if (!image) return null;
        return { width: image.image.width, height: image.image.height };
    }

    function currentViewportSize() {
        return { width: stageWidth, height: stageHeight };
    }

    function rememberPreviewView() {
        const imageSize = currentImageSize();
        const viewport = currentViewportSize();
        if (!imageSize || viewport.width <= 0 || viewport.height <= 0) return;

        savedPreviewZoom = clampPreviewDisplayZoom(previewZoom);
        savedPreviewFocus = previewDisplayNormalizedFocus(
            imageSize,
            viewport,
            savedPreviewZoom,
            { x: previewPanX, y: previewPanY }
        );
    }

    function applySavedPreviewView() {
        const imageSize = currentImageSize();
        const viewport = currentViewportSize();
        previewZoom = clampPreviewDisplayZoom(savedPreviewZoom);

        if (!imageSize || viewport.width <= 0 || viewport.height <= 0 || previewZoom <= 1) {
            previewPanX = 0;
            previewPanY = 0;
            return;
        }

        const nextPan = previewDisplayPanForNormalizedFocus(
            imageSize,
            viewport,
            previewZoom,
            savedPreviewFocus
        );
        previewPanX = nextPan.x;
        previewPanY = nextPan.y;
    }

    function clampCurrentPreviewPan() {
        const imageSize = currentImageSize();
        const viewport = currentViewportSize();
        if (!imageSize || viewport.width <= 0 || viewport.height <= 0) {
            previewPanX = 0;
            previewPanY = 0;
            return;
        }

        const nextPan = clampPreviewDisplayPan(
            imageSize,
            viewport,
            previewZoom,
            { x: previewPanX, y: previewPanY }
        );
        previewPanX = nextPan.x;
        previewPanY = nextPan.y;
    }

    function resetPreviewZoomToFit() {
        savedPreviewZoom = 1;
        savedPreviewFocus = { x: 0.5, y: 0.5 };
        previewZoom = 1;
        previewPanX = 0;
        previewPanY = 0;
    }

    function applyPreviewZoom(nextZoom: number, pointer?: { x: number; y: number }) {
        const imageSize = currentImageSize();
        const viewport = currentViewportSize();
        const previousZoom = previewZoom;
        const zoom = clampPreviewDisplayZoom(nextZoom);

        if (!imageSize || viewport.width <= 0 || viewport.height <= 0) {
            previewZoom = zoom;
            previewPanX = 0;
            previewPanY = 0;
            rememberPreviewView();
            return;
        }

        if (zoom <= 1) {
            resetPreviewZoomToFit();
            return;
        }

        if (pointer && previousZoom > 0) {
            const previousZoomed = previewDisplayZoomedSize(imageSize, viewport, previousZoom);
            const focus = previousZoomed.width > 0 && previousZoomed.height > 0
                ? {
                    x: Math.max(0, Math.min(1, 0.5 + (pointer.x - previewPanX) / previousZoomed.width)),
                    y: Math.max(0, Math.min(1, 0.5 + (pointer.y - previewPanY) / previousZoomed.height)),
                }
                : savedPreviewFocus;
            const nextZoomed = previewDisplayZoomedSize(imageSize, viewport, zoom);
            const nextPan = clampPreviewDisplayPan(imageSize, viewport, zoom, {
                x: pointer.x - (focus.x - 0.5) * nextZoomed.width,
                y: pointer.y - (focus.y - 0.5) * nextZoomed.height,
            });
            previewZoom = zoom;
            previewPanX = nextPan.x;
            previewPanY = nextPan.y;
        } else {
            previewZoom = zoom;
            const nextPan = previewDisplayPanForNormalizedFocus(imageSize, viewport, zoom, savedPreviewFocus);
            previewPanX = nextPan.x;
            previewPanY = nextPan.y;
        }

        rememberPreviewView();
    }

    function zoomPreviewByFactor(factor: number) {
        applyPreviewZoom(previewZoom * factor);
    }

    function handlePreviewPointerMove(event: PointerEvent) {
        showPreviewHeader();
        if (!draggingPreview || dragPointerId !== event.pointerId) return;
        const imageSize = currentImageSize();
        const viewport = currentViewportSize();
        if (!imageSize) return;

        const nextPan = clampPreviewDisplayPan(imageSize, viewport, previewZoom, {
            x: dragStartPanX + (event.clientX - dragStartX),
            y: dragStartPanY + (event.clientY - dragStartY),
        });
        previewPanX = nextPan.x;
        previewPanY = nextPan.y;
        rememberPreviewView();
    }

    function handlePreviewPointerDown(event: PointerEvent) {
        if (event.button !== 0 || previewZoom <= 1 || !zoomableSinglePreview()) return;
        event.preventDefault();
        draggingPreview = true;
        dragPointerId = event.pointerId;
        dragStartX = event.clientX;
        dragStartY = event.clientY;
        dragStartPanX = previewPanX;
        dragStartPanY = previewPanY;
        (event.currentTarget as HTMLImageElement | null)?.setPointerCapture(event.pointerId);
    }

    function stopPreviewDrag(event?: PointerEvent) {
        if (!draggingPreview) return;
        if (event && dragPointerId !== event.pointerId) return;
        draggingPreview = false;
        dragPointerId = null;
        rememberPreviewView();
    }

    function handlePreviewWheel(event: WheelEvent) {
        if ((event.target as HTMLElement | null)?.closest('.preview-info')) return;
        if (!zoomableSinglePreview()) return;
        event.preventDefault();
        const rect = stageEl?.getBoundingClientRect();
        const factor = event.deltaY < 0 ? 1.15 : 1 / 1.15;
        applyPreviewZoom(previewZoom * factor, rect ? {
            x: event.clientX - rect.left - rect.width / 2,
            y: event.clientY - rect.top - rect.height / 2,
        } : undefined);
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

    function clearCaptureStatusTimer() {
        if (!captureStatusTimer) return;
        clearTimeout(captureStatusTimer);
        captureStatusTimer = null;
    }

    function showCaptureStatus(message: string) {
        clearCaptureStatusTimer();
        captureStatus = message;
        captureStatusTimer = setTimeout(() => {
            captureStatus = '';
            captureStatusTimer = null;
        }, 3000);
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
        sourceLoadFailures = {};
        resetDetails();

        if (next.blanked) {
            rememberPreviewView();
            requestSeq++;
            image = null;
            displayImages = [];
            loadState = 'blanked';
            showBlankMessageTemporarily();
            return;
        }

        hideBlankMessage();

        const requestedIds = previewStateImageIds(next);
        if (requestedIds.length === 0) {
            rememberPreviewView();
            requestSeq++;
            image = null;
            displayImages = [];
            loadState = 'empty';
            return;
        }

        const seq = ++requestSeq;
        loadState = 'loading';

        try {
            const records = await getImagesByIds(requestedIds);
            if (seq !== requestSeq) return;
            rememberPreviewView();
            const byId = new Map(records.map((record) => [record.image.id, record]));
            displayImages = requestedIds
                .map((id) => byId.get(id))
                .filter((record): record is ImageWithFile => record !== undefined);
            image = displayImages[0] ?? null;
            loadState = image ? 'ready' : 'missing';
            applySavedPreviewView();
            if (image) {
                await loadDetails(next, image.image.id, seq);
            }
        } catch (e) {
            if (seq !== requestSeq) return;
            console.error('Failed to load Preview Display image:', e);
            image = null;
            displayImages = [];
            loadState = 'error';
        }
    }

    function handleImageError(img: ImageWithFile) {
        const canFallback = !sourceLoadFailures[img.image.id] && !isRawFormat(img.image.format) && !!img.thumbnail_path;
        if (canFallback) {
            sourceLoadFailures = { ...sourceLoadFailures, [img.image.id]: true };
            return;
        }
        if (displayImages.length <= 1) loadState = 'error';
    }

    function pngBase64(dataUrl: string): string {
        return dataUrl.split(',')[1] ?? '';
    }

    async function capturePreviewPng(): Promise<string> {
        if (!captureRoot || loadState !== 'ready') {
            throw new Error('Preview Display has no rendered content to capture');
        }
        capturing = true;
        await tick();
        try {
            return await toPng(
                captureRoot,
                buildHtmlToImageOptions(window.innerWidth, window.innerHeight)
            );
        } finally {
            capturing = false;
        }
    }

    async function copyPreviewToClipboard(dataUrl: string) {
        if (!navigator.clipboard?.write || typeof ClipboardItem === 'undefined') {
            throw new Error('Image clipboard write is unavailable in this webview');
        }
        const blob = await (await fetch(dataUrl)).blob();
        await navigator.clipboard.write([
            new ClipboardItem({ [blob.type || 'image/png']: blob }),
        ]);
    }

    async function exportPreviewPng(dataUrl: string) {
        const stamp = new Date().toISOString().replace(/[:.]/g, '-');
        const target = await saveDialog({
            title: 'Export Preview Display',
            defaultPath: `cull-preview-display-${stamp}.png`,
            filters: [{ name: 'PNG Image', extensions: ['png'] }],
        });
        if (!target) return;
        const written = await savePngToPath(target, pngBase64(dataUrl));
        showCaptureStatus(`Exported ${written.split('/').pop() ?? written}`);
    }

    async function handleCaptureRequest(destination: CaptureDestination) {
        try {
            const dataUrl = await capturePreviewPng();
            if (destination === 'clipboard') {
                await copyPreviewToClipboard(dataUrl);
                showCaptureStatus('Copied Monitor Image');
                return;
            }
            await exportPreviewPng(dataUrl);
        } catch (e) {
            showCaptureStatus(`Capture failed: ${formatExportError(e)}`);
            console.error('Failed to capture Preview Display:', e);
        }
    }

    async function applyPreviewPreset(displayMode: PreviewDisplayMode) {
        if (!previewState) return;
        const overlay = overlayForPreviewDisplayMode(displayMode);
        const next = await updatePreviewState(
            previewState.image_id,
            displayMode,
            overlay,
            previewState.frozen,
            previewState.blanked,
            previewState.layout,
            previewStateImageIds(previewState)
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
        if (isPreviewDisplayPresetCycleShortcut(event)) {
            event.preventDefault();
            if (!previewState) return;
            const displayMode = nextPreviewDisplayPresetMode(previewState.display_mode);
            applyPreviewPreset(displayMode).catch((e) => {
                console.error('Failed to cycle Preview Display preset:', e);
            });
            return;
        }

        if (event.metaKey || event.ctrlKey || event.altKey) return;
        if (!zoomableSinglePreview()) return;
        if (event.key === '+' || event.key === '=') {
            event.preventDefault();
            zoomPreviewByFactor(1.15);
        } else if (event.key === '-' || event.key === '_') {
            event.preventDefault();
            zoomPreviewByFactor(1 / 1.15);
        } else if (event.key === '0' || event.key === 'Home') {
            event.preventDefault();
            resetPreviewZoomToFit();
        }
    }

    onMount(() => {
        const observer = new ResizeObserver((entries) => {
            const rect = entries[0]?.contentRect;
            if (!rect) return;
            rememberPreviewView();
            stageWidth = rect.width;
            stageHeight = rect.height;
            applySavedPreviewView();
            clampCurrentPreviewPan();
        });
        if (stageEl) observer.observe(stageEl);

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
        const captureUnlisten = listen<{ destination: CaptureDestination }>('preview-display:capture-request', (event) => {
            handleCaptureRequest(event.payload.destination).catch((e) => {
                console.error('Failed to handle Preview Display capture request:', e);
            });
        });
        scheduleHeaderHide();

        return () => {
            stateUnlisten.then((fn) => fn());
            captureUnlisten.then((fn) => fn());
            observer.disconnect();
            clearHeaderHideTimer();
            clearBlankMessageTimer();
            clearCaptureStatusTimer();
        };
    });
</script>

<svelte:window onkeydown={handlePreviewKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
    bind:this={captureRoot}
    class="preview-display"
    class:capturing
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
    <main
        bind:this={stageEl}
        class="preview-stage"
        onwheel={handlePreviewWheel}
    >
        {#if loadState === 'ready' && image}
            {#if previewState?.layout === 'single' || displayImages.length === 1}
                <img
                    class="preview-image"
                    class:zoomed={previewZoom > 1}
                    class:dragging={draggingPreview}
                    src={imageSrc}
                    alt={filename}
                    draggable="false"
                    title={previewZoomLabel}
                    style="transform: {previewTransform};"
                    onerror={() => handleImageError(image!)}
                    onpointerdown={handlePreviewPointerDown}
                    onpointerup={stopPreviewDrag}
                    onpointercancel={stopPreviewDrag}
                />
            {:else}
                <div
                    class="preview-layout"
                    data-layout={previewState?.layout}
                    aria-label={`Preview Display ${layoutLabel(previewState?.layout ?? 'single')} layout`}
                >
                    {#each displayImages as displayImage (displayImage.image.id)}
                        <figure class="preview-tile">
                            <img
                                class="preview-tile-image"
                                src={displayImageSrc(displayImage)}
                                alt={displayFilename(displayImage)}
                                draggable="false"
                                onerror={() => handleImageError(displayImage)}
                            />
                            {#if previewState?.overlay.showFilename || previewState?.overlay.showRating || previewState?.overlay.showDecision}
                                <figcaption class="preview-tile-caption">
                                    {#if previewState?.overlay.showFilename}
                                        <span>{displayFilename(displayImage)}</span>
                                    {/if}
                                    {#if previewState?.overlay.showRating}
                                        <span>{displayImage.selection?.star_rating ? `${displayImage.selection.star_rating} stars` : 'Unrated'}</span>
                                    {/if}
                                    {#if previewState?.overlay.showDecision}
                                        <span>{displayImage.selection?.decision ?? 'undecided'}</span>
                                    {/if}
                                </figcaption>
                            {/if}
                        </figure>
                    {/each}
                </div>
            {/if}
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
        {#if captureStatus}
            <div class="capture-status">{captureStatus}</div>
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

    .preview-display.capturing .preview-header {
        opacity: 0;
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
        transform-origin: center center;
        touch-action: none;
        user-select: none;
        will-change: transform;
        cursor: zoom-in;
    }

    .preview-image.zoomed {
        cursor: grab;
    }

    .preview-image.dragging {
        cursor: grabbing;
    }

    .preview-layout {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        padding: 16px;
        box-sizing: border-box;
        display: grid;
        gap: 8px;
    }

    .preview-layout[data-layout="compare"] {
        grid-template-columns: repeat(2, minmax(0, 1fr));
        grid-template-rows: minmax(0, 1fr);
    }

    .preview-layout[data-layout="grid"] {
        grid-template-columns: repeat(2, minmax(0, 1fr));
        grid-template-rows: repeat(2, minmax(0, 1fr));
    }

    .preview-tile {
        min-width: 0;
        min-height: 0;
        margin: 0;
        border: 1px solid var(--border);
        background: var(--surface);
        display: grid;
        grid-template-rows: minmax(0, 1fr) auto;
        overflow: hidden;
    }

    .preview-tile-image {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        object-fit: contain;
        user-select: none;
    }

    .preview-tile-caption {
        min-height: 32px;
        padding: 6px 8px;
        border-top: 1px solid var(--border);
        color: var(--text-secondary);
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
        align-items: center;
        font-size: 11px;
        line-height: 1.25;
        overflow: hidden;
    }

    .preview-tile-caption span {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .preview-message {
        color: var(--text-secondary);
        font-size: 13px;
        text-transform: uppercase;
        letter-spacing: 0;
    }

    .capture-status {
        position: absolute;
        left: 16px;
        top: calc(var(--macos-titlebar-safe-area) + 12px);
        z-index: 25;
        max-width: min(520px, calc(100vw - 32px));
        padding: 8px 10px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--text);
        font-size: 12px;
        line-height: 1.35;
        overflow-wrap: anywhere;
    }

    .preview-display.capturing .capture-status {
        display: none;
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
