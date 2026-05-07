<script lang="ts">
    import { onMount } from 'svelte';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { UMAP } from 'umap-js';
    import { images, focusedIndex, viewMode, zenMode } from '$lib/stores';
    import { openUrl } from '@tauri-apps/plugin-opener';
    import {
        isModelAvailable,
        downloadClipModel,
        generateEmbeddings,
        getAllEmbeddings,
        getEmbeddingCount,
        listImages,
        getApiKey,
        setApiKey,
        validateApiKey,
        generateGeminiEmbeddings,
    } from '$lib/api';
    import type { ImageWithFile } from '$lib/api';

    // State
    let modelAvailable = $state(false);
    let downloading = $state(false);
    let generating = $state(false);
    let genProgress = $state({ current: 0, total: 0 });
    let embeddingCount = $state(0);
    let totalImages = $state(0);

    // Provider config
    type Provider = 'clip' | 'gemini';
    let selectedProvider = $state<Provider>('clip');
    let configOpen = $state(false);
    let apiKey = $state('');
    let keyValid = $state<boolean | null>(null);
    let validating = $state(false);
    let geminiEmbeddingCount = $state(0);

    // Download progress
    let downloadProgress = $state({ downloaded: 0, total: 0, status: '' });
    let downloadStartTime = $state(0);
    let downloadSpeed = $state('');

    // UMAP projection
    type Point = { id: string; x: number; y: number; cluster: number };
    let points = $state<Point[]>([]);
    let clusters = $state<{ id: number; label: string; count: number; color: string; previewPaths: string[] }[]>([]);
    let hoveredPoint = $state<Point | null>(null);
    let selectedPoint = $state<Point | null>(null);
    let highlightedCluster = $state<number | null>(null);

    // Thumbnail images for scatter
    let thumbnailImages: Map<string, HTMLImageElement> = new Map();

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
        '#41a6b5', '#c3e88d', '#fc5d7c', '#89b4fa', '#f5c2e7',
        '#fab387', '#94e2d5', '#cba6f7',
    ];

    onMount(async () => {
        await checkModel();
        await loadEmbeddingState();
        await loadApiKeyState();
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

    async function loadApiKeyState() {
        try {
            const key = await getApiKey('google');
            if (key) {
                apiKey = key;
                keyValid = true; // assume valid if stored
                geminiEmbeddingCount = await getEmbeddingCount('gemini-embedding-2');
            }
        } catch (e) {
            console.error('Failed to load API key state:', e);
        }
    }

    async function handleSaveApiKey() {
        if (!apiKey.trim()) {
            keyValid = null;
            return;
        }
        validating = true;
        try {
            const valid = await validateApiKey('google', apiKey.trim());
            keyValid = valid;
            if (valid) {
                await setApiKey('google', apiKey.trim());
            }
        } catch (e) {
            keyValid = false;
            console.error('Validation failed:', e);
        } finally {
            validating = false;
        }
    }

    async function handleGenerateGemini() {
        generating = true;
        genProgress = { current: 0, total: 0 };

        const unlisten: UnlistenFn = await listen<{ current: number; total: number; provider: string }>(
            'embedding-progress',
            (event) => {
                genProgress = event.payload;
            }
        );

        try {
            const imageIds = $images.map(img => img.image.id);
            const count = await generateGeminiEmbeddings(imageIds);
            geminiEmbeddingCount = await getEmbeddingCount('gemini-embedding-2');
            if (geminiEmbeddingCount > 0) {
                await loadProjection();
            }
        } catch (e) {
            console.error('Gemini generate failed:', e);
        } finally {
            unlisten();
            generating = false;
        }
    }

    function openApiKeyPage() {
        openUrl('https://aistudio.google.com/apikey');
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

    function preloadThumbnails() {
        thumbnailImages.clear();
        for (const point of points) {
            const img = imageMap.get(point.id);
            if (!img?.thumbnail_path) continue;
            const el = new Image();
            el.src = convertFileSrc(img.thumbnail_path);
            el.onload = () => {
                thumbnailImages.set(point.id, el);
                requestDraw();
            };
        }
    }

    function nameCluster(clusterPoints: Point[]): string {
        const folderCounts: Map<string, number> = new Map();
        for (const p of clusterPoints) {
            const img = imageMap.get(p.id);
            if (!img) continue;
            const parts = img.path.split('/');
            const folder = parts.length >= 2 ? parts[parts.length - 2] : 'unknown';
            folderCounts.set(folder, (folderCounts.get(folder) || 0) + 1);
        }
        let best = 'cluster';
        let bestCount = 0;
        for (const [name, count] of folderCounts) {
            if (count > bestCount) { best = name; bestCount = count; }
        }
        return best;
    }

    function getClusterPreviewPaths(clusterPoints: Point[]): string[] {
        const paths: string[] = [];
        for (const p of clusterPoints) {
            if (paths.length >= 4) break;
            const img = imageMap.get(p.id);
            if (img?.thumbnail_path) {
                paths.push(img.thumbnail_path);
            }
        }
        return paths;
    }

    function focusCluster(clusterId: number) {
        highlightedCluster = highlightedCluster === clusterId ? null : clusterId;
        if (highlightedCluster !== null) {
            // Pan/zoom to fit the cluster
            const clusterPts = points.filter(p => p.cluster === clusterId);
            if (clusterPts.length > 0) {
                const xs = clusterPts.map(p => p.x);
                const ys = clusterPts.map(p => p.y);
                const minX = Math.min(...xs);
                const maxX = Math.max(...xs);
                const minY = Math.min(...ys);
                const maxY = Math.max(...ys);
                const rangeX = maxX - minX || 1;
                const rangeY = maxY - minY || 1;
                const padding = 120;
                const scaleX = (canvasWidth - padding * 2) / rangeX;
                const scaleY = (canvasHeight - padding * 2) / rangeY;
                scale = Math.min(scaleX, scaleY);
                panX = canvasWidth / 2 - ((minX + maxX) / 2) * scale;
                panY = canvasHeight / 2 - ((minY + maxY) / 2) * scale;
            }
        }
        requestDraw();
    }

    async function loadProjection() {
        try {
            const modelName = selectedProvider === 'gemini' ? 'gemini-embedding-2' : undefined;
            const embeddings = await getAllEmbeddings(modelName);
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

            // Run UMAP — tight clusters with clear separation
            const vectors = embeddings.map(([_, vec]) => vec);
            const nNeighbors = Math.min(15, Math.max(2, Math.floor(vectors.length / 5)));
            const umap = new UMAP({ nNeighbors, minDist: 0.05, spread: 1.5, nComponents: 2 });
            const projection = umap.fit(vectors);

            // More clusters for richer structure
            const k = Math.min(16, Math.max(3, Math.floor(Math.sqrt(projection.length))));
            const clusterLabels = kMeans(projection, k);

            // Build points
            const newPoints: Point[] = embeddings.map(([id, _], i) => ({
                id,
                x: projection[i][0],
                y: projection[i][1],
                cluster: clusterLabels[i],
            }));

            points = newPoints;

            // Build cluster info with named labels and preview thumbnails
            const clusterGroups = new Map<number, Point[]>();
            for (const p of newPoints) {
                if (!clusterGroups.has(p.cluster)) clusterGroups.set(p.cluster, []);
                clusterGroups.get(p.cluster)!.push(p);
            }
            clusters = Array.from(clusterGroups.entries())
                .sort((a, b) => b[1].length - a[1].length)
                .map(([idx, pts]) => ({
                    id: idx,
                    label: nameCluster(pts),
                    count: pts.length,
                    color: CLUSTER_COLORS[idx % CLUSTER_COLORS.length],
                    previewPaths: getClusterPreviewPaths(pts),
                }));

            // Auto-fit view
            highlightedCluster = null;
            fitView();
            preloadThumbnails();
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

        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        ctx.clearRect(0, 0, canvasWidth, canvasHeight);

        // Thumbnail size: small at overview, grows when zoomed in
        const pointDensityFactor = Math.max(1, Math.sqrt(points.length / 10));
        const baseThumbSize = Math.max(4, Math.min(48, (8 * Math.sqrt(scale)) / pointDensityFactor));
        const useThumb = baseThumbSize >= 8;

        // Draw points — clean thumbnails, no borders, no transparency
        const margin = baseThumbSize + 10;
        for (const p of points) {
            const [sx, sy] = worldToScreen(p.x, p.y);
            if (sx < -margin || sx > canvasWidth + margin || sy < -margin || sy > canvasHeight + margin) continue;

            const color = CLUSTER_COLORS[p.cluster % CLUSTER_COLORS.length];
            const isSelected = selectedPoint && selectedPoint.id === p.id;
            const isHovered = hoveredPoint && hoveredPoint.id === p.id;

            const thumbEl = useThumb ? thumbnailImages.get(p.id) : undefined;

            if (thumbEl && thumbEl.complete && thumbEl.naturalWidth > 0) {
                const thumbSize = baseThumbSize;
                const half = thumbSize / 2;

                ctx.drawImage(thumbEl, sx - half, sy - half, thumbSize, thumbSize);

                // Only highlight selected/hovered — thin white outline
                if (isSelected || isHovered) {
                    ctx.strokeStyle = '#ffffff';
                    ctx.lineWidth = isSelected ? 2 : 1;
                    ctx.strokeRect(sx - half, sy - half, thumbSize, thumbSize);
                }
            } else {
                // Colored dot fallback
                const radius = Math.max(2, Math.min(5, 4 / Math.sqrt(scale)));
                ctx.fillStyle = color;
                ctx.beginPath();
                ctx.arc(sx, sy, radius, 0, Math.PI * 2);
                ctx.fill();

                if (isSelected || isHovered) {
                    ctx.strokeStyle = '#ffffff';
                    ctx.lineWidth = 1;
                    ctx.stroke();
                }
            }
        }

        // Draw cluster labels at centroid when zoomed out enough
        if (highlightedCluster === null && scale < 5) {
            ctx.save();
            ctx.font = 'bold 11px JetBrains Mono, monospace';
            ctx.textAlign = 'center';
            for (const cluster of clusters) {
                const clusterPts = points.filter(p => p.cluster === cluster.id);
                if (clusterPts.length === 0) continue;
                const cx = clusterPts.reduce((s, p) => s + p.x, 0) / clusterPts.length;
                const cy = clusterPts.reduce((s, p) => s + p.y, 0) / clusterPts.length;
                const [sx, sy] = worldToScreen(cx, cy);
                // Background pill
                const text = cluster.label;
                const tw = ctx.measureText(text).width;
                ctx.fillStyle = 'rgba(0, 0, 0, 0.6)';
                ctx.beginPath();
                const pillX = sx - tw / 2 - 6;
                const pillY = sy - baseThumbSize / 2 - 22;
                const pillW = tw + 12;
                const pillH = 18;
                const r = 4;
                ctx.moveTo(pillX + r, pillY);
                ctx.lineTo(pillX + pillW - r, pillY);
                ctx.quadraticCurveTo(pillX + pillW, pillY, pillX + pillW, pillY + r);
                ctx.lineTo(pillX + pillW, pillY + pillH - r);
                ctx.quadraticCurveTo(pillX + pillW, pillY + pillH, pillX + pillW - r, pillY + pillH);
                ctx.lineTo(pillX + r, pillY + pillH);
                ctx.quadraticCurveTo(pillX, pillY + pillH, pillX, pillY + pillH - r);
                ctx.lineTo(pillX, pillY + r);
                ctx.quadraticCurveTo(pillX, pillY, pillX + r, pillY);
                ctx.fill();
                ctx.fillStyle = cluster.color;
                ctx.fillText(text, sx, pillY + 13);
            }
            ctx.restore();
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

        // Hit test - use thumbnail size for hit area when thumbnails are visible
        const rect = canvas.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const thumbSize = Math.max(8, Math.min(60, 20 * Math.sqrt(scale)));
        const hitHalf = Math.max(6, thumbSize / 2);

        let found: Point | null = null;
        for (const p of points) {
            const [sx, sy] = worldToScreen(p.x, p.y);
            if (Math.abs(mx - sx) < hitHalf && Math.abs(my - sy) < hitHalf) {
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
        const idx = $images.findIndex(img => img.image.id === point.id);
        if (idx >= 0) {
            focusedIndex.set(idx);
        }
        zoomToPoint(point);
    }

    function zoomToPoint(point: Point) {
        const targetScale = Math.max(scale * 2.5, 800);
        const targetPanX = canvasWidth / 2 - point.x * targetScale;
        const targetPanY = canvasHeight / 2 - point.y * targetScale;

        const startScale = scale;
        const startPanX = panX;
        const startPanY = panY;
        const duration = 300;
        const startTime = performance.now();

        function animate(now: number) {
            const t = Math.min((now - startTime) / duration, 1);
            const ease = 1 - (1 - t) * (1 - t);
            scale = startScale + (targetScale - startScale) * ease;
            panX = startPanX + (targetPanX - startPanX) * ease;
            panY = startPanY + (targetPanY - startPanY) * ease;
            requestDraw();
            if (t < 1) requestAnimationFrame(animate);
        }
        requestAnimationFrame(animate);
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

    function handleCanvasDblClick(e: MouseEvent) {
        if (!hoveredPoint) return;
        const idx = $images.findIndex(img => img.image.id === hoveredPoint!.id);
        if (idx >= 0) {
            focusedIndex.set(idx);
            viewMode.set('loupe');
        }
    }

    function handleResize(node: HTMLDivElement) {
        const observer = new ResizeObserver(entries => {
            for (const entry of entries) {
                canvasWidth = entry.contentRect.width;
                canvasHeight = entry.contentRect.height;
                if (canvas) {
                    const dpr = window.devicePixelRatio || 1;
                    canvas.width = canvasWidth * dpr;
                    canvas.height = canvasHeight * dpr;
                    canvas.style.width = canvasWidth + 'px';
                    canvas.style.height = canvasHeight + 'px';
                    const ctx = canvas.getContext('2d');
                    if (ctx) {
                        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
                    }
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

<div class="embedding-explorer" class:zen={$zenMode}>
    {#if !$zenMode}
    <div class="left-panel">
        <div class="panel-section">
            <div class="section-header-row">
                <div class="section-header">PROVIDER</div>
                <button class="gear-btn" onclick={() => configOpen = !configOpen} title="Settings">
                    &#9881;
                </button>
            </div>
            <select class="provider-select" bind:value={selectedProvider} onchange={() => loadProjection()}>
                <option value="clip">CLIP ViT-B/32 (local)</option>
                <option value="gemini">Gemini Embedding 2 (API)</option>
            </select>
            <div class="model-detail">
                {#if selectedProvider === 'clip'}
                    {#if !modelAvailable}
                        Model not downloaded
                    {:else}
                        {embeddingCount}/{$images.length} images
                    {/if}
                {:else}
                    {geminiEmbeddingCount}/{$images.length} images
                {/if}
            </div>
        </div>

        {#if configOpen}
            <div class="panel-section config-section">
                <div class="section-header">GEMINI API KEY</div>
                <div class="api-key-row">
                    <input
                        type="password"
                        placeholder="AIza..."
                        bind:value={apiKey}
                        class="api-input"
                        onblur={handleSaveApiKey}
                    />
                    <button class="link-btn" onclick={openApiKeyPage}>
                        Get Key &rarr;
                    </button>
                </div>
                <div class="key-status" class:valid={keyValid === true} class:invalid={keyValid === false}>
                    {#if validating}
                        Validating...
                    {:else if keyValid === true}
                        &#9679; Connected
                    {:else if keyValid === false}
                        &#9675; Invalid key
                    {:else}
                        &#9675; No key set
                    {/if}
                </div>
            </div>
        {/if}

        {#if selectedProvider === 'clip'}
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
        {:else}
            <div class="panel-section">
                <div class="stat-row">
                    <span class="stat-label">Images</span>
                    <span class="stat-value">{$images.length}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Embeddings</span>
                    <span class="stat-value">{geminiEmbeddingCount}</span>
                </div>
                <button class="action-btn" onclick={handleGenerateGemini} disabled={generating || keyValid !== true}>
                    {#if generating}
                        Generating {genProgress.current}/{genProgress.total}...
                    {:else if keyValid !== true}
                        Set API Key First
                    {:else if geminiEmbeddingCount < $images.length}
                        Generate Embeddings ({$images.length - geminiEmbeddingCount} remaining)
                    {:else}
                        Regenerate All
                    {/if}
                </button>
            </div>
        {/if}

        {#if clusters.length > 0}
            <div class="panel-section">
                <div class="section-header">CLUSTERS</div>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div class="cluster-item all" class:active={highlightedCluster === null} onclick={() => { highlightedCluster = null; fitView(); requestDraw(); }}>
                    <span class="cluster-dot" style="background: var(--text-secondary)"></span>
                    All Images
                    <span class="cluster-count">({points.length})</span>
                </div>
                {#each clusters as cluster}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div class="cluster-item" class:active={highlightedCluster === cluster.id} onclick={() => focusCluster(cluster.id)}>
                        <div class="cluster-info-row">
                            <span class="cluster-dot" style="background: {cluster.color}"></span>
                            <span class="cluster-name">{cluster.label}</span>
                            <span class="cluster-count">({cluster.count})</span>
                        </div>
                        {#if cluster.previewPaths.length > 0}
                            <div class="cluster-previews">
                                {#each cluster.previewPaths.slice(0, 4) as preview}
                                    <img src={convertFileSrc(preview)} class="cluster-thumb" alt="" />
                                {/each}
                            </div>
                        {/if}
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
    {/if}

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
            ondblclick={handleCanvasDblClick}
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
    .embedding-explorer.zen .right-panel {
        width: 100%;
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
        flex-direction: column;
        gap: 4px;
        font-size: 11px;
        padding: 5px 4px;
        color: var(--text);
        cursor: pointer;
        border-radius: var(--radius);
        transition: background 0.15s;
    }

    .cluster-item:hover {
        background: rgba(255, 255, 255, 0.05);
    }

    .cluster-item.active {
        background: rgba(122, 162, 247, 0.1);
    }

    .cluster-item.all {
        flex-direction: row;
        align-items: center;
        gap: 6px;
        margin-bottom: 4px;
        font-weight: 500;
    }

    .cluster-info-row {
        display: flex;
        align-items: center;
        gap: 6px;
    }

    .cluster-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        flex-shrink: 0;
    }

    .cluster-name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .cluster-count {
        color: var(--text-secondary);
        margin-left: auto;
        font-size: 10px;
        flex-shrink: 0;
    }

    .cluster-previews {
        display: flex;
        gap: 2px;
        margin-left: 14px;
    }

    .cluster-thumb {
        width: 20px;
        height: 20px;
        object-fit: cover;
        border-radius: 2px;
        opacity: 0.85;
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

    .section-header-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 8px;
    }

    .section-header-row .section-header {
        margin-bottom: 0;
    }

    .gear-btn {
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 14px;
        padding: 0 2px;
        line-height: 1;
        transition: color 0.15s;
    }

    .gear-btn:hover {
        color: var(--text);
    }

    .provider-select {
        width: 100%;
        background: var(--bg);
        color: var(--text);
        border: 1px solid var(--border);
        font-family: var(--font);
        font-size: 11px;
        padding: 4px 6px;
        border-radius: var(--radius);
        cursor: pointer;
    }

    .provider-select:focus {
        outline: none;
        border-color: var(--blue);
    }

    .config-section {
        background: rgba(0, 0, 0, 0.15);
    }

    .api-key-row {
        display: flex;
        gap: 6px;
        align-items: center;
    }

    .api-input {
        flex: 1;
        background: var(--bg);
        color: var(--text);
        border: 1px solid var(--border);
        font-family: var(--font);
        font-size: 11px;
        padding: 4px 6px;
        border-radius: var(--radius);
    }

    .api-input:focus {
        outline: none;
        border-color: var(--blue);
    }

    .api-input::placeholder {
        color: var(--text-secondary);
        opacity: 0.5;
    }

    .link-btn {
        background: none;
        border: none;
        color: var(--blue);
        font-family: var(--font);
        font-size: 10px;
        cursor: pointer;
        white-space: nowrap;
        padding: 0;
    }

    .link-btn:hover {
        text-decoration: underline;
    }

    .key-status {
        font-size: 10px;
        color: var(--text-secondary);
        margin-top: 4px;
    }

    .key-status.valid {
        color: #9ece6a;
    }

    .key-status.invalid {
        color: #f7768e;
    }
</style>
