<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { images, focusedIndex, selectedIds, statusHint, navigateBack } from '$lib/stores';
    import type { ImageWithFile } from '$lib/api';

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

    const ITEM_GAP = 20;
    const ITEM_HEIGHT = 200;

    $effect(() => {
        const imgs = $images;
        if (canvasItems.length === 0 && imgs.length > 0) {
            layoutGrid(imgs);
        }
    });

    function layoutGrid(imgs: ImageWithFile[]) {
        const cols = Math.ceil(Math.sqrt(imgs.length));
        canvasItems = imgs.map((img, i) => {
            const aspect = img.image.width / img.image.height;
            const w = ITEM_HEIGHT * aspect;
            const col = i % cols;
            const row = Math.floor(i / cols);
            return {
                id: img.image.id,
                image: img,
                x: col * (w + ITEM_GAP),
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
    }

    function handleItemMouseDown(e: MouseEvent, item: CanvasItem) {
        if (e.button !== 0 || e.altKey) return;
        e.stopPropagation();
        dragItem = item.id;
        dragOffsetX = (e.clientX - panX) / zoom - item.x;
        dragOffsetY = (e.clientY - panY) / zoom - item.y;
    }

    function handleItemDblClick(item: CanvasItem) {
        const idx = $images.findIndex(img => img.image.id === item.id);
        if (idx >= 0) {
            focusedIndex.set(idx);
            navigateBack();
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
            <div
                class="canvas-item"
                class:selected={$selectedIds.has(item.id)}
                style="left: {item.x}px; top: {item.y}px; width: {item.width}px; height: {item.height}px;"
                onmousedown={(e) => handleItemMouseDown(e, item)}
                ondblclick={() => handleItemDblClick(item)}
                role="img"
                aria-label={item.image.path.split('/').pop()}
            >
                <img
                    src={item.image.thumbnail_path ? convertFileSrc(item.image.thumbnail_path) : convertFileSrc(item.image.path)}
                    alt=""
                    draggable="false"
                />
            </div>
        {/each}
    </div>
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
</style>
