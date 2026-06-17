<script lang="ts">
    import { onDestroy } from 'svelte';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import {
        activeCanvas,
        focusedImage,
        focusedIndex,
        images,
        navigateTo,
        requestTextInput,
        selectedIds,
        sessionCanvases,
        showToast,
        statusHint,
    } from '$lib/stores';
    import { updateCanvasLayout, type ImageWithFile } from '$lib/api';
    import {
        computeCanvasItemDragPosition,
        computeCanvasPanDrag,
        computeCanvasResize,
        computeCanvasZoomAtPoint,
        isCanvasSpacePanKey,
    } from '$lib/canvas-interactions';
    import { wheelGestureIntent } from '$lib/gesture-interactions';
    import { serializeCanvasDocumentLayout, type CanvasDocument } from '$lib/canvas-document';
    import {
        addCanvasItemAnnotation,
        applyCanvasViewItemCrop,
        canvasItemAnnotations,
        createCanvasDocumentFromLayoutJson,
        createCanvasViewItems,
        rotateCanvasViewItemClockwise,
        setCanvasViewItemCropFromPoints,
        updateCanvasDocumentFromViewItems,
        type CanvasViewItem,
    } from '$lib/canvas-view-model';
    import type { CanvasCrop } from '$lib/canvas-document';
    import { safeAssetPreviewPath } from '$lib/view-utils';
    import {
        computeVisibleCanvasItems,
        capCanvasItems,
        CANVAS_RENDER_CAP,
    } from '$lib/canvas-utils';
    import ContextMenu from './ContextMenu.svelte';

    type CropPoint = { x: number; y: number };
    type CropDraft = { itemId: string; anchor: CropPoint; current: CropPoint };

    const MIN_CROP_DRAG_SIZE = 0.02;

    let canvasItems = $state<CanvasViewItem[]>([]);
    let canvasDocument = $state<CanvasDocument | null>(null);
    let canvasEl: HTMLDivElement | undefined = $state();
    let panX = $state(0);
    let panY = $state(0);
    let zoom = $state(1);
    let panning = $state(false);
    let panStartX = $state(0);
    let panStartY = $state(0);
    let panOriginX = $state(0);
    let panOriginY = $state(0);
    let spacePanActive = $state(false);
    let suppressNextItemClick = $state(false);

    let dragItem = $state<string | null>(null);
    let dragOffsetX = $state(0);
    let dragOffsetY = $state(0);

    let resizeItem = $state<string | null>(null);
    let resizeStartX = $state(0);
    let resizeStartY = $state(0);
    let resizeStartW = $state(0);
    let resizeStartH = $state(0);
    let cropModeItemId = $state<string | null>(null);
    let cropDraft = $state<CropDraft | null>(null);
    let loadedCanvasKey = $state('');
    let saveTimer: ReturnType<typeof setTimeout> | null = null;

    // Viewport culling + render cap (P2): only mount items intersecting the viewport.
    const dpr = typeof window !== 'undefined' ? (window.devicePixelRatio || 1) : 1;
    let viewportWidth = $state(0);
    let viewportHeight = $state(0);

    $effect(() => {
        if (!canvasEl) return;
        const ro = new ResizeObserver((entries) => {
            for (const entry of entries) {
                viewportWidth = entry.contentRect.width;
                viewportHeight = entry.contentRect.height;
            }
        });
        ro.observe(canvasEl);
        return () => ro.disconnect();
    });

    // Cull to the viewport (with a generous margin) then cap the rendered node count.
    let visibleCanvas = $derived.by(() => {
        const culled = computeVisibleCanvasItems(
            canvasItems,
            { panX, panY, zoom, width: viewportWidth, height: viewportHeight },
            { margin: Math.max(viewportWidth, viewportHeight) },
        );
        return capCanvasItems(culled, CANVAS_RENDER_CAP);
    });

    onDestroy(() => {
        if (saveTimer) clearTimeout(saveTimer);
    });

    function handleResizeMouseDown(e: MouseEvent, item: CanvasViewItem) {
        e.stopPropagation();
        e.preventDefault();
        resizeItem = item.id;
        resizeStartX = e.clientX;
        resizeStartY = e.clientY;
        resizeStartW = item.width;
        resizeStartH = item.height;
    }

    $effect(() => {
        const imgs = $images;
        const canvas = $activeCanvas;
        const key = [
            canvas?.id ?? '__ephemeral__',
            canvas?.layout_json ?? '{}',
            imgs.map(i => i.image.id).join(','),
        ].join('|');
        if (key !== loadedCanvasKey) {
            loadedCanvasKey = key;
            loadCanvasState(canvas?.layout_json ?? '{}', imgs);
        }
    });

    function loadCanvasState(layoutJson: string, imgs: ImageWithFile[]) {
        try {
            canvasDocument = createCanvasDocumentFromLayoutJson(layoutJson, imgs);
        } catch (e) {
            showToast('Canvas layout invalid', { detail: String(e), type: 'error', duration: 10000 });
            canvasDocument = createCanvasDocumentFromLayoutJson('{}', imgs);
        }
        panX = canvasDocument.viewport.panX;
        panY = canvasDocument.viewport.panY;
        zoom = canvasDocument.viewport.zoom;
        canvasItems = createCanvasViewItems(canvasDocument, imgs);
    }

    function queueCanvasSave() {
        if (!$activeCanvas || !canvasDocument) return;
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
            saveTimer = null;
            persistCanvasLayout();
        }, 350);
    }

    async function persistCanvasLayout() {
        const canvas = $activeCanvas;
        if (!canvas || !canvasDocument) return;

        const updatedDocument = updateCanvasDocumentFromViewItems(canvasDocument, canvasItems, {
            panX,
            panY,
            zoom,
        });

        try {
            const layoutJson = serializeCanvasDocumentLayout(updatedDocument);
            canvasDocument = updatedDocument;
            await updateCanvasLayout(canvas.id, layoutJson);
            const updatedCanvas = { ...canvas, layout_json: layoutJson };
            activeCanvas.set(updatedCanvas);
            sessionCanvases.update(list => list.map(item => item.id === updatedCanvas.id ? updatedCanvas : item));
        } catch (e) {
            showToast('Canvas save failed', { detail: String(e), type: 'error', duration: 10000 });
        }
    }

    function handleCanvasMouseDown(e: MouseEvent) {
        canvasEl?.focus();
        if (e.button === 1 || (e.button === 0 && (e.altKey || spacePanActive))) {
            const target = e.target instanceof HTMLElement ? e.target : null;
            const startedOnItem = target?.closest('.canvas-item') !== null;
            panning = true;
            suppressNextItemClick = spacePanActive && e.button === 0 && startedOnItem;
            panStartX = e.clientX;
            panStartY = e.clientY;
            panOriginX = panX;
            panOriginY = panY;
            e.preventDefault();
        }
    }

    function handleCanvasMouseMove(e: MouseEvent) {
        if (cropDraft) {
            const item = canvasItems.find(it => it.id === cropDraft?.itemId);
            if (item) {
                cropDraft = {
                    ...cropDraft,
                    current: cropPointFromPointer(e, item),
                };
            }
        } else if (panning) {
            const nextPan = computeCanvasPanDrag(
                { panX: panOriginX, panY: panOriginY, zoom },
                { x: panStartX, y: panStartY },
                { x: e.clientX, y: e.clientY },
            );
            panX = nextPan.panX;
            panY = nextPan.panY;
        } else if (resizeItem) {
            const item = canvasItems.find(it => it.id === resizeItem);
            if (item) {
                const nextSize = computeCanvasResize({
                    startClientX: resizeStartX,
                    currentClientX: e.clientX,
                    startWidth: resizeStartW,
                    startHeight: resizeStartH,
                    imageWidth: item.image.image.width,
                    imageHeight: item.image.image.height,
                    zoom,
                });
                item.width = nextSize.width;
                item.height = nextSize.height;
                canvasItems = canvasItems;
            }
        } else if (dragItem) {
            const item = canvasItems.find(it => it.id === dragItem);
            if (item) {
                const nextPosition = computeCanvasItemDragPosition(
                    { x: e.clientX, y: e.clientY },
                    { panX, panY, zoom },
                    { x: dragOffsetX, y: dragOffsetY },
                );
                item.x = nextPosition.x;
                item.y = nextPosition.y;
                canvasItems = canvasItems;
            }
        }
    }

    function handleCanvasMouseUp() {
        if (cropDraft) {
            const draft = cropDraft;
            if (Math.abs(draft.current.x - draft.anchor.x) < MIN_CROP_DRAG_SIZE
                || Math.abs(draft.current.y - draft.anchor.y) < MIN_CROP_DRAG_SIZE) {
                cropDraft = null;
                return;
            }
            canvasItems = canvasItems.map(candidate =>
                candidate.id === draft.itemId
                    ? setCanvasViewItemCropFromPoints(candidate, draft.anchor, draft.current)
                    : candidate
            );
            cropDraft = null;
            cropModeItemId = null;
            queueCanvasSave();
            return;
        }

        const changed = panning || dragItem !== null || resizeItem !== null;
        panning = false;
        dragItem = null;
        resizeItem = null;
        if (changed) queueCanvasSave();
    }

    function handleItemMouseDown(e: MouseEvent, item: CanvasViewItem) {
        if (cropModeItemId === item.id) {
            if (e.button !== 0) return;
            e.stopPropagation();
            e.preventDefault();
            const point = cropPointFromPointer(e, item);
            cropDraft = { itemId: item.id, anchor: point, current: point };
            return;
        }
        if (spacePanActive) return;
        if (e.button !== 0 || e.altKey) return;
        e.stopPropagation();
        dragItem = item.id;
        dragOffsetX = (e.clientX - panX) / zoom - item.x;
        dragOffsetY = (e.clientY - panY) / zoom - item.y;
    }

    let ctxMenu = $state<{ visible: boolean; x: number; y: number; image: ImageWithFile | null }>({
        visible: false, x: 0, y: 0, image: null
    });

    function handleItemContextMenu(e: MouseEvent, item: CanvasViewItem) {
        e.preventDefault();
        e.stopPropagation();
        const idx = $images.findIndex(img => img.image.id === item.imageId);
        if (idx >= 0) focusedIndex.set(idx);
        ctxMenu = { visible: true, x: e.clientX, y: e.clientY, image: item.image };
    }

    function handleItemClick(e: MouseEvent, item: CanvasViewItem) {
        if (suppressNextItemClick) {
            suppressNextItemClick = false;
            return;
        }
        const idx = $images.findIndex(img => img.image.id === item.imageId);
        if (idx >= 0) focusedIndex.set(idx);
    }

    function rotateItemClockwise(item: CanvasViewItem) {
        canvasItems = canvasItems.map(candidate =>
            candidate.id === item.id ? rotateCanvasViewItemClockwise(candidate) : candidate
        );
        queueCanvasSave();
    }

    function beginItemCrop(item: CanvasViewItem) {
        cropModeItemId = cropModeItemId === item.id ? null : item.id;
        cropDraft = null;
        const idx = $images.findIndex(img => img.image.id === item.imageId);
        if (idx >= 0) focusedIndex.set(idx);
    }

    function resetItemCrop(item: CanvasViewItem) {
        canvasItems = canvasItems.map(candidate =>
            candidate.id === item.id ? applyCanvasViewItemCrop(candidate, null) : candidate
        );
        cropModeItemId = cropModeItemId === item.id ? null : cropModeItemId;
        cropDraft = cropDraft?.itemId === item.id ? null : cropDraft;
        queueCanvasSave();
    }

    async function addItemAnnotation(item: CanvasViewItem) {
        if (!canvasDocument) return;
        const body = await requestTextInput({
            title: 'Add Canvas Note',
            label: 'Note',
            description: item.image.path.split('/').pop() ?? 'Canvas item',
            placeholder: 'What should be remembered about this item?',
            confirmLabel: 'Add Note',
        });
        if (!body || !canvasDocument) return;

        try {
            canvasDocument = addCanvasItemAnnotation(canvasDocument, item.id, body, {
                x: 0.5,
                y: 0.5,
            });
            queueCanvasSave();
        } catch (e) {
            showToast('Canvas note failed', { detail: String(e), type: 'error', duration: 10000 });
        }
    }

    function cancelCropMode() {
        cropModeItemId = null;
        cropDraft = null;
    }

    function handleItemDblClick(item: CanvasViewItem) {
        const idx = $images.findIndex(img => img.image.id === item.imageId);
        if (idx >= 0) {
            focusedIndex.set(idx);
            navigateTo('loupe');
        }
    }

    function handleItemKeydown(e: KeyboardEvent, item: CanvasViewItem) {
        if (e.key === 'Enter') {
            e.preventDefault();
            handleItemDblClick(item);
        } else if (e.key === ' ') {
            e.preventDefault();
            handleItemClick(new MouseEvent('click'), item);
        } else if (e.key.toLowerCase() === 'r') {
            e.preventDefault();
            rotateItemClockwise(item);
        } else if (e.key.toLowerCase() === 'c') {
            e.preventDefault();
            beginItemCrop(item);
        } else if (e.key === 'Escape' && cropModeItemId) {
            e.preventDefault();
            cancelCropMode();
        }
    }

    function handleWheel(e: WheelEvent) {
        if (dragItem || resizeItem || cropDraft) return;
        const rect = canvasEl?.getBoundingClientRect();
        if (!rect) return;
        const intent = wheelGestureIntent({
            surface: 'canvas',
            deltaX: e.deltaX,
            deltaY: e.deltaY,
            deltaMode: e.deltaMode,
            clientX: e.clientX,
            clientY: e.clientY,
            ctrlKey: e.ctrlKey,
            metaKey: e.metaKey,
            altKey: e.altKey,
            shiftKey: e.shiftKey,
            viewportHeight: rect.height,
            target: e.target,
        });
        if (!intent) return;

        if (intent.type === 'pan') {
            e.preventDefault();
            panX -= intent.deltaX;
            panY -= intent.deltaY;
            queueCanvasSave();
            return;
        }

        if (intent.type === 'zoom') {
            e.preventDefault();
            const nextViewport = computeCanvasZoomAtPoint(
                { panX, panY, zoom },
                { x: e.clientX - rect.left, y: e.clientY - rect.top },
                intent.factor,
            );
            panX = nextViewport.panX;
            panY = nextViewport.panY;
            zoom = nextViewport.zoom;
            queueCanvasSave();
        }
    }

    function handleCanvasKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape' && cropModeItemId) {
            cancelCropMode();
            e.preventDefault();
            return;
        }
        if (!isCanvasSpacePanKey(keyInputFromEvent(e))) return;
        spacePanActive = true;
        e.preventDefault();
    }

    function handleCanvasKeyup(e: KeyboardEvent) {
        if (e.key !== ' ' && e.key !== 'Spacebar' && e.code !== 'Space') return;
        spacePanActive = false;
        e.preventDefault();
    }

    function handleCanvasBlur() {
        spacePanActive = false;
        if (panning) handleCanvasMouseUp();
    }

    function keyInputFromEvent(e: KeyboardEvent) {
        const target = e.target instanceof HTMLElement ? e.target : null;
        return {
            key: e.key,
            code: e.code,
            altKey: e.altKey,
            ctrlKey: e.ctrlKey,
            metaKey: e.metaKey,
            targetTagName: target?.tagName ?? null,
            isContentEditable: target?.isContentEditable ?? false,
        };
    }

    function cropPointFromPointer(e: MouseEvent, item: CanvasViewItem): CropPoint {
        const rect = canvasEl?.getBoundingClientRect();
        const localX = rect ? e.clientX - rect.left : e.clientX;
        const localY = rect ? e.clientY - rect.top : e.clientY;
        return {
            x: clamp(((localX - panX) / zoom - item.x) / item.width, 0, 1),
            y: clamp(((localY - panY) / zoom - item.y) / item.height, 0, 1),
        };
    }

    function cropImageStyle(item: CanvasViewItem) {
        const crop = item.crop ?? { x: 0, y: 0, width: 1, height: 1 };
        return [
            `left: ${cropImageOffset(crop.x, crop.width)}%`,
            `top: ${cropImageOffset(crop.y, crop.height)}%`,
            `width: ${100 / crop.width}%`,
            `height: ${100 / crop.height}%`,
        ].join('; ');
    }

    function cropImageOffset(origin: number, size: number) {
        return (-origin / size) * 100;
    }

    function cropSelectionForItem(item: CanvasViewItem): CanvasCrop | null {
        if (cropDraft?.itemId === item.id) {
            const x = Math.min(cropDraft.anchor.x, cropDraft.current.x);
            const y = Math.min(cropDraft.anchor.y, cropDraft.current.y);
            return {
                x,
                y,
                width: Math.abs(cropDraft.current.x - cropDraft.anchor.x),
                height: Math.abs(cropDraft.current.y - cropDraft.anchor.y),
            };
        }
        return cropModeItemId === item.id ? item.crop : null;
    }

    function cropRectStyle(crop: CanvasCrop) {
        return [
            `left: ${crop.x * 100}%`,
            `top: ${crop.y * 100}%`,
            `width: ${crop.width * 100}%`,
            `height: ${crop.height * 100}%`,
        ].join('; ');
    }

    function clamp(value: number, min: number, max: number) {
        return Math.max(min, Math.min(max, value));
    }

    function itemAnnotations(item: CanvasViewItem) {
        return canvasItemAnnotations(canvasDocument, item.id);
    }

    function annotationTitle(item: CanvasViewItem) {
        return itemAnnotations(item).map(annotation => annotation.body).join('\n');
    }

    $effect(() => {
        const count = canvasItems.length;
        statusHint.set(`Canvas — ${count} image${count !== 1 ? 's' : ''} | Zoom: ${Math.round(zoom * 100)}%`);
        return () => statusHint.set(null);
    });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
    class="canvas-viewport"
    bind:this={canvasEl}
    onmousedown={handleCanvasMouseDown}
    onmousemove={handleCanvasMouseMove}
    onmouseup={handleCanvasMouseUp}
    onmouseleave={handleCanvasMouseUp}
    onwheel={handleWheel}
    onkeydown={handleCanvasKeydown}
    onkeyup={handleCanvasKeyup}
    onblur={handleCanvasBlur}
    role="application"
    aria-label="Image canvas"
    tabindex="0"
    class:space-pan={spacePanActive}
    class:panning={panning}
