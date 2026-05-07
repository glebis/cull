<script lang="ts">
    import { onMount } from 'svelte';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { UMAP } from 'umap-js';
    import { images, focusedIndex, viewMode } from '$lib/stores';
    import {
        isModelAvailable,
        downloadClipModel,
        generateEmbeddings,
        getAllEmbeddings,
        getEmbeddingCount,
        listImages,
    } from '$lib/api';
    import type { ImageWithFile } from '$lib/api';

    // State
    let modelAvailable = $state(false);
    let downloading = $state(false);
    let generating = $state(false);
    let genProgress = $state({ current: 0, total: 0 });
    let embeddingCount = $state(0);
    let totalImages = $state(0);

    // Download progress
    let downloadProgress = $state({ downloaded: 0, total: 0, status: '' });
    let downloadStartTime = $state(0);
    let downloadSpeed = $state('');

    // UMAP projection
    type Point = { id: string; x: number; y: number; cluster: number };
    let points = $state<Point[]>([]);
    let clusters = $state<{ label: string; count: number; color: string }[]>([]);
    let hoveredPoint = $state<Point | null>(null);
    let selectedPoint = $state<Point | null>(null);

    // Canvas interaction
    let canvas: HTMLCanvasElement;
    let canvasWidth = $state(800);
    let canvasHeight = $state(600);
    let panX = $state(0);
    let panY = $state(0);
    let scale = $state(1);
    let dragging = $state(false);
    let dragStartX = 0;
    let dragStartY = 0;
    let dragStartPanX = 0;
    let dragStartPanY = 0;

    // Image lookup for thumbnails
    let imageMap = $state<Map<string, ImageWithFile>>(new Map());

    const CLUSTER_COLORS = [
        '#7aa2f7', '#9ece6a', '#e0af68', '#bb9af7', '#f7768e',
        '#73daca', '#ff9e64', '#2ac3de', '#c0caf5', '#a9b1d6',
    ];

    onMount(async () => {
        await checkModel();
        await loadEmbeddingState();
    });

    async function checkModel() {
        try {
            modelAvailable = await isModelAvailable();
        } catch (e) {
            console.error('Failed to check model:', e);
        }
    }

    async function loadEmbeddingState() {
        try {
            embeddingCount = await getEmbeddingCount();
            totalImages = $images.length;
            if (embeddingCount > 0) {
                await loadProjection();
            }
        } catch (e) {
            console.error('Failed to load embedding state:', e);
        }
    }

    function formatBytes(bytes: number): string {
        if (bytes === 0) return '0 B';
        const mb = bytes / (1024 * 1024);
        if (mb >= 1) return `${mb.toFixed(0)} MB`;
        const kb = bytes / 1024;
        return `${kb.toFixed(0)} KB`;
    }

    async function handleDownload() {
        downloading = true;
        downloadProgress = { downloaded: 0, total: 0, status: 'downloading' };
        downloadStartTime = Date.now();
        downloadSpeed = '';

        const unlisten: UnlistenFn = await listen<{ downloaded: number; total: number; status: string }>(
            'model-download-progress',
            (event) => {
                downloadProgress = event.payload;
                // Calculate speed
                const elapsed = (Date.now() - downloadStartTime) / 1000;
                if (elapsed > 0 && event.payload.downloaded > 0) {
                    const bytesPerSec = event.payload.downloaded / elapsed;
                    downloadSpeed = `${formatBytes(bytesPerSec)}/s`;
                }
            }
        );

        try {
            await downloadClipModel();
            modelAvailable = true;
        } catch (e) {
            console.error('Download failed:', e);
        } finally {
            unlisten();
            downloading = false;
        }
    }

    async function handleGenerate() {
        generating = true;
        genProgress = { current: 0, total: 0 };

        const unlisten: UnlistenFn = await listen<{ current: number; total: number }>(
            'embedding-progress',
            (event) => {
                genProgress = event.payload;
            }
        );

        try {
            const imageIds = $images.map(img => img.image.id);
            const count = await generateEmbeddings(imageIds);
            embeddingCount = await getEmbeddingCount();
            if (embeddingCount > 0) {
                await loadProjection();
            }
        } catch (e) {
            console.error('Generate failed:', e);
        } finally {
            unlisten();
            generating = false;
        }
    }

    async function loadProjection() {
        try {
            const embeddings = await getAllEmbeddings();
            if (embeddings.length < 2) {
                points = [];
                return;
            }

            // Build image map for lookup
            const map = new Map<string, ImageWithFile>();
            for (const img of $images) {
                map.set(img.image.id, img);
            }
            imageMap = map;

            // Run UMAP
            const vectors = embeddings.map(([_, vec]) => vec);
            const nNeighbors = Math.min(15, Math.max(2, Math.floor(vectors.length / 3)));
            const umap = new UMAP({ nNeighbors, minDist: 0.1, nComponents: 2 });
            const projection = umap.fit(vectors);

            // Simple k-means clustering on 2D projection
            const k = Math.min(8, Math.max(2, Math.floor(Math.sqrt(projection.length / 2))));
            const clusterLabels = kMeans(projection, k);

            // Build points
            const newPoints: Point[] = embeddings.map(([id, _], i) => ({
                id,
                x: projection[i][0],
                y: projection[i][1],
                cluster: clusterLabels[i],
            }));

            points = newPoints;

            // Build cluster info
            const clusterCounts = new Map<number, number>();
            for (const p of newPoints) {
                clusterCounts.set(p.cluster, (clusterCounts.get(p.cluster) || 0) + 1);
            }
            clusters = Array.from(clusterCounts.entries())
                .sort((a, b) => b[1] - a[1])
                .map(([idx, count], i) => ({
                    label: `Cluster ${idx + 1}`,
                    count,
                    color: CLUSTER_COLORS[idx % CLUSTER_COLORS.length],
                }));

            // Auto-fit view
            fitView();
            requestDraw();
        } catch (e) {
            console.error('Failed to load projection:', e);
        }
    }

    function kMeans(data: number[][], k: number): number[] {
        if (data.length === 0) return [];

        // Initialize centroids randomly
        const indices = new Set<number>();
        while (indices.size < k && indices.size < data.length) {
            indices.add(Math.floor(Math.random() * data.length));
        }
        let centroids = Array.from(indices).map(i => [...data[i]]);
        let labels = new Array(data.length).fill(0);

        for (let iter = 0; iter < 20; iter++) {
            // Assign
            for (let i = 0; i < data.length; i++) {
                let minDist = Infinity;
                for (let c = 0; c < centroids.length; c++) {
                    const dx = data[i][0] - centroids[c][0];
                    const dy = data[i][1] - centroids[c][1];
                    const dist = dx * dx + dy * dy;
                    if (dist < minDist) {
                        minDist = dist;
                        labels[i] = c;
                    }
                }
            }

            // Update centroids
            const sums = centroids.map(() => [0, 0]);
            const counts = new Array(centroids.length).fill(0);
            for (let i = 0; i < data.length; i++) {
                sums[labels[i]][0] += data[i][0];
                sums[labels[i]][1] += data[i][1];
                counts[labels[i]]++;
            }
            for (let c = 0; c < centroids.length; c++) {
                if (counts[c] > 0) {
                    centroids[c] = [sums[c][0] / counts[c], sums[c][1] / counts[c]];
                }
            }
        }

        return labels;
    }

    function fitView() {
        if (points.length === 0) return;
        const xs = points.map(p => p.x);
        const ys = points.map(p => p.y);
        const minX = Math.min(...xs);
        const maxX = Math.max(...xs);
        const minY = Math.min(...ys);
        const maxY = Math.max(...ys);
        const rangeX = maxX - minX || 1;
        const rangeY = maxY - minY || 1;
        const padding = 60;
        const scaleX = (canvasWidth - padding * 2) / rangeX;
        const scaleY = (canvasHeight - padding * 2) / rangeY;
        scale = Math.min(scaleX, scaleY);
        panX = canvasWidth / 2 - ((minX + maxX) / 2) * scale;
        panY = canvasHeight / 2 - ((minY + maxY) / 2) * scale;
    }

    function screenToWorld(sx: number, sy: number): [number, number] {
        return [(sx - panX) / scale, (sy - panY) / scale];
    }

    function worldToScreen(wx: number, wy: number): [number, number] {
        return [wx * scale + panX, wy * scale + panY];
    }

    function requestDraw() {
        if (typeof requestAnimationFrame !== 'undefined') {
            requestAnimationFrame(draw);
        }
    }

    function draw() {
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        ctx.clearRect(0, 0, canvasWidth, canvasHeight);

        // Draw points
        const radius = Math.max(3, Math.min(8, 6 / Math.sqrt(scale)));
        for (const p of points) {
            const [sx, sy] = worldToScreen(p.x, p.y);
            if (sx < -20 || sx > canvasWidth + 20 || sy < -20 || sy > canvasHeight + 20) continue;

            ctx.beginPath();
            ctx.arc(sx, sy, radius, 0, Math.PI * 2);
            const color = CLUSTER_COLORS[p.cluster % CLUSTER_COLORS.length];

            if (selectedPoint && selectedPoint.id === p.id) {
                ctx.fillStyle = '#ffffff';
                ctx.strokeStyle = color;
                ctx.lineWidth = 2;
                ctx.fill();
                ctx.stroke();
            } else if (hoveredPoint && hoveredPoint.id === p.id) {
                ctx.fillStyle = color;
                ctx.globalAlpha = 1;
                ctx.fill();
                ctx.strokeStyle = '#ffffff';
                ctx.lineWidth = 1.5;
                ctx.stroke();
            } else {
                ctx.fillStyle = color;
                ctx.globalAlpha = 0.7;
                ctx.fill();
            }
            ctx.globalAlpha = 1;
        }

        // Draw hover tooltip
        if (hoveredPoint) {
            const [sx, sy] = worldToScreen(hoveredPoint.x, hoveredPoint.y);
            const img = imageMap.get(hoveredPoint.id);
            if (img) {
                const name = img.path.split('/').pop() || img.image.id;
                ctx.fillStyle = 'rgba(0, 0, 0, 0.8)';
                ctx.font = '11px JetBrains Mono, monospace';
                const textWidth = ctx.measureText(name).width;
                const tx = Math.min(sx + 12, canvasWidth - textWidth - 16);
                const ty = Math.max(sy - 8, 20);
                ctx.fillRect(tx - 4, ty - 12, textWidth + 8, 18);
                ctx.fillStyle = '#e0e0e0';
                ctx.fillText(name, tx, ty);
            }
        }
    }

    function handleWheel(e: WheelEvent) {
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;

        const factor = e.deltaY > 0 ? 0.9 : 1.1;
        const newScale = scale * factor;

        // Zoom toward cursor
        panX = mx - (mx - panX) * (newScale / scale);
        panY = my - (my - panY) * (newScale / scale);
        scale = newScale;

        requestDraw();
    }

    function handleMouseDown(e: MouseEvent) {
        dragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        dragStartPanX = panX;
        dragStartPanY = panY;
    }

    function handleMouseMove(e: MouseEvent) {
        if (dragging) {
            panX = dragStartPanX + (e.clientX - dragStartX);
            panY = dragStartPanY + (e.clientY - dragStartY);
            requestDraw();
            return;
        }

        // Hit test
        const rect = canvas.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const hitRadius = Math.max(6, 10 / Math.sqrt(scale));

        let found: Point | null = null;
        for (const p of points) {
            const [sx, sy] = worldToScreen(p.x, p.y);
            const dx = mx - sx;
            const dy = my - sy;
            if (dx * dx + dy * dy < hitRadius * hitRadius) {
                found = p;
                break;
            }
        }

        if (found !== hoveredPoint) {
            hoveredPoint = found;
            canvas.style.cursor = found ? 'pointer' : 'grab';
            requestDraw();
        }
    }

    function handleMouseUp(e: MouseEvent) {
        if (dragging) {
            const moved = Math.abs(e.clientX - dragStartX) + Math.abs(e.clientY - dragStartY);
            if (moved < 4 && hoveredPoint) {
                handlePointClick(hoveredPoint);
            }
        }
        dragging = false;
    }

    function handlePointClick(point: Point) {
        selectedPoint = point;
        // Find and focus in grid
        const idx = $images.findIndex(img => img.image.id === point.id);
        if (idx >= 0) {
            focusedIndex.set(idx);
        }
        requestDraw();
    }

    function handleFocusInGrid() {
        if (selectedPoint) {
            const idx = $images.findIndex(img => img.image.id === selectedPoint!.id);
            if (idx >= 0) {
                focusedIndex.set(idx);
                viewMode.set('loupe');
            }
        }
    }

    function handleResize(node: HTMLDivElement) {
        const observer = new ResizeObserver(entries => {
            for (const entry of entries) {
                canvasWidth = entry.contentRect.width;
                canvasHeight = entry.contentRect.height;
                if (canvas) {
                    canvas.width = canvasWidth;
                    canvas.height = canvasHeight;
                }
                if (points.length > 0) {
                    fitView();
                    requestDraw();
                }
            }
        });
        observer.observe(node);
        return {
            destroy() { observer.disconnect(); }
        };
    }
