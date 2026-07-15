<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import type { ImageWithFile } from '$lib/api';
    import { shouldDecodeGridOverviewThumbnails } from '$lib/grid-overview';
    import { safeAssetPreviewPath } from '$lib/view-utils';

    interface Props {
        items: ImageWithFile[];
        width: number;
        height: number;
        scrollTop: number;
        size: number;
        gap: number;
        cols: number;
        focusedIndex: number;
        selectedIds: Set<string>;
        onclick: (event: MouseEvent) => void;
        ondblclick: (event: MouseEvent) => void;
    }

    let {
        items,
        width,
        height,
        scrollTop,
        size,
        gap,
        cols,
        focusedIndex,
        selectedIds,
        onclick,
        ondblclick,
    }: Props = $props();

    let canvas: HTMLCanvasElement;
    let renderGeneration = 0;

    function token(styles: CSSStyleDeclaration, name: string): string {
        return styles.getPropertyValue(name).trim();
    }

    function imageColor(index: number, palette: string[]): string {
        const id = items[index]?.image.id ?? String(index);
        let hash = 0;
        for (let i = 0; i < id.length; i += 1) hash = ((hash << 5) - hash + id.charCodeAt(i)) | 0;
        return palette[Math.abs(hash) % palette.length];
    }

    $effect(() => {
        if (!canvas || width <= 0 || height <= 0 || cols <= 0 || size <= 0) return;
        const generation = ++renderGeneration;
        const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
        canvas.width = Math.max(1, Math.round(width * dpr));
        canvas.height = Math.max(1, Math.round(height * dpr));
        const ctx = canvas.getContext('2d', { alpha: false });
        if (!ctx) return;
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.imageSmoothingEnabled = size >= 8;

        const styles = getComputedStyle(canvas);
        const background = token(styles, '--bg');
        const palette = [
            token(styles, '--surface'),
            token(styles, '--border'),
            token(styles, '--text-secondary'),
            token(styles, '--blue'),
            token(styles, '--purple'),
        ];
        const selectedColor = token(styles, '--green');
        const focusedColor = token(styles, '--blue');
        ctx.fillStyle = background;
        ctx.fillRect(0, 0, width, height);

        const cellSize = size + gap;
        const firstRow = Math.max(0, Math.floor(scrollTop / cellSize));
        const lastRow = Math.min(Math.ceil(items.length / cols), Math.ceil((scrollTop + height) / cellSize) + 1);
        const pending: Array<{ index: number; x: number; y: number; src: string }> = [];
        const decodeThumbnails = shouldDecodeGridOverviewThumbnails(size);

        for (let row = firstRow; row < lastRow; row += 1) {
            for (let col = 0; col < cols; col += 1) {
                const index = row * cols + col;
                const item = items[index];
                if (!item) break;
                const x = col * cellSize;
                const y = row * cellSize - scrollTop;
                ctx.fillStyle = selectedIds.has(item.image.id)
                    ? selectedColor
                    : index === focusedIndex
                        ? focusedColor
                        : imageColor(index, palette);
                ctx.fillRect(x, y, size, size);

                if (decodeThumbnails) {
                    const path = safeAssetPreviewPath(item, { displayPx: size, dpr: 1 });
                    if (path) pending.push({ index, x, y, src: convertFileSrc(path) });
                }
            }
        }

        // Decode progressively with a small concurrency window. The canvas itself is the
        // cache, so even a six-figure overview never mounts six figures of DOM nodes.
        let cursor = 0;
        const workers = Math.min(12, pending.length);
        const drawNext = () => {
            if (generation !== renderGeneration) return;
            const next = pending[cursor++];
            if (!next) return;
            const img = new Image();
            img.decoding = 'async';
            img.onload = () => {
                if (generation === renderGeneration) {
                    ctx.drawImage(img, next.x, next.y, size, size);
                    const item = items[next.index];
                    if (item && selectedIds.has(item.image.id)) {
                        ctx.strokeStyle = selectedColor;
                        ctx.lineWidth = Math.max(1, Math.min(2, size / 3));
                        ctx.strokeRect(next.x + 0.5, next.y + 0.5, Math.max(0, size - 1), Math.max(0, size - 1));
                    }
                }
                drawNext();
            };
            img.onerror = drawNext;
            img.src = next.src;
        };
        for (let i = 0; i < workers; i += 1) drawNext();

        return () => {
            renderGeneration += 1;
        };
    });
</script>

<canvas
    bind:this={canvas}
    class="overview-canvas"
    style="top: {scrollTop}px; width: {width}px; height: {height}px;"
    aria-label="Canvas overview of {items.length} images"
    tabindex="0"
    {onclick}
    {ondblclick}
></canvas>

<style>
    .overview-canvas {
        position: absolute;
        left: 0;
        display: block;
        cursor: crosshair;
        outline: none;
        touch-action: pan-y;
    }

    .overview-canvas:focus-visible {
        box-shadow: inset 0 0 0 1px var(--blue);
    }
</style>
