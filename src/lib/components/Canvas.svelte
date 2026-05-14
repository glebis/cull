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
    import { serializeCanvasDocumentLayout, type CanvasDocument } from '$lib/canvas-document';
    import {
        createCanvasDocumentFromLayoutJson,
        createCanvasViewItems,
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
        if (e.button === 1 || (e.button === 0 && e.altKey)) {
            panning = true;
            panStartX = e.clientX;
            panStartY = e.clientY;
            panOriginX = panX;
            panOriginY = panY;
            e.preventDefault();
        }
    }

    function handleCanvasMouseMove(e: MouseEvent) {
        if (panning) {
            panX = panOriginX + (e.clientX - panStartX);
            panY = panOriginY + (e.clientY - panStartY);
        } else if (resizeItem) {
            const item = canvasItems.find(it => it.id === resizeItem);
            if (item) {
                const dx = (e.clientX - resizeStartX) / zoom;
                const aspect = item.image.image.width / item.image.image.height;
                item.width = Math.max(50, resizeStartW + dx);
                item.height = item.width / aspect;
                canvasItems = canvasItems;
            }
        } else if (dragItem) {
            const item = canvasItems.find(it => it.id === dragItem);
            if (item) {
                item.x = (e.clientX - panX) / zoom - dragOffsetX;
                item.y = (e.clientY - panY) / zoom - dragOffsetY;
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
        const idx = $images.findIndex(img => img.image.id === item.imageId);
        if (idx >= 0) focusedIndex.set(idx);
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
        }
    }

    function handleWheel(e: WheelEvent) {
        e.preventDefault();
        const factor = e.deltaY > 0 ? 0.9 : 1.1;
        const rect = canvasEl?.getBoundingClientRect();
        if (!rect) return;
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const newZoom = Math.max(0.1, Math.min(5, zoom * factor));
        panX = mx - (mx - panX) * (newZoom / zoom);
        panY = my - (my - panY) * (newZoom / zoom);
        zoom = newZoom;
        queueCanvasSave();
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
    role="application"
    aria-label="Image canvas"
    tabindex="0"
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
        background: #111;
        cursor: grab;
        position: relative;
        user-select: none;
    }
    .canvas-viewport:active {
        cursor: grabbing;
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
    }
    .canvas-item.focused {
        border-color: #4a9eff;
        box-shadow: 0 0 0 1px #4a9eff;
    }
    .canvas-item.selected {
        border-color: #4a9eff;
    }
    .canvas-item:hover {
        border-color: rgba(255,255,255,0.4);
    }
    .canvas-item img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        pointer-events: none;
        display: block;
    }
    .resize-handle {
        position: absolute;
        bottom: -4px;
        right: -4px;
        width: 12px;
        height: 12px;
        background: #4a9eff;
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
        font-size: 0.65rem;
        padding: 1px 4px;
        border-radius: 2px;
        pointer-events: none;
        line-height: 1.2;
    }
    .decision-badge {
        top: 4px;
        left: 4px;
        font-size: 0.8rem;
        font-weight: bold;
    }
    .decision-badge.accept {
        background: rgba(34, 197, 94, 0.85);
        color: #fff;
    }
    .decision-badge.reject {
        background: rgba(239, 68, 68, 0.85);
        color: #fff;
    }
    .rating-badge {
        top: 4px;
        right: 4px;
        color: #fbbf24;
        background: rgba(0,0,0,0.6);
        font-size: 0.7rem;
        letter-spacing: -1px;
    }
</style>
