<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import {
        activeCanvas,
        focusedImage,
        focusedIndex,
        images,
        navigateTo,
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
        computeCanvasWheelZoom,
        isCanvasSpacePanKey,
    } from '$lib/canvas-interactions';
    import { serializeCanvasDocumentLayout, type CanvasDocument } from '$lib/canvas-document';
    import {
        createCanvasDocumentFromLayoutJson,
        createCanvasViewItems,
        rotateCanvasViewItemClockwise,
        updateCanvasDocumentFromViewItems,
        type CanvasViewItem,
    } from '$lib/canvas-view-model';
    import ContextMenu from './ContextMenu.svelte';

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
    let loadedCanvasKey = $state('');
    let saveTimer: ReturnType<typeof setTimeout> | null = null;

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
        if (panning) {
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
        const changed = panning || dragItem !== null || resizeItem !== null;
        panning = false;
        dragItem = null;
        resizeItem = null;
        if (changed) queueCanvasSave();
    }

    function handleItemMouseDown(e: MouseEvent, item: CanvasViewItem) {
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
        }
    }

    function handleWheel(e: WheelEvent) {
        e.preventDefault();
        const rect = canvasEl?.getBoundingClientRect();
        if (!rect) return;
        const nextViewport = computeCanvasWheelZoom(
            { panX, panY, zoom },
            { x: e.clientX - rect.left, y: e.clientY - rect.top },
            e.deltaY,
        );
        panX = nextViewport.panX;
        panY = nextViewport.panY;
        zoom = nextViewport.zoom;
        queueCanvasSave();
    }

    function handleCanvasKeydown(e: KeyboardEvent) {
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
    <div class="canvas-layer" style="transform: translate({panX}px, {panY}px) scale({zoom});">
        {#each canvasItems as item (item.id)}
            {@const rating = item.image.selection?.star_rating ?? 0}
            {@const decision = item.image.selection?.decision ?? 'undecided'}
            <div
                class="canvas-item"
                class:selected={$selectedIds.has(item.imageId)}
                class:focused={$focusedImage?.image.id === item.imageId}
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
            >
                <img
                    src={item.image.thumbnail_path ? convertFileSrc(item.image.thumbnail_path) : convertFileSrc(item.image.path)}
                    alt=""
                    draggable="false"
                />
                <button
                    class="rotate-btn"
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
                {#if decision !== 'undecided'}
                    <div class="badge decision-badge" class:accept={decision === 'accept'} class:reject={decision === 'reject'}>
                        {decision === 'accept' ? '✓' : '×'}
                    </div>
                {/if}
                {#if rating > 0}
                    <div class="badge rating-badge">{'★'.repeat(rating)}</div>
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
    .canvas-item img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        pointer-events: none;
        display: block;
        transform: rotate(var(--item-rotation, 0deg));
        transform-origin: center;
    }
    .rotate-btn {
        position: absolute;
        bottom: 4px;
        left: 4px;
        display: grid;
        place-items: center;
        width: 24px;
        height: 24px;
        padding: 0;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--blue);
        font-family: var(--font);
        font-size: 13px;
        line-height: 1;
        opacity: 0;
        cursor: pointer;
        transition: opacity 0.15s, border-color 0.15s;
    }
    .rotate-btn:hover,
    .rotate-btn:focus-visible {
        border-color: var(--blue);
        outline: none;
    }
    .canvas-item:hover .rotate-btn,
    .canvas-item:focus-within .rotate-btn {
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
        letter-spacing: -1px;
    }
</style>