</script>

<div class="embedding-explorer">
    <div class="left-panel">
        <div class="panel-section">
            <div class="section-header">MODEL</div>
            <div class="model-info">CLIP ViT-B/32</div>
            <div class="model-detail">
                {#if !modelAvailable}
                    Model not downloaded
                {:else}
                    {embeddingCount}/{$images.length} images
                {/if}
            </div>
        </div>

        {#if !modelAvailable}
            <div class="panel-section">
                {#if downloading}
                    <div class="download-progress">
                        <div class="progress-text">
                            {#if downloadProgress.total > 0}
                                Downloading: {formatBytes(downloadProgress.downloaded)} / {formatBytes(downloadProgress.total)}
                                ({Math.round((downloadProgress.downloaded / downloadProgress.total) * 100)}%)
                            {:else}
                                Downloading...
                            {/if}
                        </div>
                        <div class="progress-bar-track">
                            <div
                                class="progress-bar-fill"
                                style="width: {downloadProgress.total > 0 ? (downloadProgress.downloaded / downloadProgress.total) * 100 : 0}%"
                            ></div>
                        </div>
                        {#if downloadSpeed}
                            <div class="progress-speed">{downloadSpeed}</div>
                        {/if}
                    </div>
                {:else}
                    <button class="action-btn" onclick={handleDownload}>
                        Download Model (~350MB)
                    </button>
                {/if}
                <div class="manual-download">
                    <div class="section-header" style="margin-top: 10px">MANUAL DOWNLOAD</div>
                    <pre class="manual-cmd">curl -L -o ~/.../models/clip-vit-b32-vision.onnx \
  https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx</pre>
                </div>
            </div>
        {:else}
            <div class="panel-section">
                <div class="stat-row">
                    <span class="stat-label">Images</span>
                    <span class="stat-value">{$images.length}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Embeddings</span>
                    <span class="stat-value">{embeddingCount}</span>
                </div>
                <button class="action-btn" onclick={handleGenerate} disabled={generating}>
                    {#if generating}
                        Generating {genProgress.current}/{genProgress.total}...
                    {:else if embeddingCount < $images.length}
                        Generate Embeddings ({$images.length - embeddingCount} remaining)
                    {:else}
                        Regenerate All
                    {/if}
                </button>
            </div>
        {/if}

        {#if clusters.length > 0}
            <div class="panel-section">
                <div class="section-header">CLUSTERS</div>
                <div class="cluster-item all">
                    <span class="cluster-dot" style="background: var(--text-secondary)"></span>
                    All Images
                    <span class="cluster-count">({points.length})</span>
                </div>
                {#each clusters as cluster}
                    <div class="cluster-item">
                        <span class="cluster-dot" style="background: {cluster.color}"></span>
                        {cluster.label}
                        <span class="cluster-count">({cluster.count})</span>
                    </div>
                {/each}
            </div>
        {/if}

        {#if selectedPoint}
            <div class="panel-section">
                <div class="section-header">SELECTED</div>
                {#if imageMap.get(selectedPoint.id)}
                    {@const img = imageMap.get(selectedPoint.id)!}
                    <div class="selected-preview">
                        <img
                            src={convertFileSrc(img.thumbnail_path || img.path)}
                            alt=""
                            class="preview-img"
                        />
                        <div class="preview-name">{img.path.split('/').pop()}</div>
                        <div class="preview-dims">{img.image.width} x {img.image.height}</div>
                    </div>
                    <button class="action-btn small" onclick={handleFocusInGrid}>
                        Open in Loupe
                    </button>
                {/if}
            </div>
        {/if}
    </div>

    <div class="right-panel" use:handleResize>
        {#if points.length === 0}
            <div class="empty-state">
                {#if !modelAvailable}
                    <div class="empty-icon">&#9881;</div>
                    <div class="empty-title">Model Required</div>
                    <div class="empty-text">Download the CLIP model to generate embeddings</div>
                {:else if embeddingCount === 0}
                    <div class="empty-icon">&#9673;</div>
                    <div class="empty-title">No Embeddings Yet</div>
                    <div class="empty-text">Generate embeddings for your images to visualize them in 2D space</div>
                {:else}
                    <div class="empty-icon">&#8987;</div>
                    <div class="empty-title">Loading...</div>
                {/if}
            </div>
        {/if}
        <canvas
            bind:this={canvas}
            width={canvasWidth}
            height={canvasHeight}
            onwheel={handleWheel}
            onmousedown={handleMouseDown}
            onmousemove={handleMouseMove}
            onmouseup={handleMouseUp}
            onmouseleave={() => { dragging = false; hoveredPoint = null; requestDraw(); }}
            class:hidden={points.length === 0}
            style="cursor: grab"
        ></canvas>
    </div>
</div>

<style>
    .embedding-explorer {
        grid-area: main;
        display: flex;
        height: 100%;
        overflow: hidden;
    }

    .left-panel {
        width: 240px;
        min-width: 240px;
        background: var(--surface);
        border-right: 1px solid var(--border);
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        gap: 0;
    }

    .panel-section {
        padding: 12px;
        border-bottom: 1px solid var(--border);
    }

    .section-header {
        font-size: 10px;
        font-weight: 700;
        color: var(--text-secondary);
        letter-spacing: 0.1em;
        margin-bottom: 8px;
    }

    .model-info {
        font-size: 12px;
        color: var(--blue);
        font-weight: 500;
    }

    .model-detail {
        font-size: 10px;
        color: var(--text-secondary);
        margin-top: 2px;
    }

    .stat-row {
        display: flex;
        justify-content: space-between;
        font-size: 11px;
        padding: 2px 0;
    }

    .stat-label {
        color: var(--text-secondary);
    }

    .stat-value {
        color: var(--text);
        font-weight: 500;
    }

    .action-btn {
        width: 100%;
        margin-top: 8px;
        background: rgba(122, 162, 247, 0.15);
        color: var(--blue);
        border: 1px solid var(--border);
        font-family: var(--font);
        font-size: 11px;
        padding: 6px 12px;
        border-radius: var(--radius);
        cursor: pointer;
        transition: all 0.15s;
    }

    .action-btn:hover:not(:disabled) {
        background: rgba(122, 162, 247, 0.25);
        border-color: var(--blue);
    }

    .action-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .action-btn.small {
        font-size: 10px;
        padding: 4px 8px;
    }

    .cluster-item {
        display: flex;
        align-items: center;
        gap: 6px;
        font-size: 11px;
        padding: 3px 0;
        color: var(--text);
    }

    .cluster-item.all {
        margin-bottom: 4px;
        font-weight: 500;
    }

    .cluster-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        flex-shrink: 0;
    }

    .cluster-count {
        color: var(--text-secondary);
        margin-left: auto;
        font-size: 10px;
    }

    .selected-preview {
        text-align: center;
    }

    .preview-img {
        max-width: 100%;
        max-height: 140px;
        border-radius: var(--radius);
        margin-bottom: 6px;
    }

    .preview-name {
        font-size: 10px;
        color: var(--text);
        word-break: break-all;
    }

    .preview-dims {
        font-size: 10px;
        color: var(--text-secondary);
    }

    .right-panel {
        flex: 1;
        position: relative;
        background: var(--bg);
        overflow: hidden;
    }

    canvas {
        display: block;
        width: 100%;
        height: 100%;
    }

    canvas.hidden {
        display: none;
    }

    .empty-state {
        position: absolute;
        inset: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
        color: var(--text-secondary);
    }

    .empty-icon {
        font-size: 32px;
        opacity: 0.5;
    }

    .empty-title {
        font-size: 14px;
        font-weight: 700;
    }

    .empty-text {
        font-size: 11px;
        opacity: 0.6;
        max-width: 280px;
        text-align: center;
    }

    .download-progress {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }

    .progress-text {
        font-size: 10px;
        color: var(--text);
    }

    .progress-bar-track {
        width: 100%;
        height: 6px;
        background: var(--border);
        border-radius: 3px;
        overflow: hidden;
    }

    .progress-bar-fill {
        height: 100%;
        background: var(--blue);
        border-radius: 3px;
        transition: width 0.2s ease;
    }

    .progress-speed {
        font-size: 9px;
        color: var(--text-secondary);
        text-align: right;
    }

    .manual-download {
        margin-top: 4px;
    }

    .manual-cmd {
        font-family: 'JetBrains Mono', monospace;
        font-size: 9px;
        color: var(--text-secondary);
        background: rgba(0, 0, 0, 0.3);
        padding: 6px;
        border-radius: var(--radius);
        overflow-x: auto;
        white-space: pre-wrap;
        word-break: break-all;
        line-height: 1.4;
        margin: 0;
    }
</style>
