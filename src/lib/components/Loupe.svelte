<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { onMount } from 'svelte';
    import ContextMenu from './ContextMenu.svelte';
    import { images, focusedIndex, focusedImage, statusHint, loupeScale, loupePanX, loupePanY, navigateBack, showDetectionBoxes, showDetectionInspector, nsfwMode, showToast } from '$lib/stores';
    import { getDetections, getVisionMetadata, cropImage, getImagesByIds } from '$lib/api';
    import type { Detection } from '$lib/api';

    let dragging = $state(false);
    let dragStartX = $state(0);
    let dragStartY = $state(0);
    let panStartX = $state(0);
    let panStartY = $state(0);

    let image = $derived($focusedImage);
    let src = $derived(image ? convertFileSrc(image.path) : '');
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
    let sourceDisplay = $derived(image?.source_label ? SOURCE_DISPLAY[image.source_label] ?? image.source_label : null);

    let prompt = $derived(image?.image.ai_prompt ?? null);
    let promptExpanded = $state(false);
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
    $effect(() => {
        const d = decision;
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
    });

    let shouldBlur = $derived(
        $nsfwMode === 'blur' && !spaceHeld && (!detectionsLoaded || isNsfw)
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
            loupePanX.set(0);
            loupePanY.set(0);
        }
    });

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
    let cropStart = $state<{x: number, y: number} | null>(null);
    let cropEnd = $state<{x: number, y: number} | null>(null);
    let cropping = $state(false);

    function enterCropMode() {
        cropMode = true;
        cropStart = null;
        cropEnd = null;
    }

    function cancelCrop() {
        cropMode = false;
        cropStart = null;
        cropEnd = null;
    }

    function getCropRect() {
        if (!cropStart || !cropEnd || !imgEl || !image) return null;
        const rect = imgEl.getBoundingClientRect();
        const scaleX = image.image.width / rect.width;
        const scaleY = image.image.height / rect.height;
        const x1 = Math.min(cropStart.x, cropEnd.x);
        const y1 = Math.min(cropStart.y, cropEnd.y);
        const x2 = Math.max(cropStart.x, cropEnd.x);
        const y2 = Math.max(cropStart.y, cropEnd.y);
        return {
            x: Math.round((x1 - rect.left) * scaleX),
            y: Math.round((y1 - rect.top) * scaleY),
            width: Math.round((x2 - x1) * scaleX),
            height: Math.round((y2 - y1) * scaleY),
        };
    }

    async function applyCrop() {
        const rect = getCropRect();
        if (!rect || !image || rect.width < 10 || rect.height < 10) return;
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
        cropStart = { x: e.clientX, y: e.clientY };
        cropEnd = { x: e.clientX, y: e.clientY };
    }

    function handleCropMouseMove(e: MouseEvent) {
        if (!cropMode || !cropStart) return;
        cropEnd = { x: e.clientX, y: e.clientY };
    }

    function handleCropMouseUp() {
        // Selection complete — user clicks Apply or Cancel
    }

    $effect(() => {
        if (!cropMode) return;
        function handleEsc(e: KeyboardEvent) {
            if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                cancelCrop();
            }
        }
        window.addEventListener('keydown', handleEsc, true);
        return () => window.removeEventListener('keydown', handleEsc, true);
    });

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
                {#each detections as det}
                    <div
                        class="bbox"
                        style="
                            left: {det.x * 100}%;
                            top: {det.y * 100}%;
                            width: {det.width * 100}%;
                            height: {det.height * 100}%;
                            transform: scale({$loupeScale}) translate({$loupePanX / $loupeScale}px, {$loupePanY / $loupeScale}px);
                        "
                    >
                        <span class="bbox-label">{det.class_name} {det.confidence.toFixed(2)}</span>
                    </div>
                {/each}
                {#each nsfwDetections as det}
                    <div
                        class="bbox bbox-nsfw"
                        style="
                            left: {det.x * 100}%;
                            top: {det.y * 100}%;
                            width: {det.width * 100}%;
                            height: {det.height * 100}%;
                            transform: scale({$loupeScale}) translate({$loupePanX / $loupeScale}px, {$loupePanY / $loupeScale}px);
                        "
                    >
                        <span class="bbox-label">{det.class_name} {det.confidence.toFixed(2)}</span>
                    </div>
                {/each}
            {/if}
        </div>
    {:else}
        <div class="empty">No image selected</div>
    {/if}

    {#if cropMode}
        <div
            class="crop-overlay"
            onmousedown={handleCropMouseDown}
            onmousemove={handleCropMouseMove}
            onmouseup={handleCropMouseUp}
        >
            {#if cropStart && cropEnd}
                {@const left = Math.min(cropStart.x, cropEnd.x)}
                {@const top = Math.min(cropStart.y, cropEnd.y)}
                {@const w = Math.abs(cropEnd.x - cropStart.x)}
                {@const h = Math.abs(cropEnd.y - cropStart.y)}
                <div class="crop-selection" style="left:{left}px;top:{top}px;width:{w}px;height:{h}px;"></div>
            {/if}
            <div class="crop-toolbar" onmousedown={(e) => e.stopPropagation()}>
                <button onclick={applyCrop} disabled={cropping || !cropStart}>
                    {cropping ? 'Cropping…' : '✓ Apply'}
                </button>
                <button onclick={cancelCrop}>✕ Cancel</button>
                <span class="crop-hint">Draw a rectangle to crop • Esc to cancel</span>
            </div>
        </div>
    {/if}

    {#if !hideOverlays && decision !== 'undecided'}
        <div class="mini-status" class:mini-accept={decision === 'accept'} class:mini-reject={decision === 'reject'}>
            {#if decision === 'accept'}✓{:else}×{/if}
        </div>
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
            <div class="prompt-text">{prompt}</div>
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
        background: rgba(0,0,0,0.85);
        padding: 12px 16px;
        font-size: 0.8rem;
        color: rgba(255,255,255,0.9);
        line-height: 1.5;
        max-height: 200px;
        overflow-y: auto;
        backdrop-filter: blur(8px);
        z-index: 10;
    }
    .prompt-text {
        white-space: pre-wrap;
        word-break: break-word;
        user-select: text;
    }
    /* Decision badge (persistent) */
    .mini-status {
        position: absolute;
        top: 18px;
        right: 18px;
        z-index: 20;
        display: grid;
        place-items: center;
        width: 30px;
        height: 30px;
        border-radius: 50%;
        font-size: 17px;
        font-weight: 800;
        pointer-events: none;
        box-shadow: 0 0 0 1px rgba(8, 8, 12, 0.8), 0 8px 22px rgba(0, 0, 0, 0.34);
    }
    .mini-status.mini-accept {
        background: var(--green);
        color: var(--bg);
    }
    .mini-status.mini-reject {
        background: var(--red);
        color: var(--bg);
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
    .bbox {
        position: absolute;
        border: 1px solid var(--green, #9ece6a);
        transform-origin: center center;
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
    /* Crop mode */
    .crop-overlay {
        position: absolute;
        inset: 0;
        cursor: crosshair;
        z-index: 20;
    }
    .crop-selection {
        position: fixed;
        border: 2px dashed #4a9eff;
        background: rgba(74, 158, 255, 0.1);
        pointer-events: none;
    }
    .crop-toolbar {
        position: absolute;
        bottom: 48px;
        left: 50%;
        transform: translateX(-50%);
        background: rgba(0,0,0,0.85);
        padding: 8px 16px;
        border-radius: 8px;
        display: flex;
        gap: 12px;
        align-items: center;
        backdrop-filter: blur(8px);
    }
    .crop-toolbar button {
        background: none;
        border: 1px solid rgba(255,255,255,0.3);
        color: #fff;
        padding: 4px 12px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 0.8rem;
    }
    .crop-toolbar button:hover:not(:disabled) {
        background: rgba(255,255,255,0.1);
    }
    .crop-toolbar button:disabled {
        opacity: 0.4;
    }
    .crop-hint {
        color: rgba(255,255,255,0.5);
        font-size: 0.75rem;
    }
</style>
