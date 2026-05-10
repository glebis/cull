<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { images, focusedIndex, focusedImage, selectedIds, statusHint, navigateTo } from '$lib/stores';
    import type { ImageWithFile } from '$lib/api';
    import ContextMenu from './ContextMenu.svelte';

    interface CanvasItem {
        id: string;
        image: ImageWithFile;
        x: number;
        y: number;
        width: number;
        height: number;
    }

    let canvasItems = $state<CanvasItem[]>([]);
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

    function handleResizeMouseDown(e: MouseEvent, item: CanvasItem) {
        e.stopPropagation();
        e.preventDefault();
        resizeItem = item.id;
        resizeStartX = e.clientX;
        resizeStartY = e.clientY;
        resizeStartW = item.width;
        resizeStartH = item.height;
    }

    const ITEM_GAP = 20;
    const ITEM_HEIGHT = 200;

    let prevImageIds = $state('');

    $effect(() => {
        const imgs = $images;
        const currentIds = imgs.map(i => i.image.id).join(',');
        if (currentIds !== prevImageIds && imgs.length > 0) {
            prevImageIds = currentIds;
            layoutGrid(imgs);
        }
    });

    function layoutGrid(imgs: ImageWithFile[]) {
        const cols = Math.ceil(Math.sqrt(imgs.length));
        const colWidths = new Array(cols).fill(0);
        for (let i = 0; i < Math.min(cols, imgs.length); i++) {
            const aspect = imgs[i].image.width / imgs[i].image.height;
            colWidths[i] = ITEM_HEIGHT * aspect;
        }
        let colX = [0];
        for (let c = 1; c < cols; c++) {
            colX[c] = colX[c - 1] + (colWidths[c - 1] || ITEM_HEIGHT) + ITEM_GAP;
        }
        canvasItems = imgs.map((img, i) => {
            const aspect = img.image.width / img.image.height;
            const w = ITEM_HEIGHT * aspect;
            const col = i % cols;
            const row = Math.floor(i / cols);
            return {
                id: img.image.id,
                image: img,
                x: colX[col],
                y: row * (ITEM_HEIGHT + ITEM_GAP),
                width: w,
                height: ITEM_HEIGHT,
            };
        });
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
        panning = false;
        dragItem = null;
        resizeItem = null;
    }

    function handleItemMouseDown(e: MouseEvent, item: CanvasItem) {
        if (e.button !== 0 || e.altKey) return;
        e.stopPropagation();
        dragItem = item.id;
        dragOffsetX = (e.clientX - panX) / zoom - item.x;
        dragOffsetY = (e.clientY - panY) / zoom - item.y;
    }

    let ctxMenu = $state<{ visible: boolean; x: number; y: number; image: ImageWithFile | null }>({
        visible: false, x: 0, y: 0, image: null
    });

    function handleItemContextMenu(e: MouseEvent, item: CanvasItem) {
        e.preventDefault();
        e.stopPropagation();
        const idx = $images.findIndex(img => img.image.id === item.id);
        if (idx >= 0) focusedIndex.set(idx);
        ctxMenu = { visible: true, x: e.clientX, y: e.clientY, image: item.image };
    }

    function handleItemClick(e: MouseEvent, item: CanvasItem) {
        const idx = $images.findIndex(img => img.image.id === item.id);
        if (idx >= 0) focusedIndex.set(idx);
    }

    function handleItemDblClick(item: CanvasItem) {
        const idx = $images.findIndex(img => img.image.id === item.id);
        if (idx >= 0) {
            focusedIndex.set(idx);
            navigateTo('loupe');
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
    }

    $effect(() => {
        const count = canvasItems.length;
        statusHint.set(`Canvas — ${count} image${count !== 1 ? 's' : ''} | Zoom: ${Math.round(zoom * 100)}%`);
        return () => statusHint.set(null);
    });
</script>

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
>
    <div class="canvas-layer" style="transform: translate({panX}px, {panY}px) scale({zoom});">
        {#each canvasItems as item (item.id)}
            {@const rating = item.image.selection?.star_rating ?? 0}
            {@const decision = item.image.selection?.decision ?? 'undecided'}
            <div
                class="canvas-item"
                class:selected={$selectedIds.has(item.id)}
                class:focused={$focusedImage?.image.id === item.id}
                style="left: {item.x}px; top: {item.y}px; width: {item.width}px; height: {item.height}px;"
                onmousedown={(e) => handleItemMouseDown(e, item)}
                onclick={(e) => handleItemClick(e, item)}
                ondblclick={() => handleItemDblClick(item)}
                oncontextmenu={(e) => handleItemContextMenu(e, item)}
                role="img"
                aria-label={item.image.path.split('/').pop()}
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
