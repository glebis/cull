<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { onMount } from 'svelte';
    import ContextMenu from './ContextMenu.svelte';
    import PromptResubmitDialog from './PromptResubmitDialog.svelte';
    import GenerationResultsStrip from './GenerationResultsStrip.svelte';
    import { images, focusedIndex, focusedImage, statusHint, loupeScale, loupePanX, loupePanY, navigateBack, showDetectionBoxes, showDetectionInspector, nsfwMode, showToast, selectedIds } from '$lib/stores';
    import { getDetections, getVisionMetadata, cropImage, getImagesByIds, getGenerationRun, isRawFormat } from '$lib/api';
    import type { Detection, GenerationRun } from '$lib/api';
    import {
        clientToImagePoint,
        chooseLoupeImagePath,
        cropRectFromImagePoints,
        cropSelectionPercentFromImagePoints,
        moveCropRect,
        resizeCropRectFromHandle
    } from '$lib/view-utils';
    import type { CropPoint, CropRect, CropResizeHandle } from '$lib/view-utils';
    import { recordImageLoadFailure } from '$lib/diagnostics';

    let dragging = $state(false);
    let dragStartX = $state(0);
    let dragStartY = $state(0);
    let panStartX = $state(0);
    let panStartY = $state(0);

    let image = $derived($focusedImage);
    let isRaw = $derived(isRawFormat(image?.image.format ?? ''));
    let sourceLoadFailed = $state(false);
    let src = $derived(image ? convertFileSrc(chooseLoupeImagePath(image, isRaw, sourceLoadFailed)) : '');
    let filename = $derived(image?.path.split('/').pop() ?? '');
    let dimensions = $derived(image ? `${image.image.width}x${image.image.height}` : '');
    let format = $derived(image?.image.format ?? '');
    let rating = $derived(image?.selection?.star_rating ?? 0);
    let decision = $derived(image?.selection?.decision ?? 'undecided');

    const SOURCE_DISPLAY: Record<string, string> = {
        gpt_image_2: 'GPT-image-2',
        dalle_3: 'DALL-E 3',
        dalle: 'DALL-E',
        openai: 'OpenAI',
        stable_diffusion: 'Stable Diffusion',
        comfyui: 'ComfyUI',
        midjourney: 'Midjourney',
        nanobanana: 'Nanobanana',
    };
    const MIN_CROP_SIZE = 10;
    const CROP_HANDLES: CropResizeHandle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'];

    type CropDragState =
        | { type: 'draw'; anchor: CropPoint }
        | { type: 'move'; startPointer: CropPoint; startRect: CropRect }
        | { type: 'resize'; handle: CropResizeHandle; startRect: CropRect };

    let sourceDisplay = $derived(image?.source_label ? SOURCE_DISPLAY[image.source_label] ?? image.source_label : null);

    let generationRun = $state<GenerationRun | null>(null);

    let prompt = $derived(generationRun?.prompt ?? image?.image.ai_prompt ?? null);
    let genModel = $derived(generationRun?.model ?? null);
    let genProvider = $derived(generationRun?.provider ?? null);
    let genSeed = $derived(generationRun?.seed ?? null);
    let promptExpanded = $state(false);
    let resubmitVisible = $state(false);
    let promptTruncated = $derived(prompt && prompt.length > 80 ? prompt.slice(0, 80) + '…' : prompt);

    let detections = $state<Detection[]>([]);
    let nsfwDetections = $state<Detection[]>([]);
    let detectionsLoaded = $state(false);
    let isNsfw = $derived(nsfwDetections.length > 0);
    let spaceHeld = $state(false);
    let imgEl: HTMLImageElement | undefined = $state();
    let visionMeta = $state<[string, string, string][]>([]);
    let hideOverlays = $state(false);
    let toastDecision = $state<string | null>(null);
    let toastKey = $state(0);

    let prevDecision = $state('');
    let prevImageIdForToast = $state('');
    $effect(() => {
        const d = decision;
        const id = image?.image.id ?? '';
        if (id !== prevImageIdForToast) {
            prevImageIdForToast = id;
            prevDecision = d;
            return;
        }
        if (d !== prevDecision && prevDecision !== '' && d !== 'undecided') {
            toastDecision = d;
            toastKey++;
            setTimeout(() => { toastDecision = null; }, 800);
        }
        prevDecision = d;
    });

    onMount(() => {
        function toggleOverlays() { hideOverlays = !hideOverlays; }
        window.addEventListener('toggle-loupe-overlays', toggleOverlays);

        function handleImageUpdated() {
            if (imgEl) {
                const currentSrc = imgEl.src;
                imgEl.src = '';
                imgEl.src = currentSrc + '?t=' + Date.now();
            }
            const img = image;
            if (img) {
                getImagesByIds([img.image.id]).then(updated => {
                    if (updated.length > 0) {
                        images.update(all => all.map(i => i.image.id === img.image.id ? updated[0] : i));
                    }
                }).catch(() => {});
            }
        }
        window.addEventListener('image-updated', handleImageUpdated);
        window.addEventListener('enter-crop-mode', enterCropMode);

        return () => {
            window.removeEventListener('toggle-loupe-overlays', toggleOverlays);
            window.removeEventListener('image-updated', handleImageUpdated);
            window.removeEventListener('enter-crop-mode', enterCropMode);
        };
    });

    $effect(() => {
        const id = image?.image.id;
        if (!id) { detections = []; nsfwDetections = []; detectionsLoaded = false; return; }
        detectionsLoaded = false;
        getDetections(id).then(dets => {
            detections = dets.filter(d => !d.class_name.includes('EXPOSED') && !d.class_name.includes('COVERED') && !d.class_name.includes('FACE_') && !d.class_name.includes('BELLY') && !d.class_name.includes('FEET') && !d.class_name.includes('ARMPITS') && !d.class_name.includes('ANUS') && !d.class_name.includes('BUTTOCKS') && !d.class_name.includes('BREAST') && !d.class_name.includes('GENITALIA'));
            nsfwDetections = dets.filter(d => d.class_name.includes('EXPOSED'));
            detectionsLoaded = true;
        }).catch(() => { detections = []; nsfwDetections = []; detectionsLoaded = true; });
        getVisionMetadata(id).then(m => { visionMeta = m; }).catch(() => { visionMeta = []; });
        getGenerationRun(id).then(r => { generationRun = r; }).catch(() => { generationRun = null; });
    });

    let shouldBlur = $derived(
        $nsfwMode === 'blur' && !spaceHeld && detectionsLoaded && isNsfw
    );

    function handleSpaceDown(e: KeyboardEvent) {
        if (e.code === 'Space' && isNsfw && $nsfwMode === 'blur') {
            e.preventDefault();
            spaceHeld = true;
        }
    }
    function handleSpaceUp(e: KeyboardEvent) {
        if (e.code === 'Space') spaceHeld = false;
    }

    $effect(() => {
        const info = `${filename} | ${dimensions} | ${format}`;
        statusHint.set(info);
        return () => statusHint.set(null);
    });

    // Reset pan (but keep zoom) when image changes
    let prevImageId = $state('');
    $effect(() => {
        const id = image?.image.id ?? '';
        if (id !== prevImageId) {
            prevImageId = id;
            sourceLoadFailed = false;
            loupePanX.set(0);
            loupePanY.set(0);
        }
    });

    function handleImageError() {
        const current = image;
        if (!current) return;

        const canFallbackToThumbnail = !isRaw && !sourceLoadFailed && !!current.thumbnail_path;
        const thumbnailWasShown = isRaw || sourceLoadFailed;
        recordImageLoadFailure({
            view: 'loupe',
            image: current,
            assetKind: thumbnailWasShown ? 'thumbnail' : 'source',
            errorKind: 'img_onerror',
            fallbackUsed: canFallbackToThumbnail || sourceLoadFailed,
            fallbackSucceeded: sourceLoadFailed ? false : null,
            phase: thumbnailWasShown ? 'thumbnail' : 'source',
        });

        if (canFallbackToThumbnail) {
            sourceLoadFailed = true;
        }
    }

    function handleWheel(e: WheelEvent) {
        e.preventDefault();
        const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
        loupeScale.update(s => {
            const next = Math.max(0.1, Math.min(20, s * factor));
            if (next <= 1) {
                loupePanX.set(0);
                loupePanY.set(0);
            }
            return next;
        });
    }

    function handleMouseDown(e: MouseEvent) {
        if ($loupeScale <= 1) return;
        dragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        panStartX = $loupePanX;
        panStartY = $loupePanY;
    }

    function handleMouseMove(e: MouseEvent) {
        if (!dragging) return;
        loupePanX.set(panStartX + (e.clientX - dragStartX));
        loupePanY.set(panStartY + (e.clientY - dragStartY));
    }

    function handleMouseUp() {
        dragging = false;
    }

    function handleDblClick() {
        navigateBack();
    }

    // Crop mode
    let cropMode = $state(false);
    let cropStart = $state<CropPoint | null>(null);
    let cropEnd = $state<CropPoint | null>(null);
    let cropDrag = $state<CropDragState | null>(null);
    let cropSpaceHeld = $state(false);
    let cropping = $state(false);
    let currentCropRect = $derived(getCropRect());
    let cropSizeLabel = $derived(currentCropRect ? `${currentCropRect.width} x ${currentCropRect.height} px` : '');
    let canApplyCrop = $derived(
        !!currentCropRect && currentCropRect.width >= MIN_CROP_SIZE && currentCropRect.height >= MIN_CROP_SIZE
    );

    function enterCropMode() {
        if (isRaw) return;
        cropMode = true;
        cropStart = null;
        cropEnd = null;
    }

    function cancelCrop() {
        cropMode = false;
        cropStart = null;
        cropEnd = null;
        cropDrag = null;
        cropSpaceHeld = false;
    }

    function getCropRect() {
        if (!cropStart || !cropEnd || !image) return null;
        return cropRectFromImagePoints(cropStart, cropEnd, image.image.width, image.image.height);
    }

    function getCropSelectionPercent() {
        if (!cropStart || !cropEnd || !image) return null;
        return cropSelectionPercentFromImagePoints(cropStart, cropEnd, image.image.width, image.image.height);
    }

    function getCropPoint(e: MouseEvent): CropPoint | null {
        if (!imgEl || !image) return null;
        return clientToImagePoint(
            e.clientX,
            e.clientY,
            imgEl.getBoundingClientRect(),
            image.image.width,
            image.image.height
        );
    }

    function setCropRect(rect: CropRect) {
        cropStart = { x: rect.x, y: rect.y };
        cropEnd = { x: rect.x + rect.width, y: rect.y + rect.height };
    }

    async function applyCrop() {
        const rect = currentCropRect;
        if (!rect || !image || rect.width < MIN_CROP_SIZE || rect.height < MIN_CROP_SIZE) return;
        cropping = true;
        try {
            await cropImage(image.image.id, rect.x, rect.y, rect.width, rect.height, false);
            showToast('Image cropped', { type: 'info', duration: 2000 });
            window.dispatchEvent(new CustomEvent('image-updated'));
        } catch (e) {
            showToast(`Crop failed: ${e}`, { type: 'error', duration: 5000 });
        }
        cropping = false;
        cancelCrop();
    }

    function handleCropMouseDown(e: MouseEvent) {
        if (!cropMode) return;
        e.preventDefault();
        e.stopPropagation();
        const point = getCropPoint(e);
        if (!point) return;
        const rect = currentCropRect;
        if (cropSpaceHeld && rect) {
            cropDrag = { type: 'move', startPointer: point, startRect: rect };
            return;
        }
        cropDrag = { type: 'draw', anchor: point };
        cropStart = point;
        cropEnd = point;
    }

    function handleCropMouseMove(e: MouseEvent) {
        if (!cropMode || !cropDrag || !image) return;
        e.preventDefault();
        e.stopPropagation();
        const point = getCropPoint(e);
        if (!point) return;
        if (cropDrag.type === 'draw') {
            cropStart = cropDrag.anchor;
            cropEnd = point;
            return;
        }
        if (cropDrag.type === 'move') {
            setCropRect(moveCropRect(
                cropDrag.startRect,
                point.x - cropDrag.startPointer.x,
                point.y - cropDrag.startPointer.y,
                image.image.width,
                image.image.height
            ));
            return;
        }
        setCropRect(resizeCropRectFromHandle(
            cropDrag.startRect,
            cropDrag.handle,
            point,
            image.image.width,
            image.image.height,
            MIN_CROP_SIZE
        ));
    }

    function handleCropMouseUp(e: MouseEvent) {
        e.preventDefault();
        e.stopPropagation();
        cropDrag = null;
        // Selection complete — user clicks Apply or Cancel
    }

    function handleCropSelectionMouseDown(e: MouseEvent) {
        if (!cropMode || !currentCropRect) return;
        e.preventDefault();
        e.stopPropagation();
        const point = getCropPoint(e);
        if (!point) return;
        cropDrag = { type: 'move', startPointer: point, startRect: currentCropRect };
    }

    function handleCropHandleMouseDown(e: MouseEvent, handle: CropResizeHandle) {
        if (!cropMode || !currentCropRect) return;
        e.preventDefault();
        e.stopPropagation();
        cropDrag = { type: 'resize', handle, startRect: currentCropRect };
    }

    $effect(() => {
        if (!cropMode) return;
        function handleCropKeyDown(e: KeyboardEvent) {
            if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                cancelCrop();
                return;
            }
            if (e.code === 'Space') {
                cropSpaceHeld = true;
                if (currentCropRect) {
                    e.preventDefault();
                    e.stopPropagation();
                }
                return;
            }
            if (e.key === 'Enter' || e.key === 'Return' || (e.metaKey && e.key.toLowerCase() === 'k')) {
                e.preventDefault();
                e.stopPropagation();
                void applyCrop();
            }
        }
        function handleCropKeyUp(e: KeyboardEvent) {
            if (e.code === 'Space') {
                cropSpaceHeld = false;
            }
        }
        window.addEventListener('keydown', handleCropKeyDown, true);
        window.addEventListener('keyup', handleCropKeyUp, true);
        return () => {
            window.removeEventListener('keydown', handleCropKeyDown, true);
            window.removeEventListener('keyup', handleCropKeyUp, true);
        };
    });

    let isSelected = $derived(image ? $selectedIds.has(image.image.id) : false);

    async function copyPrompt() {
        if (!prompt) return;
        await navigator.clipboard.writeText(prompt);
        showToast('Prompt copied', { type: 'info', duration: 1500 });
    }

    let ctxMenu = $state({ visible: false, x: 0, y: 0 });

    function handleContextMenu(e: MouseEvent) {
        if (!image) return;
        e.preventDefault();
        ctxMenu = { visible: true, x: e.clientX, y: e.clientY };
    }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<svelte:window onkeydown={handleSpaceDown} onkeyup={handleSpaceUp} />
