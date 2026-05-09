<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { revealItemInDir } from '@tauri-apps/plugin-opener';
    import { onMount } from 'svelte';
    import { images, focusedIndex, statusHint, loupeScale, loupePanX, loupePanY, navigateBack, showDetectionBoxes, showDetectionInspector, nsfwMode } from '$lib/stores';
    import { getDetections, getVisionMetadata } from '$lib/api';
    import type { Detection } from '$lib/api';

    let dragging = $state(false);
    let dragStartX = $state(0);
    let dragStartY = $state(0);
    let panStartX = $state(0);
    let panStartY = $state(0);

    let image = $derived($images[$focusedIndex] ?? null);
    let src = $derived(image ? convertFileSrc(image.path) : '');
    let filename = $derived(image?.path.split('/').pop() ?? '');
    let dimensions = $derived(image ? `${image.image.width}x${image.image.height}` : '');
    let format = $derived(image?.image.format ?? '');
    let rating = $derived(image?.selection?.star_rating ?? 0);
    let decision = $derived(image?.selection?.decision ?? 'undecided');

    let detections = $state<Detection[]>([]);
    let nsfwDetections = $state<Detection[]>([]);
    let isNsfw = $derived(nsfwDetections.length > 0);
    let spaceHeld = $state(false);
    let imgEl: HTMLImageElement | undefined = $state();
    let visionMeta = $state<[string, string, string][]>([]);
    let hideOverlays = $state(false);

    onMount(() => {
        function toggleOverlays() { hideOverlays = !hideOverlays; }
        window.addEventListener('toggle-loupe-overlays', toggleOverlays);
        return () => window.removeEventListener('toggle-loupe-overlays', toggleOverlays);
    });

    $effect(() => {
        const id = image?.image.id;
        if (!id) { detections = []; nsfwDetections = []; return; }
        getDetections(id).then(dets => {
            detections = dets.filter(d => !d.class_name.includes('EXPOSED') && !d.class_name.includes('COVERED') && !d.class_name.includes('FACE_') && !d.class_name.includes('BELLY') && !d.class_name.includes('FEET') && !d.class_name.includes('ARMPITS') && !d.class_name.includes('ANUS') && !d.class_name.includes('BUTTOCKS') && !d.class_name.includes('BREAST') && !d.class_name.includes('GENITALIA'));
            nsfwDetections = dets.filter(d => d.class_name.includes('EXPOSED'));
        }).catch(() => { detections = []; nsfwDetections = []; });
        getVisionMetadata(id).then(m => { visionMeta = m; }).catch(() => { visionMeta = []; });
    });

    let shouldBlur = $derived(isNsfw && $nsfwMode === 'blur' && !spaceHeld);

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

    let contextMenuVisible = $state(false);
    let contextMenuX = $state(0);
    let contextMenuY = $state(0);

    function handleContextMenu(e: MouseEvent) {
        if (!image) return;
        e.preventDefault();
        contextMenuX = e.clientX;
        contextMenuY = e.clientY;
        contextMenuVisible = true;

        function closeMenu() {
            contextMenuVisible = false;
            window.removeEventListener('click', closeMenu);
            window.removeEventListener('contextmenu', closeMenu);
        }
        setTimeout(() => {
            window.addEventListener('click', closeMenu);
            window.addEventListener('contextmenu', closeMenu);
        });
    }

    function revealInFinder() {
        contextMenuVisible = false;
        if (image) revealItemInDir(image.path);
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

    {#if !hideOverlays}
    <div class="overlay-bar">
        <span class="filename">{filename}</span>
        <span class="sep">|</span>
        <span class="dim">{dimensions}</span>
        <span class="sep">|</span>
        <span class="fmt">{format}</span>
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
    </div>
    {/if}

    {#if contextMenuVisible}
        <div
            class="context-menu"
            style="left: {contextMenuX}px; top: {contextMenuY}px;"
            role="menu"
        >
            <button class="context-menu-item" onclick={revealInFinder} role="menuitem">
                Reveal in Finder
            </button>
        </div>
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
        transition: filter 0.2s;
    }
    img.blurred {
        filter: blur(30px) brightness(0.5);
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
    .context-menu {
        position: fixed;
        z-index: 9999;
        background: var(--surface, #2a2a2e);
        border: 1px solid var(--border, #444);
        border-radius: 4px;
        padding: 4px 0;
        min-width: 160px;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    }
    .context-menu-item {
        display: block;
        width: 100%;
        padding: 6px 12px;
        background: none;
        border: none;
        color: var(--text, #eee);
        font-size: 12px;
        text-align: left;
        cursor: pointer;
    }
    .context-menu-item:hover {
        background: var(--blue, #3b82f6);
        color: #fff;
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
</style>
