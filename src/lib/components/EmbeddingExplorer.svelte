<script lang="ts">
    import { onMount } from 'svelte';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { images, focusedIndex, focusedImageOverride, viewMode, zenMode, navigateTo, embeddingViewState, settingsOpen } from '$lib/stores';
    import { get } from 'svelte/store';
    import { computeScatterThumbSize } from '$lib/embedding-utils';
    import {
        isModelAvailable,
        downloadClipModel,
        generateEmbeddings,
        getAllEmbeddings,
        getEmbeddingCount,
        getImageCount,
        listImageIds,
        hasApiKey,
        generateGeminiEmbeddings,
        getImagesByIds,
        regenerateThumbnails,
    } from '$lib/api';
    import type { ImageWithFile } from '$lib/api';

    // State
    let modelAvailable = $state(false);
    let downloading = $state(false);
    let generating = $state(false);
    let genProgress = $state({ current: 0, total: 0 });
    let embeddingCount = $state(0);
    let totalImages = $state(0);
    let regeneratingThumbs = $state(false);
    let staleEmbeddingCount = $state(0);

    // Provider config
    type Provider = 'clip' | 'gemini';
    let selectedProvider = $state<Provider>('clip');
    let configOpen = $state(false);
    let hasGoogleKey = $state(false);
    let geminiEmbeddingCount = $state(0);
    let currentEmbeddingCount = $derived(selectedProvider === 'gemini' ? geminiEmbeddingCount : embeddingCount);

    // Download progress
    let downloadProgress = $state({ downloaded: 0, total: 0, status: '' });
    let downloadStartTime = $state(0);
    let downloadSpeed = $state('');

    // UMAP projection
    type Point = { id: string; x: number; y: number; cluster: number };
    let points = $state<Point[]>([]);
    let clusters = $state<{ id: number; label: string; count: number; color: string; previewPaths: string[]; x: number; y: number }[]>([]);
    let hoveredPoint = $state<Point | null>(null);
    let selectedPoint = $state<Point | null>(null);
    let highlightedCluster = $state<number | null>(null);

    // Thumbnail images for scatter — keyed by "{id}_{size}"
    let thumbnailImages: Map<string, HTMLImageElement> = new Map();
    let loadingThumbnailKeys: Set<string> = new Set();
    let failedThumbnailIds = $state<Set<string>>(new Set());
    const THUMB_SIZES = [64, 128, 256, 800];
    const MAX_THUMBNAIL_CACHE = 600;

    // Canvas interaction
    let canvas: HTMLCanvasElement;
    let canvasWidth = $state(800);
    let canvasHeight = $state(600);
    let panX = $state(0);
    let panY = $state(0);
    let scale = $state(1);
    let dragging = $state(false);
    let drawQueued = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let dragStartPanX = 0;
    let dragStartPanY = 0;

    // Projection identity for view state validation
    let projectionKey = $state<string | null>(null);

    // Image lookup for thumbnails
    let imageMap = $state<Map<string, ImageWithFile>>(new Map());
    let projecting = $state(false);
    let projectionWorker: Worker | null = null;
    let cancelProjectionWork: (() => void) | null = null;
    let projectionRequestId = 0;
    let projectionLoadSeq = 0;

    const CLUSTER_COLORS = [
        '#7aa2f7', '#9ece6a', '#e0af68', '#bb9af7', '#f7768e',
        '#73daca', '#ff9e64', '#2ac3de', '#c0caf5', '#a9b1d6',
        '#41a6b5', '#c3e88d', '#fc5d7c', '#89b4fa', '#f5c2e7',
        '#fab387', '#94e2d5', '#cba6f7',
    ];

    onMount(() => {
        void (async () => {
            const savedState = get(embeddingViewState);
            selectedProvider = savedState.provider;
            await checkModel();
            await loadApiKeyState();
            await loadEmbeddingState();
        })();

        return () => {
            resetProjectionWorker();
        };
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
            totalImages = await getImageCount();
            embeddingCount = await getEmbeddingCount();
            if (hasGoogleKey) {
                geminiEmbeddingCount = await getEmbeddingCount('gemini-embedding-2');
            }
            if (currentEmbeddingCount > 0) {
                await loadProjection();
            } else {
                points = [];
                clusters = [];
                resetThumbnailCache();
            }
        } catch (e) {
            console.error('Failed to load embedding state:', e);
        }
    }

    async function loadApiKeyState() {
        try {
            hasGoogleKey = await hasApiKey('google');
            if (hasGoogleKey) {
                geminiEmbeddingCount = await getEmbeddingCount('gemini-embedding-2');
            }
        } catch (e) {
            console.error('Failed to load API key state:', e);
        }
    }

    let prevSettingsOpen = false;
    $effect(() => {
        const isOpen = $settingsOpen;
        if (prevSettingsOpen && !isOpen) {
            loadApiKeyState();
        }
        prevSettingsOpen = isOpen;
    });

    async function handleProviderChange() {
        resetProjectionWorker();
        points = [];
        clusters = [];
        selectedPoint = null;
        highlightedCluster = null;
        resetThumbnailCache();
        await loadEmbeddingState();
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
            const imageIds = await listImageIds();
            totalImages = imageIds.length;
            await generateGeminiEmbeddings(imageIds);
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
            const imageIds = await listImageIds();
            totalImages = imageIds.length;
            await generateEmbeddings(imageIds);
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

    async function handleRegenerateThumbnails() {
        regeneratingThumbs = true;
        try {
            await regenerateThumbnails();
            await loadProjection();
        } catch (e) {
            console.error('Thumbnail regeneration failed:', e);
        } finally {
            regeneratingThumbs = false;
        }
    }

    function sizedThumbPath(basePath: string, size: number): string {
        if (size === 800) return basePath;
        return basePath.replace(/\.jpg$/, `_${size}.jpg`);
    }

    function resetThumbnailCache() {
        thumbnailImages.clear();
        loadingThumbnailKeys.clear();
        failedThumbnailIds = new Set();
    }

    function preferredThumbSize(displayPx: number): number {
        const dpr = window.devicePixelRatio || 1;
        const physicalPx = displayPx * dpr * 1.5;
        return THUMB_SIZES.find(size => size >= physicalPx) ?? THUMB_SIZES[THUMB_SIZES.length - 1];
    }

    function rememberThumbnail(key: string, el: HTMLImageElement) {
        if (thumbnailImages.has(key)) thumbnailImages.delete(key);
        thumbnailImages.set(key, el);
        while (thumbnailImages.size > MAX_THUMBNAIL_CACHE) {
            const oldest = thumbnailImages.keys().next().value;
            if (!oldest) break;
            thumbnailImages.delete(oldest);
        }
    }

    function queueThumbnailLoad(id: string, size: number) {
        const key = `${id}_${size}`;
        if (thumbnailImages.has(key) || loadingThumbnailKeys.has(key)) return;
        const img = imageMap.get(id);
        if (!img?.thumbnail_path) {
            failedThumbnailIds = new Set([...failedThumbnailIds, id]);
            return;
        }

        loadingThumbnailKeys.add(key);
        const el = new Image();
        el.onload = () => {
            loadingThumbnailKeys.delete(key);
            rememberThumbnail(key, el);
            requestDraw();
        };
        el.onerror = () => {
            loadingThumbnailKeys.delete(key);
            if (!THUMB_SIZES.some(s => thumbnailImages.has(`${id}_${s}`))) {
                failedThumbnailIds = new Set([...failedThumbnailIds, id]);
            }
        };
        el.src = convertFileSrc(sizedThumbPath(img.thumbnail_path, size));
    }

    function pickThumbnail(id: string, displayPx: number): HTMLImageElement | undefined {
        if (failedThumbnailIds.has(id)) return undefined;
        const preferred = preferredThumbSize(displayPx);
        const preferredEl = thumbnailImages.get(`${id}_${preferred}`);
        if (preferredEl?.complete && preferredEl.naturalWidth > 0) {
            thumbnailImages.delete(`${id}_${preferred}`);
            thumbnailImages.set(`${id}_${preferred}`, preferredEl);
            return preferredEl;
        }
        queueThumbnailLoad(id, preferred);

        for (let i = THUMB_SIZES.length - 1; i >= 0; i--) {
            const el = thumbnailImages.get(`${id}_${THUMB_SIZES[i]}`);
            if (el?.complete && el.naturalWidth > 0) return el;
        }
        return undefined;
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
        saveViewState();
    }

    interface ProjectionWorkerCluster {
        id: number;
        label: string;
        count: number;
        colorIndex: number;
        previewPaths: string[];
        x: number;
        y: number;
    }

    interface ProjectionWorkerResponse {
        requestId: number;
        points: Point[];
        clusters: ProjectionWorkerCluster[];
        projectionKey: string;
    }

    function getProjectionWorker(): Worker {
        if (!projectionWorker) {
            projectionWorker = new Worker(new URL('../embedding-projection.worker.ts', import.meta.url), {
                type: 'module',
            });
        }
        return projectionWorker;
    }

    function runProjectionInWorker(
        embeddings: [string, number[]][],
        embeddingImages: ImageWithFile[],
    ): Promise<ProjectionWorkerResponse> {
        const worker = getProjectionWorker();
        const requestId = ++projectionRequestId;

        return new Promise((resolve, reject) => {
            cancelProjectionWork = () => {
                cleanup();
                reject(new Error('Projection cancelled'));
            };
            const handleMessage = (event: MessageEvent<ProjectionWorkerResponse>) => {
                if (event.data.requestId !== requestId) return;
                cleanup();
                resolve(event.data);
            };
            const handleError = (event: ErrorEvent) => {
                cleanup();
                reject(event.error ?? new Error(event.message));
            };
            const cleanup = () => {
                if (cancelProjectionWork) cancelProjectionWork = null;
                worker.removeEventListener('message', handleMessage as EventListener);
                worker.removeEventListener('error', handleError);
            };

            worker.addEventListener('message', handleMessage as EventListener);
            worker.addEventListener('error', handleError);
            worker.postMessage({
                requestId,
                provider: selectedProvider,
                embeddings,
                images: embeddingImages.map(img => ({
                    id: img.image.id,
                    path: img.path,
                    thumbnailPath: img.thumbnail_path,
                })),
            });
        });
    }

    function resetProjectionWorker() {
        const cancel = cancelProjectionWork;
        cancelProjectionWork = null;
        cancel?.();
        projectionWorker?.terminate();
        projectionWorker = null;
    }

    async function loadProjection() {
        const loadSeq = ++projectionLoadSeq;
        try {
            const modelName = selectedProvider === 'gemini' ? 'gemini-embedding-2' : undefined;
            const embeddings = await getAllEmbeddings(modelName);
            if (loadSeq !== projectionLoadSeq) return;
            if (embeddings.length < 2) {
                points = [];
                clusters = [];
                resetThumbnailCache();
                return;
            }

            // Build image map from all embedded images (not just current filter)
            const embeddingIds = embeddings.map(([id]) => id);
            const embeddingImages = await getImagesByIds(embeddingIds);
            if (loadSeq !== projectionLoadSeq) return;
            const map = new Map<string, ImageWithFile>();
            for (const img of embeddingImages) {
                map.set(img.image.id, img);
            }
            imageMap = map;
            staleEmbeddingCount = embeddingIds.length - embeddingImages.length;

            resetProjectionWorker();
            projecting = true;
            const projection = await runProjectionInWorker(embeddings, embeddingImages);
            if (loadSeq !== projectionLoadSeq) return;

            points = projection.points;
            clusters = projection.clusters.map(cluster => ({
                id: cluster.id,
                label: cluster.label,
                count: cluster.count,
                color: CLUSTER_COLORS[cluster.colorIndex % CLUSTER_COLORS.length],
                previewPaths: cluster.previewPaths,
                x: cluster.x,
                y: cluster.y,
            }));

            const newProjectionKey = projection.projectionKey;
            projectionKey = newProjectionKey;

            // Restore view state if it matches the current projection
            const savedState = get(embeddingViewState);
            if (savedState.hasUserView && savedState.provider === selectedProvider && savedState.projectionKey === newProjectionKey) {
                panX = savedState.panX;
                panY = savedState.panY;
                scale = savedState.scale;
                highlightedCluster = savedState.highlightedCluster;
                selectedPoint = savedState.selectedPointId
                    ? points.find(p => p.id === savedState.selectedPointId) ?? null
                    : null;
            } else {
                highlightedCluster = null;
                fitView();
            }

            resetThumbnailCache();
            requestDraw();
        } catch (e) {
            if (e instanceof Error && e.message === 'Projection cancelled') return;
            console.error('Failed to load projection:', e);
        } finally {
            if (loadSeq === projectionLoadSeq) projecting = false;
        }
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
        if (drawQueued) return;
        if (typeof requestAnimationFrame !== 'undefined') {
            drawQueued = true;
            requestAnimationFrame(() => {
                drawQueued = false;
                draw();
            });
        }
    }

    function draw() {
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        ctx.clearRect(0, 0, canvasWidth, canvasHeight);

        const { size: baseThumbSize, useThumb } = computeScatterThumbSize(scale, points.length);

        // Draw points — clean thumbnails, no borders, no transparency
        const margin = baseThumbSize + 10;
        for (const p of points) {
            const [sx, sy] = worldToScreen(p.x, p.y);
            if (sx < -margin || sx > canvasWidth + margin || sy < -margin || sy > canvasHeight + margin) continue;

            const color = CLUSTER_COLORS[p.cluster % CLUSTER_COLORS.length];
            const isSelected = selectedPoint && selectedPoint.id === p.id;
            const isHovered = hoveredPoint && hoveredPoint.id === p.id;

            const thumbEl = useThumb ? pickThumbnail(p.id, baseThumbSize) : undefined;

            if (thumbEl && thumbEl.complete && thumbEl.naturalWidth > 0) {
                const aspect = thumbEl.naturalWidth / thumbEl.naturalHeight;
                let dw: number, dh: number;
                if (aspect >= 1) {
                    dw = baseThumbSize;
                    dh = baseThumbSize / aspect;
                } else {
                    dh = baseThumbSize;
                    dw = baseThumbSize * aspect;
                }

                const dpr = window.devicePixelRatio || 1;
                const dx = Math.round((sx - dw / 2) * dpr) / dpr;
                const dy = Math.round((sy - dh / 2) * dpr) / dpr;
                const drawW = Math.round(dw * dpr) / dpr;
                const drawH = Math.round(dh * dpr) / dpr;
                ctx.drawImage(thumbEl, dx, dy, drawW, drawH);

                if (isSelected || isHovered) {
                    ctx.strokeStyle = '#ffffff';
                    ctx.lineWidth = isSelected ? 2 : 1;
                    ctx.strokeRect(sx - dw / 2, sy - dh / 2, dw, dh);
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
                const [sx, sy] = worldToScreen(cluster.x, cluster.y);
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

    function saveViewState() {
        embeddingViewState.set({
            panX, panY, scale,
            selectedPointId: selectedPoint?.id ?? null,
            highlightedCluster,
            provider: selectedProvider,
            projectionKey,
            hasUserView: true,
        });
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
        saveViewState();
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
        const { size: thumbSize } = computeScatterThumbSize(scale, points.length);
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
        saveViewState();
    }

    function focusImageForLoupe(imageId: string): boolean {
        const idx = $images.findIndex(img => img.image.id === imageId);
        if (idx >= 0) {
            focusedImageOverride.set(null);
            focusedIndex.set(idx);
            return true;
        }
        const img = imageMap.get(imageId);
        if (!img) return false;
        focusedImageOverride.set(img);
        return true;
    }

    function handlePointClick(point: Point) {
        selectedPoint = point;
        focusImageForLoupe(point.id);
        zoomToPoint(point);
        saveViewState();
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
        if (selectedPoint && focusImageForLoupe(selectedPoint.id)) {
            navigateTo('loupe');
        }
    }

    function handleCanvasDblClick(e: MouseEvent) {
        if (!hoveredPoint) return;
        if (focusImageForLoupe(hoveredPoint.id)) {
            navigateTo('loupe');
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
                    const savedState = get(embeddingViewState);
                    if (!savedState.hasUserView || savedState.provider !== selectedProvider || savedState.projectionKey !== projectionKey) {
                        fitView();
                    }
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
            <select class="provider-select" bind:value={selectedProvider} onchange={handleProviderChange}>
                <option value="clip">CLIP ViT-B/32 (local)</option>
                <option value="gemini">Gemini Embedding 2 (API)</option>
            </select>
            <div class="model-detail">
                {#if selectedProvider === 'clip'}
                    {#if !modelAvailable}
                        Model not downloaded
                    {:else}
                        {embeddingCount}/{totalImages} images
                    {/if}
                {:else}
                    {geminiEmbeddingCount}/{totalImages} images
                {/if}
            </div>
        </div>

        {#if configOpen}
            <div class="panel-section config-section">
                {#if selectedProvider === 'gemini' && !hasGoogleKey}
                    <div class="section-header">GEMINI API KEY REQUIRED</div>
                    <p class="key-missing-text">Set your Google API key in Settings to use Gemini embeddings.</p>
                    <button class="settings-link-btn" onclick={() => settingsOpen.set(true)}>
                        Open Settings
                    </button>
                {/if}
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
                        <span class="stat-value">{totalImages}</span>
                    </div>
                    <div class="stat-row">
                        <span class="stat-label">Embeddings</span>
                        <span class="stat-value">{embeddingCount}</span>
                    </div>
                    <button class="action-btn" onclick={handleGenerate} disabled={generating}>
                        {#if generating}
                            Generating {genProgress.current}/{genProgress.total}...
                        {:else if embeddingCount < totalImages}
                            Generate Embeddings ({Math.max(0, totalImages - embeddingCount)} remaining)
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
                    <span class="stat-value">{totalImages}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Embeddings</span>
                    <span class="stat-value">{geminiEmbeddingCount}</span>
                </div>
                <button class="action-btn" onclick={handleGenerateGemini} disabled={generating || !hasGoogleKey} title={hasGoogleKey ? '' : 'Set Google API key in Settings'}>
                    {#if generating}
                        Generating {genProgress.current}/{genProgress.total}...
                    {:else if !hasGoogleKey}
                        Set API Key First
                    {:else if geminiEmbeddingCount < totalImages}
                        Generate Embeddings ({Math.max(0, totalImages - geminiEmbeddingCount)} remaining)
                    {:else}
                        Regenerate All
                    {/if}
                </button>
            </div>
        {/if}

        {#if failedThumbnailIds.size > 0 || staleEmbeddingCount > 0}
            <div class="panel-section warning-section">
                {#if failedThumbnailIds.size > 0}
                    <div class="warning-row">
                        <span class="warning-icon">&#9888;</span>
                        <span>{failedThumbnailIds.size} missing thumbnails</span>
                    </div>
                    <button class="action-btn warning" onclick={handleRegenerateThumbnails} disabled={regeneratingThumbs}>
                        {regeneratingThumbs ? 'Regenerating...' : 'Regenerate Thumbnails'}
                    </button>
                {/if}
                {#if staleEmbeddingCount > 0}
                    <div class="warning-row">
                        <span class="warning-icon">&#9888;</span>
                        <span>{staleEmbeddingCount} stale embeddings</span>
                    </div>
                    <div class="warning-hint">Images deleted but embeddings remain. Regenerate embeddings to clean up.</div>
                {/if}
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
                {#if projecting}
                    <div class="empty-icon">&#8987;</div>
                    <div class="empty-title">Projecting...</div>
                {:else if !modelAvailable && selectedProvider === 'clip'}
                    <div class="empty-icon">&#9881;</div>
                    <div class="empty-title">Model Required</div>
                    <div class="empty-text">Download the CLIP model to generate embeddings</div>
                {:else if currentEmbeddingCount === 0}
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

    .warning-section {
        background: rgba(224, 175, 104, 0.08);
    }

    .warning-row {
        display: flex;
        align-items: center;
        gap: 6px;
        font-size: 11px;
        color: var(--orange);
    }

    .warning-icon {
        font-size: 12px;
    }

    .warning-hint {
        font-size: 9px;
        color: var(--text-secondary);
        margin-top: 4px;
    }

    .action-btn.warning {
        background: rgba(224, 175, 104, 0.15);
        color: var(--orange);
    }

    .action-btn.warning:hover:not(:disabled) {
        background: rgba(224, 175, 104, 0.25);
        border-color: var(--orange);
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

    .key-missing-text {
        font-size: 12px;
        color: var(--text-secondary);
        margin: 0 0 8px 0;
    }
    .settings-link-btn {
        background: none;
        border: 1px solid var(--blue);
        border-radius: var(--radius, 4px);
        padding: 4px 12px;
        font-size: 12px;
        font-family: inherit;
        color: var(--blue);
        cursor: pointer;
    }
    .settings-link-btn:hover {
        background: rgba(122, 162, 247, 0.1);
    }
</style>