<div class="loupe-wrapper" class:has-inspector={$showDetectionInspector}>
<!-- svelte-ignore a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
<div
    class="loupe-container"
    onwheel={handleWheel}
    onmousedown={handleMouseDown}
    onmousemove={handleMouseMove}
    onmouseup={handleMouseUp}
    onmouseleave={handleMouseUp}
    ondblclick={handleDblClick}
    oncontextmenu={handleContextMenu}
    class:dragging
    class:zoomed={$loupeScale > 1}
>
    {#if image}
        <div class="image-frame">
            <img
                bind:this={imgEl}
                {src}
                alt={filename}
                draggable="false"
                onerror={handleImageError}
                class:blurred={shouldBlur}
                class:unblurring={detectionsLoaded}
                class:pixel-zoom={$loupeScale > 4}
                style="transform: scale({$loupeScale}) translate({$loupePanX / $loupeScale}px, {$loupePanY / $loupeScale}px);"
            />

            {#if shouldBlur}
                <div class="nsfw-overlay">
                    <div class="nsfw-label">NSFW BLURRED</div>
                    <div class="nsfw-hint">hold Space to peek</div>
                </div>
            {/if}

            {#if $showDetectionBoxes && imgEl}
                <div
                    class="bbox-layer"
                    style="
                        left: {imgEl.offsetLeft}px;
                        top: {imgEl.offsetTop}px;
                        width: {imgEl.offsetWidth}px;
                        height: {imgEl.offsetHeight}px;
                        transform: scale({$loupeScale}) translate({$loupePanX / $loupeScale}px, {$loupePanY / $loupeScale}px);
                    "
                >
                    {#each [...detections, ...nsfwDetections] as det}
                        <div
                            class="bbox"
                            class:bbox-nsfw={det.class_name.includes('EXPOSED')}
                            style="
                                left: {det.x * 100}%;
                                top: {det.y * 100}%;
                                width: {det.width * 100}%;
                                height: {det.height * 100}%;
                            "
                        >
                            <span class="bbox-label">{det.class_name} {det.confidence.toFixed(2)}</span>
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    {:else}
        <div class="empty">No image selected</div>
    {/if}

    {#if cropMode}
        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
        <div
            class="crop-overlay"
            class:space-move={cropSpaceHeld && currentCropRect}
            onmousedown={handleCropMouseDown}
            onmousemove={handleCropMouseMove}
            onmouseup={handleCropMouseUp}
            onmouseleave={handleCropMouseUp}
        >
            {#if cropStart && cropEnd && imgEl && image}
                {@const cropSelection = getCropSelectionPercent()}
                {#if cropSelection}
                    <div
                        class="crop-selection-layer"
                        style="
                            left: {imgEl.offsetLeft}px;
                            top: {imgEl.offsetTop}px;
                            width: {imgEl.offsetWidth}px;
                            height: {imgEl.offsetHeight}px;
                            transform: scale({$loupeScale}) translate({$loupePanX / $loupeScale}px, {$loupePanY / $loupeScale}px);
                        "
                    >
                        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
                        <div
                            class="crop-selection"
                            onmousedown={handleCropSelectionMouseDown}
                            style="
                                left: {cropSelection.left}%;
                                top: {cropSelection.top}%;
                                width: {cropSelection.width}%;
                                height: {cropSelection.height}%;
                            "
                        >
                            {#each CROP_HANDLES as handle}
                                <button
                                    type="button"
                                    class="crop-handle crop-handle-{handle}"
                                    aria-label="Resize crop {handle}"
                                    title="Resize crop"
                                    onmousedown={(e) => handleCropHandleMouseDown(e, handle)}
                                ></button>
                            {/each}
                        </div>
                    </div>
                {/if}
            {/if}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
                class="crop-toolbar"
                onmousedown={(e) => e.stopPropagation()}
                onmouseup={(e) => e.stopPropagation()}
            >
                {#if cropSizeLabel}
                    <span class="crop-dimensions">{cropSizeLabel}</span>
                {/if}
                <button onclick={applyCrop} disabled={cropping || !canApplyCrop} title="Crop selection">
                    {cropping ? 'Cropping…' : '✓ Crop'}
                </button>
                <button onclick={cancelCrop} title="Cancel crop">✕ Cancel</button>
            </div>
        </div>
    {/if}

    {#if !hideOverlays && decision !== 'undecided'}
        <div class="mini-status" class:mini-accept={decision === 'accept'} class:mini-reject={decision === 'reject'}>
            {#if decision === 'accept'}✓{:else}×{/if}
        </div>
    {/if}

    {#if !hideOverlays && isSelected}
        <div class="mini-selected">SEL</div>
    {/if}

    {#if toastDecision}
        {#key toastKey}
        <div class="status-toast" class:toast-accept={toastDecision === 'accept'} class:toast-reject={toastDecision === 'reject'}>
            <span class="toast-icon">{toastDecision === 'accept' ? '✓' : '×'}</span>
            <span>{toastDecision === 'accept' ? 'Accepted' : 'Rejected'}</span>
        </div>
        {/key}
    {/if}

    {#if !hideOverlays}
    <div class="overlay-bar">
        <span class="filename">{filename}</span>
        <span class="sep">|</span>
        <span class="dim">{dimensions}</span>
        <span class="sep">|</span>
        <span class="fmt">{format}</span>
        {#if sourceDisplay}
            <span class="sep">|</span>
            <span class="source">{sourceDisplay}</span>
        {/if}
        {#if rating > 0}
            <span class="sep">|</span>
            <span class="rating">
                {#each Array(rating) as _}
                    <span class="star">&#9733;</span>
                {/each}
            </span>
        {/if}
        {#if decision !== 'undecided'}
            <span class="sep">|</span>
            <span class="decision" class:accept={decision === 'accept'} class:reject={decision === 'reject'}>
                {decision}
            </span>
        {/if}
        {#if $loupeScale !== 1}
            <span class="sep">|</span>
            <span class="zoom">{Math.round($loupeScale * 100)}%</span>
        {/if}
        {#if prompt}
            <span class="sep">|</span>
            <button class="prompt-toggle" onclick={() => promptExpanded = !promptExpanded}
                    title={promptExpanded ? 'Collapse prompt' : 'Show full prompt'}>
                ✦ {promptExpanded ? 'Hide prompt' : 'Prompt'}
            </button>
        {/if}
    </div>
    {#if prompt && promptExpanded}
        <div class="prompt-panel">
            <div class="prompt-header">
                <span class="prompt-label">PROMPT</span>
                <button class="prompt-copy" onclick={copyPrompt} title="Copy prompt">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13">
                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                        <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
                    </svg>
                    Copy
                </button>
                <button class="prompt-action" onclick={() => resubmitVisible = true} title="Re-generate with this prompt">
                    Re-generate
                </button>
            </div>
            <div class="prompt-text">{prompt}</div>
            {#if genModel || genProvider || genSeed}
                <div class="prompt-meta">
                    {#if genProvider}<span class="meta-tag">{genProvider}</span>{/if}
                    {#if genModel}<span class="meta-tag">{genModel}</span>{/if}
                    {#if genSeed}<span class="meta-tag">seed:{genSeed}</span>{/if}
                </div>
            {/if}
        </div>
    {/if}
    {/if}

    {#if ctxMenu.visible && image}
        <ContextMenu
            {image}
            x={ctxMenu.x}
            y={ctxMenu.y}
            onclose={() => ctxMenu.visible = false}
        />
    {/if}
</div>

    <PromptResubmitDialog
        visible={resubmitVisible}
        initialPrompt={prompt ?? ''}
        sourceImageId={image?.image.id ?? null}
        onclose={() => resubmitVisible = false}
        ongenerated={(ids, jobId) => {}}
    />

    <GenerationResultsStrip
        oncompare={(ids) => {}}
        onselect={(id) => {}}
    />

{#if $showDetectionInspector}
    <div class="inspector">
        <div class="inspector-header">DETECTIONS</div>
        {#if detections.length > 0}
            <div class="inspector-section">OBJECTS</div>
            {#each detections as det}
                <div class="inspector-row">
                    <span class="inspector-class">{det.class_name}</span>
                    <span class="inspector-conf">{det.confidence.toFixed(2)}</span>
                </div>
            {/each}
        {:else}
            <div class="inspector-empty">no objects</div>
        {/if}

        <div class="inspector-section">NSFW</div>
        {#if nsfwDetections.length > 0}
            {#each nsfwDetections as det}
                <div class="inspector-row">
                    <span class="inspector-class nsfw">{det.class_name}</span>
                    <span class="inspector-conf">{det.confidence.toFixed(2)}</span>
                </div>
            {/each}
        {:else}
            <div class="inspector-empty">none</div>
        {/if}

        {#if visionMeta.length > 0}
            <div class="inspector-section">VISION</div>
            {#each visionMeta as [key, value, _source]}
                <div class="inspector-meta-row">
                    <span class="meta-key">{key}</span>
                    <span class="meta-value">{value}</span>
                </div>
            {/each}
        {/if}
    </div>
{/if}
</div>

<style>
    .loupe-wrapper {
        grid-area: main;
        display: flex;
        overflow: hidden;
    }
    .loupe-wrapper.has-inspector .loupe-container {
        flex: 1;
    }
    .loupe-container {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        background: var(--bg);
        overflow: hidden;
        position: relative;
        cursor: default;
    }
    .loupe-container.zoomed {
        cursor: grab;
    }
    .loupe-container.dragging {
        cursor: grabbing;
    }
    .image-frame {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
        width: 100%;
        height: 100%;
    }
    img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        transform-origin: center center;
        user-select: none;
        -webkit-user-drag: none;
    }
    img.blurred {
        filter: blur(30px) brightness(0.5);
    }
    img.unblurring {
        transition: filter 0.2s;
    }
    img.pixel-zoom {
        image-rendering: pixelated;
    }
    .empty {
        color: var(--text-secondary);
        font-size: 14px;
    }
    .overlay-bar {
        position: absolute;
        bottom: 0;
        left: 0;
        right: 0;
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 6px 12px;
        background: rgba(8, 8, 12, 0.85);
        font-size: 11px;
    }
    .filename {
        color: var(--text);
    }
    .sep {
        color: var(--border);
    }
    .dim, .fmt {
        color: var(--text-secondary);
    }
    .source {
        color: var(--purple, #bb9af7);
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
        text-transform: uppercase;
        font-size: 10px;
        color: var(--text-secondary);
    }
    .decision.accept {
        color: var(--green);
    }
    .decision.reject {
        color: var(--red);
    }
    .zoom {
        color: var(--blue);
    }
    .prompt-toggle {
        background: none;
        border: none;
        color: rgba(255,255,255,0.7);
        cursor: pointer;
        font-size: 0.75rem;
        padding: 0;
        font-family: inherit;
    }
    .prompt-toggle:hover {
        color: #fff;
    }
    .prompt-panel {
        position: absolute;
        bottom: 36px;
        left: 0;
        right: 0;
        background: rgba(0,0,0,0.9);
        padding: 12px 16px;
        font-size: 13px;
        color: rgba(255,255,255,0.92);
        line-height: 1.6;
        max-height: 240px;
        overflow-y: auto;
        backdrop-filter: blur(12px);
        z-index: 10;
        border-top: 1px solid rgba(255,255,255,0.08);
    }
    .prompt-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 8px;
    }
    .prompt-label {
        font-size: 9px;
        font-weight: 700;
        letter-spacing: 0.1em;
        color: rgba(255,255,255,0.4);
    }
    .prompt-copy {
        display: inline-flex;
        align-items: center;
        gap: 4px;
        background: rgba(255,255,255,0.08);
        border: 1px solid rgba(255,255,255,0.15);
        border-radius: 4px;
        color: rgba(255,255,255,0.7);
        padding: 3px 8px;
        font-size: 10px;
        font-family: inherit;
        cursor: pointer;
        transition: background 0.15s, color 0.15s;
    }
    .prompt-copy:hover {
        background: rgba(255,255,255,0.15);
        color: #fff;
    }
    .prompt-text {
        white-space: pre-wrap;
        word-break: break-word;
        user-select: text;
    }
    .mini-selected {
        position: absolute;
        top: 18px;
        left: 18px;
        z-index: 20;
        display: grid;
        place-items: center;
        padding: 3px 8px;
        border-radius: 4px;
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.06em;
        pointer-events: none;
        background: var(--blue, #7aa2f7);
        color: var(--bg);
        box-shadow: 0 0 0 1px rgba(8, 8, 12, 0.8), 0 8px 22px rgba(0, 0, 0, 0.34);
    }
    /* Decision badge (persistent) */
    .mini-status {
        position: absolute;
        top: 18px;
        right: 18px;
        z-index: 20;
        display: grid;
        place-items: center;
        width: 32px;
        height: 28px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--text-secondary);
        font-size: 16px;
        font-weight: 700;
        line-height: 1;
        pointer-events: none;
        box-shadow: 0 0 0 1px var(--bg);
        opacity: 0.96;
    }
    .mini-status.mini-accept {
        border-color: var(--green);
        color: var(--green);
    }
    .mini-status.mini-reject {
        border-color: var(--red);
        color: var(--red);
    }
    /* Decision toast (transient) */
    .status-toast {
        position: absolute;
        left: 50%;
        bottom: 56px;
        z-index: 30;
        transform: translateX(-50%);
        display: inline-flex;
        align-items: center;
        gap: 10px;
        height: 42px;
        padding: 0 16px;
        background: rgba(8, 8, 12, 0.76);
        border: 1px solid color-mix(in srgb, currentColor 32%, transparent);
        border-radius: 999px;
        backdrop-filter: blur(16px);
        -webkit-backdrop-filter: blur(16px);
        font-size: 13px;
        letter-spacing: 0.04em;
        text-transform: uppercase;
        box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
        pointer-events: none;
        animation: status-pop 720ms ease-out forwards;
    }
    .toast-icon {
        display: grid;
        place-items: center;
        width: 24px;
        height: 24px;
        border-radius: 50%;
        font-size: 15px;
        font-weight: 700;
    }
    .toast-accept {
        color: var(--green);
    }
    .toast-accept .toast-icon {
        background: var(--green);
        color: var(--bg);
    }
    .toast-reject {
        color: var(--red);
    }
    .toast-reject .toast-icon {
        background: var(--red);
        color: var(--bg);
    }
    @keyframes status-pop {
        0% { opacity: 0; transform: translateX(-50%) translateY(8px) scale(0.96); }
        14% { opacity: 1; transform: translateX(-50%) translateY(0) scale(1); }
        78% { opacity: 1; transform: translateX(-50%) translateY(0) scale(1); }
        100% { opacity: 0; transform: translateX(-50%) translateY(-4px) scale(0.98); }
    }
    /* Bounding boxes */
    .bbox-layer {
        position: absolute;
        transform-origin: center center;
        pointer-events: none;
    }
    .bbox {
        position: absolute;
        border: 1px solid var(--green, #9ece6a);
        pointer-events: none;
    }
    .bbox-nsfw {
        border-color: var(--red, #f7768e);
    }
    .bbox-label {
        position: absolute;
        top: -16px;
        left: 0;
        font-size: 9px;
        padding: 1px 4px;
        background: rgba(8, 8, 12, 0.8);
        color: var(--green, #9ece6a);
        white-space: nowrap;
    }
    .bbox-nsfw .bbox-label {
        color: var(--red, #f7768e);
    }
    /* NSFW overlay */
    .nsfw-overlay {
        position: absolute;
        inset: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        pointer-events: none;
    }
    .nsfw-label {
        font-size: 14px;
        font-weight: 700;
        color: var(--red, #f7768e);
        letter-spacing: 0.1em;
    }
    .nsfw-hint {
        font-size: 10px;
        color: var(--text-secondary, #565f89);
        margin-top: 4px;
    }
    /* Inspector panel */
    .inspector {
        width: 180px;
        background: var(--surface, #0c0c12);
        border-left: 1px solid var(--border, #1a1a2e);
        padding: 8px;
        overflow-y: auto;
        font-size: 11px;
    }
    .inspector-header {
        font-size: 10px;
        font-weight: 700;
        color: var(--text-secondary, #565f89);
        letter-spacing: 0.1em;
        margin-bottom: 8px;
    }
    .inspector-section {
        font-size: 9px;
        font-weight: 700;
        color: var(--text-secondary, #565f89);
        letter-spacing: 0.08em;
        margin-top: 8px;
        margin-bottom: 4px;
    }
    .inspector-row {
        display: flex;
        justify-content: space-between;
        padding: 2px 0;
    }
    .inspector-class {
        color: var(--purple, #bb9af7);
    }
    .inspector-class.nsfw {
        color: var(--red, #f7768e);
    }
    .inspector-conf {
        color: var(--text-secondary, #565f89);
    }
    .inspector-empty {
        color: var(--text-secondary, #565f89);
        font-style: italic;
        font-size: 10px;
    }
    .inspector-meta-row {
        display: flex;
        flex-direction: column;
        padding: 1px 0;
        font-size: 10px;
    }
    .meta-key {
        color: var(--text-secondary, #565f89);
        font-size: 9px;
    }
    .meta-value {
        color: var(--text-primary, #e0e0e0);
        word-break: break-word;
    }
    .prompt-meta {
        display: flex;
        gap: 6px;
        margin-top: 6px;
        flex-wrap: wrap;
    }
    .meta-tag {
        background: var(--bg-elevated, #2a2a3e);
        color: var(--text-secondary, #888);
        padding: 1px 6px;
        border-radius: 3px;
        font-size: 10px;
        font-family: var(--font-mono);
    }
    /* Crop mode */
    .crop-overlay {
        position: absolute;
        inset: 0;
        cursor: crosshair;
        z-index: 20;
    }
    .crop-overlay.space-move {
        cursor: move;
    }
    .crop-selection-layer {
        position: absolute;
        transform-origin: center center;
        overflow: hidden;
        pointer-events: none;
    }
    .crop-selection {
        position: absolute;
        border: 2px dashed var(--blue);
        background: color-mix(in srgb, var(--blue) 8%, transparent);
        box-shadow: 0 0 0 9999px color-mix(in srgb, var(--bg) 58%, transparent);
        cursor: move;
        pointer-events: auto;
    }
    .crop-handle {
        position: absolute;
        width: 10px;
        height: 10px;
        padding: 0;
        border: 2px solid var(--bg);
        border-radius: 50%;
        background: var(--blue);
        box-shadow: 0 0 0 1px var(--border);
    }
    .crop-handle-nw { top: -6px; left: -6px; cursor: nwse-resize; }
    .crop-handle-n { top: -6px; left: 50%; transform: translateX(-50%); cursor: ns-resize; }
    .crop-handle-ne { top: -6px; right: -6px; cursor: nesw-resize; }
    .crop-handle-e { top: 50%; right: -6px; transform: translateY(-50%); cursor: ew-resize; }
    .crop-handle-se { right: -6px; bottom: -6px; cursor: nwse-resize; }
    .crop-handle-s { bottom: -6px; left: 50%; transform: translateX(-50%); cursor: ns-resize; }
    .crop-handle-sw { bottom: -6px; left: -6px; cursor: nesw-resize; }
    .crop-handle-w { top: 50%; left: -6px; transform: translateY(-50%); cursor: ew-resize; }
    .crop-handle:focus-visible {
        outline: 2px solid var(--text);
        outline-offset: 2px;
    }
    .crop-toolbar {
        position: absolute;
        bottom: 48px;
        left: 50%;
        transform: translateX(-50%);
        background: color-mix(in srgb, var(--bg) 88%, transparent);
        padding: 8px 16px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        display: flex;
        gap: 12px;
        align-items: center;
        backdrop-filter: blur(8px);
        z-index: 1;
    }
    .crop-dimensions {
        color: var(--blue);
        font-size: 11px;
        white-space: nowrap;
    }
    .crop-toolbar button {
        background: none;
        border: 1px solid color-mix(in srgb, var(--text) 30%, transparent);
        color: var(--text);
        padding: 4px 12px;
        border-radius: var(--radius);
        cursor: pointer;
        font-size: 0.8rem;
    }
    .crop-toolbar button:hover:not(:disabled) {
        background: color-mix(in srgb, var(--text) 10%, transparent);
    }
    .crop-toolbar button:disabled {
        opacity: 0.4;
    }
    .prompt-action {
        background: var(--blue);
        color: var(--bg);
        border: none;
        border-radius: var(--radius);
        font-family: var(--font);
        font-size: 11px;
        padding: 2px 8px;
        cursor: pointer;
        margin-left: auto;
    }
    .prompt-action:hover {
        filter: brightness(1.15);
    }
</style>