>
    {#if visibleCanvas.droppedCount > 0}
        <div class="canvas-cap-banner" aria-live="polite">
            Showing {visibleCanvas.rendered.length} of {canvasItems.length} — zoom in to see more
        </div>
    {/if}
    <div class="canvas-layer" style="transform: translate({panX}px, {panY}px) scale({zoom});">
        {#each visibleCanvas.rendered as item (item.id)}
            {@const rating = item.image.selection?.star_rating ?? 0}
            {@const decision = item.image.selection?.decision ?? 'undecided'}
            {@const annotations = itemAnnotations(item)}
            {@const previewPath = safeAssetPreviewPath(item.image, { displayPx: Math.max(item.width, item.height) * zoom, dpr })}
            <div
                class="canvas-item"
                class:selected={$selectedIds.has(item.imageId)}
                class:focused={$focusedImage?.image.id === item.imageId}
                class:crop-active={cropModeItemId === item.id}
                style="left: {item.x}px; top: {item.y}px; width: {item.width}px; height: {item.height}px;"
                style:--item-rotation={`${item.rotationDegrees}deg`}
                onmousedown={(e) => handleItemMouseDown(e, item)}
                onclick={(e) => handleItemClick(e, item)}
                ondblclick={() => handleItemDblClick(item)}
                oncontextmenu={(e) => handleItemContextMenu(e, item)}
                role="button"
                aria-label={item.image.path.split('/').pop()}
                tabindex="0"
                onkeydown={(e) => handleItemKeydown(e, item)}
                data-agent-image-id={item.imageId}
                data-agent-filename={item.image.path.split('/').filter(Boolean).pop() ?? item.imageId}
                data-agent-path={item.image.path}
                data-agent-thumbnail-path={item.image.thumbnail_path ?? ''}
                data-agent-rating={rating || ''}
                data-agent-decision={decision}
                data-agent-selected={$selectedIds.has(item.imageId)}
                data-agent-focused={$focusedImage?.image.id === item.imageId}
                data-agent-view-role="canvas-item"
            >
                <div class="image-stage">
                    {#if previewPath}
                        <img
                            src={convertFileSrc(previewPath)}
                            alt=""
                            draggable="false"
                            style={cropImageStyle(item)}
                        />
                    {:else}
                        <div class="preview-unavailable">Preview unavailable</div>
                    {/if}
                </div>
                {#if cropModeItemId === item.id}
                    {@const cropSelection = cropSelectionForItem(item)}
                    <div class="crop-overlay" aria-hidden="true">
                        {#if cropSelection}
                            <div class="crop-selection" style={cropRectStyle(cropSelection)}></div>
                        {/if}
                    </div>
                {/if}
                <div class="item-tools">
                    <button
                        class="tool-btn crop-btn"
                        type="button"
                        title={cropModeItemId === item.id ? 'Cancel crop' : 'Crop item'}
                        aria-label={cropModeItemId === item.id ? 'Cancel crop' : 'Crop item'}
                        onclick={(e) => {
                            e.stopPropagation();
                            beginItemCrop(item);
                        }}
                        onmousedown={(e) => {
                            e.stopPropagation();
                            e.preventDefault();
                        }}
                    >{cropModeItemId === item.id ? '×' : 'C'}</button>
                    {#if item.crop}
                        <button
                            class="tool-btn reset-crop-btn"
                            type="button"
                            title="Reset crop"
                            aria-label="Reset crop"
                            onclick={(e) => {
                                e.stopPropagation();
                                resetItemCrop(item);
                            }}
                            onmousedown={(e) => {
                                e.stopPropagation();
                                e.preventDefault();
                            }}
                        >↺</button>
                    {/if}
                    <button
                        class="tool-btn note-btn"
                        type="button"
                        title="Add note"
                        aria-label="Add note"
                        onclick={(e) => {
                            e.stopPropagation();
                            addItemAnnotation(item);
                        }}
                        onmousedown={(e) => {
                            e.stopPropagation();
                            e.preventDefault();
                        }}
                    >✎</button>
                    <button
                        class="tool-btn rotate-btn"
                        type="button"
                        title="Rotate clockwise"
                        aria-label="Rotate clockwise"
                        onclick={(e) => {
                            e.stopPropagation();
                            rotateItemClockwise(item);
                        }}
                        onmousedown={(e) => {
                            e.stopPropagation();
                            e.preventDefault();
                        }}
                    >↻</button>
                </div>
                {#if decision !== 'undecided'}
                    <div class="badge decision-badge" class:accept={decision === 'accept'} class:reject={decision === 'reject'}>
                        {decision === 'accept' ? '✓' : '×'}
                    </div>
                {/if}
                {#if rating > 0}
                    <div class="badge rating-badge">{'★'.repeat(rating)}</div>
                {/if}
                {#if annotations.length > 0}
                    <div class="badge annotation-badge" title={annotationTitle(item)}>
                        {annotations.length}
                    </div>
                {/if}
                <div
                    class="resize-handle"
                    role="presentation"
                    onmousedown={(e) => handleResizeMouseDown(e, item)}
                ></div>
            </div>
        {/each}
    </div>

    {#if ctxMenu.visible && ctxMenu.image}
        <ContextMenu
            image={ctxMenu.image}
            x={ctxMenu.x}
            y={ctxMenu.y}
            onclose={() => ctxMenu = { visible: false, x: 0, y: 0, image: null }}
        />
    {/if}
</div>

<style>
    .canvas-cap-banner {
        position: absolute;
        top: 8px;
        left: 50%;
        transform: translateX(-50%);
        z-index: 10;
        padding: 4px 10px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--text-secondary);
        font-size: 11px;
        pointer-events: none;
    }
    .canvas-viewport {
        grid-area: main;
        overflow: hidden;
        background: var(--bg);
        cursor: default;
        position: relative;
        user-select: none;
        outline: none;
    }
    .canvas-viewport.space-pan {
        cursor: grab;
    }
    .canvas-viewport.panning,
    .canvas-viewport.space-pan:active {
        cursor: grabbing;
    }
    .canvas-viewport:focus-visible {
        box-shadow: inset 0 0 0 1px var(--blue);
    }
    .canvas-layer {
        position: absolute;
        top: 0;
        left: 0;
        transform-origin: 0 0;
    }
    .canvas-item {
        position: absolute;
        border: 2px solid transparent;
        border-radius: 2px;
        cursor: move;
        transition: border-color 0.1s;
        overflow: hidden;
    }
    .canvas-item.focused {
        border-color: var(--blue);
        box-shadow: 0 0 0 1px var(--blue);
    }
    .canvas-item.selected {
        border-color: var(--blue);
    }
    .canvas-item:hover {
        border-color: var(--text-secondary);
    }
    .canvas-item.crop-active {
        cursor: crosshair;
        border-color: var(--orange);
    }
    .image-stage {
        position: absolute;
        inset: 0;
        overflow: hidden;
        pointer-events: none;
        transform: rotate(var(--item-rotation, 0deg));
        transform-origin: center;
    }
    .canvas-item img {
        position: absolute;
        object-fit: cover;
        display: block;
    }
    .preview-unavailable {
        display: grid;
        place-items: center;
        width: 100%;
        height: 100%;
        color: var(--text-secondary);
        background: var(--surface);
        font-size: 11px;
        text-align: center;
    }
    .crop-overlay {
        position: absolute;
        inset: 0;
        pointer-events: none;
        background: rgba(8, 8, 12, 0.42);
        z-index: 2;
    }
    .crop-selection {
        position: absolute;
        border: 1px solid var(--orange);
        background: rgba(224, 175, 104, 0.16);
        box-shadow: 0 0 0 9999px rgba(8, 8, 12, 0.38);
    }
    .item-tools {
        position: absolute;
        bottom: 4px;
        left: 4px;
        display: flex;
        flex-wrap: wrap;
        gap: 4px;
        max-width: calc(100% - 8px);
        opacity: 0;
        transition: opacity 0.15s;
        z-index: 3;
    }
    .tool-btn {
        display: grid;
        place-items: center;
        height: 24px;
        padding: 0;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--blue);
        font-family: var(--font);
        font-size: 13px;
        line-height: 1;
        cursor: pointer;
        transition: border-color 0.15s, color 0.15s;
    }
    .crop-btn,
    .reset-crop-btn,
    .note-btn,
    .rotate-btn {
        width: 24px;
    }
    .tool-btn:hover,
    .tool-btn:focus-visible {
        border-color: var(--blue);
        outline: none;
    }
    .crop-btn {
        color: var(--orange);
    }
    .canvas-item:hover .item-tools,
    .canvas-item:focus-within .item-tools,
    .canvas-item.crop-active .item-tools {
        opacity: 1;
    }
    .resize-handle {
        position: absolute;
        bottom: -4px;
        right: -4px;
        width: 12px;
        height: 12px;
        background: var(--blue);
        border-radius: 2px;
        cursor: nwse-resize;
        opacity: 0;
        transition: opacity 0.15s;
        z-index: 3;
    }
    .canvas-item:hover .resize-handle {
        opacity: 1;
    }
    .badge {
        position: absolute;
        pointer-events: none;
    }
    .decision-badge {
        top: 0;
        left: 0;
        display: grid;
        place-items: center;
        width: 24px;
        height: 22px;
        padding: 0;
        border: 1px solid var(--border);
        border-top: 0;
        border-left: 0;
        border-radius: 0 0 var(--radius) 0;
        background: var(--surface);
        color: var(--text-secondary);
        box-shadow: 0 0 0 1px var(--bg);
        font-size: 12px;
        font-weight: 700;
        line-height: 1;
        opacity: 0.96;
    }
    .decision-badge.accept {
        border-right-color: var(--green);
        border-bottom-color: var(--green);
        color: var(--green);
    }
    .decision-badge.reject {
        border-right-color: var(--red);
        border-bottom-color: var(--red);
        color: var(--red);
    }
    .rating-badge {
        top: 4px;
        right: 4px;
        padding: 1px 4px;
        border-radius: 2px;
        color: var(--orange);
        background: var(--surface);
        font-size: 0.7rem;
        line-height: 1.2;
        letter-spacing: 0;
    }
    .annotation-badge {
        right: 4px;
        bottom: 4px;
        display: grid;
        place-items: center;
        min-width: 18px;
        height: 18px;
        padding: 0 4px;
        border: 1px solid var(--purple);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--purple);
        font-size: 10px;
        font-weight: 700;
        line-height: 1;
        z-index: 2;
    }
</style>
