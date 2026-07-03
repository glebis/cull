<script lang="ts">
    import { open } from '@tauri-apps/plugin-dialog';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { totalCount, folders, activeFolder, minSizeFilter, collections, activeCollection, activeDetectedClass, detectedClasses as detectedClassesStore, collectMode, collectModeTarget, smartCollections, activeSmartCollection, showToast, pinnedCollection, showMissing, requestTextInput, clipboardMonitorStatus } from '$lib/stores';
    import { importFolder as apiImportFolder, listImageIds, getImageCount, listFolders, deleteFolder as apiDeleteFolder, listCollections, createCollection, deleteCollectionApi, listSmartCollections, isYoloAvailable, isNudenetAvailable, getDetectionCount, countByDetectedClass, detectObjects, detectNsfw, regenerateThumbnails, rescanSources, checkOllama, analyzeImages, getVisionCount, getClipboardMonitorStatus, startClipboardMonitor, stopClipboardMonitor, setClipboardMonitorCaptureExistingOnStart, moveClipboardCaptureFolder, publishClipboardCollection } from '$lib/api';
    import { loadImagesForCurrentScope } from '$lib/image-loading';
    import type { ClipboardMonitorStatus, ClipboardPublishResult, SmartCollection } from '$lib/api';
    import { applyClipboardMonitorCollection } from '$lib/clipboard-monitor';
    import { MODEL_SETUP_GUIDE_URL, resolveAiSectionExpanded } from '$lib/onboarding';
    import { openUrl } from '@tauri-apps/plugin-opener';
    import { onMount } from 'svelte';
    import { get } from 'svelte/store';

    let importing = $state(false);
    let importCurrent = $state(0);
    let importTotal = $state(0);
    let lastResult = $state('');
    let regenerating = $state(false);
    let regenProgress = $state({ current: 0, total: 0 });
    let rescanning = $state(false);
    let foldersExpanded = $state(true);
    let clipboardStatus = $state<ClipboardMonitorStatus | null>(null);
    let clipboardMoving = $state(false);
    let clipboardPublishing = $state(false);
    let clipboardPublishResult = $state<ClipboardPublishResult | null>(null);

    function setClipboardStatus(status: ClipboardMonitorStatus | null) {
        clipboardStatus = status;
        clipboardMonitorStatus.set(status);
    }

    import { buildDisplayFolders, formatSidebarCount } from '$lib/sidebar-utils';
    import SessionSwitcher from './SessionSwitcher.svelte';
    import { activeCanvas, activeSession, navigateTo, sessionCanvases } from '$lib/stores';
    import { createCanvas, type Canvas } from '$lib/api';

    let displayFolders = $derived(buildDisplayFolders($folders));

    onMount(async () => {
        try {
            const f = await listFolders();
            folders.set(f);
        } catch (e) {
            console.error('Failed to load folders:', e);
            showToast('Failed to load folders', { detail: String(e), type: 'error', duration: 8000 });
        }
        try {
            const c = await listCollections();
            collections.set(c);
        } catch (e) {
            console.error('Failed to load collections:', e);
            showToast('Failed to load collections', { detail: String(e), type: 'error', duration: 8000 });
        }
        try {
            const sc = await listSmartCollections();
            smartCollections.set(sc);
        } catch (e) {
            console.error('Failed to load smart collections:', e);
            showToast('Failed to load smart collections', { detail: String(e), type: 'error', duration: 8000 });
        }
        try {
            setClipboardStatus(await getClipboardMonitorStatus());
        } catch (e) {
            console.error('Failed to load clipboard monitor status:', e);
        }
        try {
            await listen('clipboard-monitor:capture', async () => {
                setClipboardStatus(await getClipboardMonitorStatus());
                const c = await listCollections();
                collections.set(c);
                if (clipboardStatus?.collection_id && get(activeCollection) === clipboardStatus.collection_id) {
                    await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
                }
            });
        } catch (e) {
            console.error('Failed to listen for clipboard monitor captures:', e);
        }
        loadAiState().catch(e => console.error('Failed to load AI state:', e));
    });

    function folderName(path: string): string {
        const parts = path.split('/');
        return parts[parts.length - 1] || path;
    }

    function pinCollection(collectionId: string) {
        pinnedCollection.set(collectionId);
        showToast('Collection pinned, new imports will be added here', { type: 'info', duration: 5000 });
    }

    function unpinCollection() {
        pinnedCollection.set(null);
        showToast('Collection unpinned', { type: 'info', duration: 3000 });
    }

    async function selectSmartCollection(sc: SmartCollection) {
        activeSession.set(null);
        sessionCanvases.set([]);
        activeCanvas.set(null);
        activeSmartCollection.set(sc);
        activeFolder.set(null);
        activeCollection.set(null);
        activeDetectedClass.set(null);
        if (sc.filter_json) {
            try {
                await loadImagesForCurrentScope();
            } catch (e) {
                console.error('Failed to evaluate smart collection:', e);
            }
        }
    }

    async function selectFolder(folder: string | null) {
        activeSession.set(null);
        sessionCanvases.set([]);
        activeCanvas.set(null);
        activeFolder.set(folder);
        activeCollection.set(null);
        activeSmartCollection.set(null);
        activeDetectedClass.set(null);
        try {
            await loadImagesForCurrentScope();
        } catch (e) {
            console.error('Failed to load images for folder:', e);
        }
    }

    async function selectCollection(collectionId: string) {
        activeSession.set(null);
        sessionCanvases.set([]);
        activeCanvas.set(null);
        activeCollection.set(collectionId);
        activeFolder.set(null);
        activeSmartCollection.set(null);
        activeDetectedClass.set(null);
        try {
            await loadImagesForCurrentScope();
        } catch (e) {
            console.error('Failed to load collection images:', e);
        }
    }

    async function handleNewCollection() {
        const name = await requestTextInput({
            title: 'New Collection',
            label: 'Collection name',
            placeholder: 'Collection name',
            confirmLabel: 'Create',
        });
        if (!name || !name.trim()) return;
        try {
            await createCollection(name.trim());
            const c = await listCollections();
            collections.set(c);
        } catch (e) {
            console.error('Failed to create collection:', e);
        }
    }

    async function handleDeleteCollection(event: Event, collectionId: string, collectionName: string) {
        event.stopPropagation();
        if (!window.confirm(`Delete collection "${collectionName}"?`)) return;
        try {
            await deleteCollectionApi(collectionId);
            if (get(activeCollection) === collectionId) {
                activeCollection.set(null);
                activeDetectedClass.set(null);
                await loadImagesForCurrentScope({ force: true, invalidateCache: true });
            }
            const c = await listCollections();
            collections.set(c);
        } catch (e) {
            console.error('Failed to delete collection:', e);
        }
    }

    async function handleDeleteFolder(event: Event, folder: string) {
        event.stopPropagation();
        const name = folderName(folder);
        if (!window.confirm(`Remove folder from library "${name}"? Cull records for images that only exist in this folder will be removed. Original files stay on disk.`)) return;
        try {
            const count = await apiDeleteFolder(folder);
            lastResult = `Removed ${count} images from "${name}"`;
            if (get(activeFolder) === folder) {
                activeFolder.set(null);
            }
            await refreshImages();
        } catch (e) {
            lastResult = `Error: ${e}`;
        }
    }

    async function handleToggleClipboardMonitor() {
        const wasRunning = clipboardStatus?.running ?? false;
        try {
            const nextStatus = wasRunning
                ? await stopClipboardMonitor()
                : await startClipboardMonitor(null);
            setClipboardStatus(nextStatus);
            const c = await listCollections();
            collections.set(c);
            if (!wasRunning && nextStatus.collection_id) {
                await applyClipboardMonitorCollection(nextStatus.collection_id);
            }
        } catch (e) {
            showToast('Clipboard Monitor failed', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleMoveClipboardCaptureFolder() {
        if (clipboardMoving) return;
        const selected = await open({ directory: true, multiple: false });
        if (!selected || Array.isArray(selected)) return;
        clipboardMoving = true;
        try {
            setClipboardStatus(await moveClipboardCaptureFolder(selected));
            showToast('Clipboard folder moved', { detail: selected, type: 'success', duration: 8000 });
        } catch (e) {
            showToast('Move failed', { detail: String(e), type: 'error', duration: 10000 });
        } finally {
            clipboardMoving = false;
        }
    }

    async function handleClipboardCaptureExistingChange(event: Event) {
        const enabled = (event.currentTarget as HTMLInputElement).checked;
        try {
            setClipboardStatus(await setClipboardMonitorCaptureExistingOnStart(enabled));
        } catch (e) {
            showToast('Clipboard setting failed', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handlePublishClipboardCollection() {
        if (!clipboardStatus?.collection_id || clipboardPublishing) return;
        clipboardPublishing = true;
        try {
            clipboardPublishResult = await publishClipboardCollection(clipboardStatus.collection_id);
            try {
                await navigator.clipboard.writeText(clipboardPublishResult.url);
            } catch (e) {
                showToast('Published clipboard collection', { detail: `Copy failed: ${e}`, type: 'warning', duration: 8000 });
                return;
            }
            showToast('Published clipboard collection', { detail: clipboardPublishResult.url, type: 'success', duration: 10000 });
        } catch (e) {
            showToast('Publish failed', { detail: String(e), type: 'error', duration: 10000 });
        } finally {
            clipboardPublishing = false;
        }
    }

    const SIZE_PRESETS = [
        { label: 'All', value: 0 },
        { label: '>64', value: 64 },
        { label: '>256', value: 256 },
        { label: '>512', value: 512 },
        { label: '>1024', value: 1024 },
    ];

    function handleSizeFilter(value: number) {
        minSizeFilter.set(value);
    }

    async function handleRescan() {
        rescanning = true;
        try {
            const count = await rescanSources();
            lastResult = `Detected sources for ${count} images`;
            await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        } catch (e) {
            lastResult = `Rescan error: ${e}`;
        } finally {
            rescanning = false;
        }
    }

    async function handleRegenerateThumbnails() {
        regenerating = true;
        regenProgress = { current: 0, total: 0 };

        const unlisten: UnlistenFn = await listen<{ current: number; total: number }>(
            'thumbnail-progress',
            (event) => {
                regenProgress = event.payload;
            }
        );

        try {
            const count = await regenerateThumbnails();
            lastResult = `Regenerated ${count} thumbnails`;
        } catch (e) {
            lastResult = `Thumbnail error: ${e}`;
        } finally {
            unlisten();
            regenerating = false;
        }
    }

    async function handleImportFolder() {
        const selected = await open({ directory: true, multiple: false });
        if (!selected) return;

        importing = true;
        importCurrent = 0;
        importTotal = 0;
        lastResult = '';

        // Listen for progress events
        let lastRefresh = 0;
        const unlisten: UnlistenFn = await listen<{ current: number; total: number; filename: string }>(
            'import-progress',
            async (event) => {
                importCurrent = event.payload.current;
                importTotal = event.payload.total;

                // Refresh image count every 20 imports
                if (importCurrent - lastRefresh >= 20) {
                    lastRefresh = importCurrent;
                    const count = await getImageCount();
                    totalCount.set(count);
                }
            }
        );

        try {
            const result = await apiImportFolder(selected as string);
            const folderName = (selected as string).split('/').filter(Boolean).pop() ?? selected;
            lastResult = `+${result.imported} imported, ${result.skipped} skipped`;
            if (result.errors.length > 0) {
                lastResult += `, ${result.errors.length} errors`;
            }
            showToast(`Imported "${folderName}"`, {
                detail: lastResult,
                type: 'success',
                duration: 8000,
            });
            await refreshImages();
        } catch (e) {
            lastResult = `Error: ${e}`;
            showToast('Import failed', { detail: String(e), type: 'error', duration: 10000 });
        } finally {
            unlisten();
            importing = false;
        }
    }

    // AI Models state. Collapsed by default until the library has images
    // so first-run users see content sections, not model jargon; a manual
    // toggle always wins.
    let aiToggled = $state<boolean | null>(null);
    let aiExpanded = $derived(resolveAiSectionExpanded(aiToggled, $totalCount));
    let yoloReady = $state(false);
    let nudenetReady = $state(false);
    let yoloProcessed = $state(0);
    let nudenetProcessed = $state(0);
    let selectedYoloVariant = $state('medium');
    let detectedClasses = $state<[string, number][]>([]);
    let detectingBatch = $state(false);
    let ollamaModels = $state<string[]>([]);
    let ollamaReady = $derived(ollamaModels.length > 0);
    let visionProcessed = $state(0);
    let analyzingBatch = $state(false);

    function openModelSetupGuide() {
        openUrl(MODEL_SETUP_GUIDE_URL).catch(e => console.error('Failed to open setup guide:', e));
    }

    async function loadAiState() {
        try {
            yoloReady = await isYoloAvailable(selectedYoloVariant);
            nudenetReady = await isNudenetAvailable();
            if (yoloReady) {
                const variantName = selectedYoloVariant === 'nano' ? 'yolo11n' : selectedYoloVariant === 'small' ? 'yolo11s' : 'yolo11m';
                yoloProcessed = await getDetectionCount(variantName);
            }
            if (nudenetReady) {
                nudenetProcessed = await getDetectionCount('nudenet');
            }
            await loadDetectedClasses();
        } catch (_) {}
        try {
            ollamaModels = await checkOllama();
            visionProcessed = await getVisionCount();
        } catch (_) {
            ollamaModels = [];
        }
    }

    async function handleAnalyzeBatch() {
        if (analyzingBatch) return;
        analyzingBatch = true;
        try {
            const allIds = await listImageIds();
            await analyzeImages(allIds);
            await loadAiState();
            await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        } catch (e) {
            console.error('Vision analysis error:', e);
        } finally {
            analyzingBatch = false;
        }
    }

    async function loadDetectedClasses() {
        const commonClasses = ['person', 'dog', 'cat', 'car', 'bicycle', 'bird', 'horse', 'chair', 'bottle', 'laptop', 'phone', 'book'];
        const results: [string, number][] = [];
        for (const cls of commonClasses) {
            try {
                const count = await countByDetectedClass(cls);
                if (count > 0) results.push([cls, count]);
            } catch (_) {}
        }
        results.sort((a, b) => b[1] - a[1]);
        detectedClasses = results;
        detectedClassesStore.set(results);
    }

    async function handleDetectRemaining() {
        if (detectingBatch) return;
        detectingBatch = true;
        try {
            const allIds = await listImageIds();
            if (yoloReady) await detectObjects(allIds, selectedYoloVariant);
            if (nudenetReady) await detectNsfw(allIds);
            await loadAiState();
            await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        } catch (e) {
            console.error('Batch detection error:', e);
        } finally {
            detectingBatch = false;
        }
    }

    async function filterByClass(className: string) {
        try {
            const count = await countByDetectedClass(className);
            if (count === 0) return;
            activeSession.set(null);
            sessionCanvases.set([]);
            activeCanvas.set(null);
            activeSmartCollection.set(null);
            activeFolder.set(null);
            activeCollection.set(null);
            activeDetectedClass.set(className);
            await loadImagesForCurrentScope();
        } catch (e) {
            console.error('Filter by class error:', e);
        }
    }

    function selectCanvas(canvas: Canvas) {
        activeCanvas.set(canvas);
        navigateTo('canvas');
    }

    async function refreshImages() {
        const count = await getImageCount();
        totalCount.set(count);
        await loadImagesForCurrentScope({ force: true, invalidateCache: true });
        // Refresh folders too
        try {
            const f = await listFolders();
            folders.set(f);
        } catch (_) {}
    }
</script>

<aside class="sidebar" aria-label="Library sidebar">
    <div class="sidebar-scroll">
        <SessionSwitcher />

    {#if $activeSession}
        <div class="section">
            <div class="section-header">
                <span>CANVASES</span>
                <button class="section-action" onclick={async () => {
                    if ($activeSession) {
                        const canvas = await createCanvas($activeSession.id, 'New Canvas', 'manual');
                        sessionCanvases.update(c => [...c, canvas]);
                        selectCanvas(canvas);
                    }
                }} aria-label="New canvas">+</button>
            </div>
            {#each $sessionCanvases as canvas}
                <button
                    class="section-item"
                    class:active={$activeCanvas?.id === canvas.id}
                    onclick={() => selectCanvas(canvas)}
                    aria-current={$activeCanvas?.id === canvas.id ? 'true' : undefined}
                >
                    <span class="item-label">{canvas.name}</span>
                    <span class="count">{canvas.canvas_type}</span>
                </button>
            {/each}
        </div>
    {/if}

    <div class="section">
        <div class="section-header">LIBRARY</div>
        <button
            class="section-item"
            class:active={$activeFolder === null && $activeCollection === null && $activeSmartCollection === null}
            onclick={() => selectFolder(null)}
            aria-current={$activeFolder === null && $activeCollection === null && $activeSmartCollection === null ? 'true' : undefined}
        >
            <span class="icon">&#9632;</span>
            <span class="item-label">All Images</span>
            <span class="count">{formatSidebarCount($totalCount)}</span>
        </button>

        {#if displayFolders.length > 0}
            <button
                class="folders-toggle"
                onclick={() => foldersExpanded = !foldersExpanded}
                aria-expanded={foldersExpanded}
            >
                <span class="toggle-arrow">{foldersExpanded ? '▾' : '▸'}</span>
                <span class="folders-toggle-label">Folders</span>
                <span class="count">{formatSidebarCount(displayFolders.length)}</span>
            </button>

            {#if foldersExpanded}
                <div role="tree" aria-label="Folder hierarchy">
                {#each displayFolders as folder}
                    <div class="folder-row" class:active={$activeFolder === folder.fullPath} style="padding-left: {folder.depth * 12}px" role="treeitem" aria-level={folder.depth + 1} {...(folder.hasChildren && folder.count === 0 ? { 'aria-expanded': 'true' } : {})}>
                        {#if folder.count > 0}
                            <button
                                class="section-item"
                                onclick={() => selectFolder(folder.fullPath)}
                                title={folder.fullPath}
                                aria-current={$activeFolder === folder.fullPath ? 'true' : undefined}
                            >
                                <span class="icon">{folder.hasChildren ? '▾' : '▸'}</span>
                                <span class="folder-label">{folder.name}</span>
                                <span class="count">{formatSidebarCount(folder.count)}</span>
                            </button>
                            <button
                                class="delete-btn"
                                onclick={(e: Event) => handleDeleteFolder(e, folder.fullPath)}
                                title="Remove folder from library"
                                aria-label={`Remove folder from library: ${folder.name}`}
                            >&times;</button>
                        {:else}
                            <span class="section-item folder-group">
                                <span class="icon">▾</span>
                                <span class="folder-label">{folder.name}</span>
                            </span>
                        {/if}
                    </div>
                {/each}
                </div>
            {/if}
        {/if}
    </div>

    <div class="section">
        <div class="section-header">FILTERS</div>
        <div class="filter-row">
            <span class="filter-label">Min size</span>
            <div class="filter-presets">
                {#each SIZE_PRESETS as preset}
                    <button
                        class="preset-btn"
                        class:active={$minSizeFilter === preset.value}
                        onclick={() => handleSizeFilter(preset.value)}
                    >{preset.label}</button>
                {/each}
            </div>
        </div>
        <label class="show-missing-toggle">
            <input type="checkbox" bind:checked={$showMissing} />
            Show missing files
        </label>
    </div>

    <div class="section">
        <button
            class="folders-toggle"
            onclick={() => aiToggled = !aiExpanded}
            aria-expanded={aiExpanded}
        >
            <span class="toggle-arrow">{aiExpanded ? '▾' : '▸'}</span>
            <span class="folders-toggle-label">AI MODELS</span>
        </button>

        {#if aiExpanded}
            <div class="ai-models-content">
                <div class="model-row">
                    <span class="model-name">Object detection YOLO</span>
                    {#if yoloReady}
                        <span class="model-status ready">ready</span>
                    {:else}
                        <span class="model-status missing">optional</span>
                    {/if}
                </div>

                {#if !yoloReady}
                    <div class="model-download-row">
                        <select class="variant-select" bind:value={selectedYoloVariant}>
                            <option value="nano">nano 6MB</option>
                            <option value="small">small 22MB</option>
                            <option value="medium">medium 50MB</option>
                        </select>
                        <button class="model-help-link" onclick={openModelSetupGuide}>Setup guide ↗</button>
                    </div>
                {/if}

                <div class="model-row">
                    <span class="model-name">Content filter NudeNet</span>
                    {#if nudenetReady}
                        <span class="model-status ready">ready</span>
                    {:else}
                        <span class="model-status missing">optional</span>
                    {/if}
                </div>

                {#if !nudenetReady}
                    <button class="model-help-link" onclick={openModelSetupGuide}>Setup guide ↗</button>
                {/if}

                <div class="model-row">
                    <span class="model-name">Image descriptions Ollama</span>
                    {#if ollamaReady}
                        <span class="model-status ready">{ollamaModels.length} models</span>
                    {:else}
                        <span class="model-status missing">optional</span>
                    {/if}
                </div>

                {#if yoloReady || nudenetReady}
                    <div class="processed-row">
                        <span class="processed-label">Detection</span>
                        <span class="processed-count">{yoloProcessed}/{$totalCount}</span>
                    </div>
                    {#if yoloProcessed < $totalCount}
                        <button class="detect-btn" onclick={handleDetectRemaining} disabled={detectingBatch}>
                            {detectingBatch ? 'Detecting...' : `Analyze uncatalogued images ${formatSidebarCount($totalCount - yoloProcessed)}`}
                        </button>
                    {/if}
                {/if}

                {#if ollamaReady}
                    <div class="processed-row">
                        <span class="processed-label">Vision</span>
                        <span class="processed-count">{visionProcessed}/{$totalCount}</span>
                    </div>
                    {#if visionProcessed < $totalCount}
                        <button class="detect-btn" onclick={handleAnalyzeBatch} disabled={analyzingBatch}>
                            {analyzingBatch ? 'Analyzing...' : `Analyze uncatalogued images ${formatSidebarCount($totalCount - visionProcessed)}`}
                        </button>
                    {/if}
                {/if}

                {#if detectedClasses.length > 0}
                    <div class="detected-header">DETECTED</div>
                    {#each detectedClasses as [cls, count]}
                        <button class="section-item detected-class" onclick={() => filterByClass(cls)}>
                            <span class="class-tag">{cls}</span>
                            <span class="count">{formatSidebarCount(count)}</span>
                        </button>
                    {/each}
                {/if}
            </div>
        {/if}
    </div>

    {#if $smartCollections.length > 0}
    <div class="section">
        <div class="section-header">SMART</div>
        {#each $smartCollections as sc}
            <button class="section-item"
                class:active={$activeSmartCollection?.id === sc.id}
                onclick={() => selectSmartCollection(sc)}
                aria-current={$activeSmartCollection?.id === sc.id ? 'true' : undefined}>
                <span class="icon">&#9733;</span>
                <span class="item-label">{sc.name}</span>
                <span class="count">{formatSidebarCount(sc.image_count)}</span>
            </button>
        {/each}
    </div>
    {/if}

    <div class="section clipboard-monitor">
        <div class="section-header">CLIPBOARD MONITOR</div>
        <button
            class="section-item"
            class:active={clipboardStatus?.running}
            onclick={handleToggleClipboardMonitor}
            disabled={clipboardMoving || clipboardPublishing}
            aria-pressed={clipboardStatus?.running ?? false}
        >
            <span class="icon">{clipboardStatus?.running ? '■' : '▶'}</span>
            {clipboardStatus?.running ? 'Stop Monitor' : 'Monitor Clipboard'}
        </button>
        {#if clipboardStatus}
            <div class="section-meta">{clipboardStatus.access_status}</div>
            <div class="section-meta" title={clipboardStatus.capture_dir}>
                {clipboardStatus.capture_dir.split('/').pop() || clipboardStatus.capture_dir}
            </div>
            {#if clipboardStatus.collection_name}
                <div class="section-meta">{clipboardStatus.collection_name} · {clipboardStatus.captured_count}</div>
            {/if}
            <label class="clipboard-option">
                <input
                    type="checkbox"
                    checked={clipboardStatus.capture_existing_on_start}
                    onchange={handleClipboardCaptureExistingChange}
                    disabled={clipboardMoving || clipboardPublishing}
                />
                <span>Capture current image on start</span>
            </label>
            <div class="section-actions">
                <button
                    class="section-item compact"
                    onclick={handleMoveClipboardCaptureFolder}
                    disabled={clipboardMoving}
                >
                    <span class="icon">↔</span>
                    {clipboardMoving ? 'Moving...' : 'Move Folder'}
                </button>
                <button
                    class="section-item compact"
                    onclick={handlePublishClipboardCollection}
                    disabled={!clipboardStatus.collection_id || clipboardPublishing}
                >
                    <span class="icon">↗</span>
                    {clipboardPublishing ? 'Publishing...' : 'Publish clipboard collection'}
                </button>
            </div>
            {#if clipboardPublishResult}
                <div class="section-meta" title={clipboardPublishResult.url}>{clipboardPublishResult.url}</div>
            {/if}
        {/if}
    </div>

    <div class="section">
        <div class="section-header">
            COLLECTIONS
            <button class="new-collection-btn" onclick={handleNewCollection} title="New Collection" aria-label="New collection">+</button>
        </div>
        {#if $pinnedCollection}
            {@const pinnedName = $collections.find(([id]) => id === $pinnedCollection)?.[1] ?? 'Unknown'}
            <div class="pinned-indicator">
                <span class="pin-icon generated-pin" aria-hidden="true"></span>
                <span class="pin-name">{pinnedName}</span>
                <button class="pin-action" onclick={unpinCollection}>Unpin</button>
            </div>
        {/if}
        {#if $collectMode && $collectModeTarget}
            <div class="collect-indicator">Collecting into: {$collections.find(c => c[0] === $collectModeTarget)?.[1] ?? '...'}</div>
        {/if}
        {#if $collections.length === 0}
            <div class="section-empty">No collections yet</div>
        {:else}
            {#each $collections as [id, name, count]}
                <div class="folder-row" class:active={$activeCollection === id}>
                    <button
                        class="section-item"
                        onclick={() => selectCollection(id)}
                        aria-current={$activeCollection === id ? 'true' : undefined}
                    >
                        <span class="icon">&#9671;</span>
                        <span class="item-label">{name}</span>
                        <span class="count">{formatSidebarCount(count)}</span>
                    </button>
                    <button
                        class="pin-btn"
                        class:active={$pinnedCollection === id}
                        onclick={(e: Event) => { e.stopPropagation(); $pinnedCollection === id ? unpinCollection() : pinCollection(id); }}
                        title={$pinnedCollection === id ? 'Unpin' : 'Pin as active'}
                        aria-label={$pinnedCollection === id ? `Unpin collection: ${name}` : `Pin collection as active: ${name}`}
                        aria-pressed={$pinnedCollection === id}
                    >
                        <span class="generated-pin" aria-hidden="true"></span>
                    </button>
                    <button
                        class="delete-btn"
                        onclick={(e: Event) => handleDeleteCollection(e, id, name)}
                        title="Delete collection"
                        aria-label={`Delete collection: ${name}`}
                    >&times;</button>
                </div>
            {/each}
        {/if}
    </div>
    </div>

    <div class="sidebar-footer" aria-live="polite" aria-busy={importing || regenerating || rescanning}>
        {#if lastResult}
            <div class="import-result">{lastResult}</div>
        {/if}
        {#if importing}
            <div class="sr-only">
                {importTotal > 0 ? `Importing ${importCurrent} of ${importTotal}` : 'Scanning folder'}
            </div>
        {:else if regenerating}
            <div class="sr-only">
                Regenerating thumbnails {regenProgress.current} of {regenProgress.total}
            </div>
        {:else if rescanning}
            <div class="sr-only">Rescanning sources</div>
        {/if}
        <div class="footer-actions">
            <button class="import-btn primary" onclick={handleImportFolder} disabled={importing || regenerating || rescanning}>
                {importing ? (importTotal > 0 ? `Importing ${importCurrent}/${importTotal}...` : 'Scanning...') : '+ Import Folder'}
            </button>
            <div class="footer-secondary-actions">
                <button
                    class="import-btn secondary"
                    onclick={handleRegenerateThumbnails}
                    disabled={importing || regenerating || rescanning}
                    aria-label={regenerating ? `Regenerating thumbnails ${regenProgress.current} of ${regenProgress.total}` : 'Regenerate thumbnails'}
                >
                    {regenerating ? `${regenProgress.current}/${regenProgress.total}` : 'Thumbnails'}
                </button>
                <button
                    class="import-btn secondary"
                    onclick={handleRescan}
                    disabled={importing || regenerating || rescanning}
                    aria-label={rescanning ? 'Rescanning sources' : 'Rescan sources'}
                >
                    {rescanning ? 'Scanning' : 'Sources'}
                </button>
            </div>
        </div>
    </div>
</aside>

<style>
    .sidebar {
        width: 220px;
        background: var(--surface);
        border-right: 1px solid var(--border);
        display: flex;
        flex-direction: column;
        grid-area: sidebar;
        min-height: 0;
        overflow: hidden;
    }
    .sidebar-scroll {
        flex: 1 1 auto;
        min-height: 0;
        overflow-y: auto;
        padding-bottom: var(--spacing);
    }
    .section {
        padding: var(--spacing);
    }
    .section-header {
        font-size: 10px;
        font-weight: 700;
        color: var(--text-secondary);
        letter-spacing: 0.1em;
        margin-bottom: 6px;
        display: flex;
        align-items: center;
    }
    .section-item {
        font-size: 12px;
        padding: 6px 8px;
        border-radius: var(--radius);
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 6px;
        width: 100%;
        background: none;
        border: none;
        color: inherit;
        font-family: inherit;
        text-align: left;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        min-height: 28px;
    }
    .section-item:hover {
        background: var(--border);
    }
    .section-item.active {
        background: color-mix(in srgb, var(--blue) 10%, transparent);
        color: var(--blue);
    }
    .section-item:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .section-item.compact {
        font-size: 11px;
        line-height: 1.25;
        min-height: 32px;
        padding: 6px 8px;
        white-space: normal;
    }
    .section-actions {
        display: grid;
        grid-template-columns: minmax(0, 1fr);
        gap: 4px;
        padding-top: 4px;
    }
    .section-meta {
        color: var(--text-secondary);
        font-size: 10px;
        overflow: hidden;
        padding: 2px 8px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .clipboard-option {
        align-items: flex-start;
        color: var(--text-secondary);
        display: flex;
        font-size: 11px;
        gap: 6px;
        line-height: 1.3;
        padding: 6px 8px 2px;
    }
    .clipboard-option input {
        accent-color: var(--blue);
        flex: none;
        margin: 1px 0 0;
    }
    .icon {
        font-size: 8px;
        flex: none;
    }
    .item-label {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .count {
        color: var(--text-secondary);
        margin-left: auto;
        font-size: 11px;
        flex: none;
    }
    .folder-row {
        display: flex;
        align-items: center;
        border-radius: var(--radius);
    }
    .folder-row:hover {
        background: var(--border);
    }
    .folder-row.active {
        background: color-mix(in srgb, var(--blue) 10%, transparent);
    }
    .folder-row.active .section-item {
        color: var(--blue);
    }
    .folder-row .section-item:hover {
        background: none;
    }
    .folder-row .section-item {
        flex: 1;
        min-width: 0;
    }
    .delete-btn {
        align-items: center;
        display: inline-flex;
        height: 24px;
        justify-content: center;
        margin-right: 4px;
        font-size: 14px;
        line-height: 1;
        color: var(--text-secondary);
        cursor: pointer;
        flex-shrink: 0;
        background: none;
        border: none;
        opacity: 0;
        padding: 0;
        pointer-events: none;
        font-family: inherit;
        width: 24px;
    }
    .folder-row:hover .delete-btn,
    .folder-row:focus-within .delete-btn {
        opacity: 1;
        pointer-events: auto;
    }
    .delete-btn:hover {
        color: var(--red);
    }
    .folders-toggle {
        font-size: 11px;
        padding: 6px 8px;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 4px;
        width: 100%;
        background: none;
        border: none;
        color: var(--text-secondary);
        font-family: inherit;
        text-align: left;
        margin-top: 4px;
        min-height: 28px;
    }
    .folders-toggle:hover {
        color: var(--text);
    }
    .toggle-arrow {
        font-size: 8px;
        width: 10px;
        text-align: center;
    }
    .folders-toggle-label {
        font-size: 10px;
        font-weight: 600;
        letter-spacing: 0.05em;
        text-transform: uppercase;
    }
    .folder-label {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .folder-group {
        cursor: default;
        color: var(--text-secondary);
        font-size: 11px;
        font-weight: 600;
    }
    .folder-group:hover {
        background: none;
    }
    .filter-row {
        padding: 4px 8px;
    }
    .filter-label {
        font-size: 11px;
        color: var(--text-secondary);
        display: block;
        margin-bottom: 4px;
    }
    .filter-presets {
        display: flex;
        gap: 2px;
    }
    .preset-btn {
        font-size: 10px;
        padding: 4px 8px;
        border-radius: var(--radius);
        border: 1px solid var(--border);
        background: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-family: inherit;
    }
    .preset-btn:hover {
        background: var(--border);
    }
    .preset-btn.active {
        background: color-mix(in srgb, var(--blue) 15%, transparent);
        color: var(--blue);
        border-color: var(--blue);
    }
    .show-missing-toggle {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 6px 8px;
        font-size: 11px;
        color: var(--text-secondary);
        cursor: pointer;
    }
    .show-missing-toggle:hover {
        color: var(--text);
    }
    .show-missing-toggle input {
        accent-color: var(--blue);
    }
    .new-collection-btn {
        align-items: center;
        display: inline-flex;
        justify-content: center;
        margin-left: auto;
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 14px;
        font-weight: 700;
        height: 24px;
        padding: 0;
        line-height: 1;
        font-family: inherit;
        width: 24px;
    }
    .new-collection-btn:hover {
        color: var(--blue);
    }
    .collect-indicator {
        font-size: 10px;
        color: var(--green);
        padding: 2px 8px 4px;
        font-style: italic;
    }
    .section-empty {
        font-size: 11px;
        color: var(--text-secondary);
        padding: 4px 8px;
        font-style: italic;
    }
    .sidebar-footer {
        margin-top: auto;
        padding: var(--spacing);
        border-top: 1px solid var(--border);
        background: var(--surface);
        flex: 0 0 auto;
    }
    .import-result {
        font-size: 10px;
        color: var(--green);
        margin-bottom: 6px;
        word-break: break-word;
    }
    .footer-actions {
        display: grid;
        gap: 6px;
    }
    .footer-secondary-actions {
        display: grid;
        gap: 6px;
        grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
    .import-btn {
        width: 100%;
        background: color-mix(in srgb, var(--blue) 15%, transparent);
        color: var(--blue);
        border: 1px solid var(--border);
        font-family: var(--font);
        font-size: 12px;
        align-items: center;
        border-radius: var(--radius);
        cursor: pointer;
        display: flex;
        justify-content: center;
        line-height: 1.2;
        min-height: 32px;
        overflow: hidden;
        padding: 0 10px;
        text-align: center;
        text-overflow: ellipsis;
        transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
        white-space: nowrap;
    }
    .import-btn:hover:not(:disabled) {
        background: color-mix(in srgb, var(--blue) 25%, transparent);
        border-color: var(--blue);
    }
    .import-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .import-btn.secondary {
        background: color-mix(in srgb, var(--blue) 8%, transparent);
        font-size: 10px;
        min-height: 32px;
        padding: 0 6px;
    }
    /* AI Models section */
    .ai-models-content {
        padding: 0 0 0 8px;
    }
    .model-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 6px;
        padding: 3px 0;
        font-size: 11px;
    }
    .model-name {
        color: var(--text);
        font-weight: 600;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .model-status {
        flex: none;
        font-size: 10px;
        white-space: nowrap;
    }
    .model-status.ready {
        color: var(--green);
    }
    .model-status.missing {
        color: var(--text-secondary);
    }
    .model-status.downloading {
        color: var(--orange);
    }
    .progress-bar {
        height: 3px;
        background: var(--border);
        border-radius: 2px;
        margin: 2px 0 4px;
        overflow: hidden;
    }
    .progress-fill {
        height: 100%;
        background: var(--blue);
        transition: width 0.3s;
    }
    .model-download-row {
        display: flex;
        gap: 4px;
        margin: 2px 0 4px;
    }
    .model-help-link {
        background: none;
        border: none;
        color: var(--blue);
        cursor: pointer;
        font-family: var(--font);
        font-size: 10px;
        min-height: 24px;
        padding: 2px 0;
        text-align: left;
        text-decoration: underline;
    }
    .model-help-link:hover {
        opacity: 0.8;
    }
    .variant-select {
        flex: 1;
        font-size: 10px;
        padding: 2px 4px;
        background: var(--bg);
        color: var(--text);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        font-family: inherit;
    }
    .download-btn {
        font-size: 10px;
        padding: 2px 6px;
        background: color-mix(in srgb, var(--blue) 15%, transparent);
        color: var(--blue);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        cursor: pointer;
        font-family: inherit;
    }
    .download-btn:hover {
        background: color-mix(in srgb, var(--blue) 25%, transparent);
        border-color: var(--blue);
    }
    .download-btn.full-width {
        width: 100%;
        margin: 2px 0 4px;
    }
    .processed-row {
        display: flex;
        justify-content: space-between;
        font-size: 10px;
        color: var(--text-secondary);
        padding: 4px 0 2px;
    }
    .processed-label {
        color: var(--text-secondary);
    }
    .processed-count {
        color: var(--text);
    }
    .detect-btn {
        width: 100%;
        font-size: 10px;
        padding: 3px 6px;
        background: none;
        color: var(--blue);
        border: none;
        cursor: pointer;
        font-family: inherit;
        text-align: left;
    }
    .detect-btn:hover:not(:disabled) {
        color: var(--text);
    }
    .detect-btn:disabled {
        color: var(--text-secondary);
        cursor: not-allowed;
    }
    .detected-header {
        font-size: 9px;
        font-weight: 700;
        color: var(--text-secondary);
        letter-spacing: 0.1em;
        padding: 6px 0 2px;
    }
    .detected-class {
        padding: 2px 0;
    }
    .class-tag {
        color: var(--purple);
    }
    .pinned-indicator {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 6px 12px;
        margin: 4px 8px;
        background: var(--bg);
        border-radius: 6px;
        border: 1px solid var(--green);
        font-size: 12px;
    }
    .pin-icon { flex: none; }
    .pin-name { color: var(--text); flex: 1; }
    .pin-action {
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 11px;
        font-family: inherit;
    }
    .pin-action:hover { color: var(--text); }
    .pin-btn {
        align-items: center;
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        display: inline-flex;
        height: 24px;
        justify-content: center;
        opacity: 0;
        padding: 0;
        pointer-events: none;
        transition: color 0.12s ease, opacity 0.12s ease;
        width: 24px;
    }
    .folder-row:hover .pin-btn,
    .folder-row:focus-within .pin-btn {
        opacity: 0.7;
        pointer-events: auto;
    }
    .pin-btn:hover,
    .pin-btn:focus-visible,
    .pin-btn.active {
        opacity: 1;
        pointer-events: auto;
    }
    .pin-btn:hover,
    .pin-btn:focus-visible {
        color: var(--blue);
    }
    .pin-btn.active {
        color: var(--green);
    }
    .generated-pin {
        display: inline-block;
        height: 13px;
        position: relative;
        transform: rotate(35deg);
        width: 10px;
    }
    .generated-pin::before {
        background: color-mix(in srgb, currentColor 12%, transparent);
        border: 1px solid currentColor;
        border-radius: 1px;
        content: '';
        height: 6px;
        left: 1px;
        position: absolute;
        top: 0;
        width: 7px;
    }
    .generated-pin::after {
        background: currentColor;
        box-shadow: 0 7px 0 -0.5px currentColor;
        content: '';
        height: 9px;
        left: 5px;
        position: absolute;
        top: 6px;
        width: 1px;
    }
    .sr-only {
        border: 0;
        clip: rect(0 0 0 0);
        height: 1px;
        margin: -1px;
        overflow: hidden;
        padding: 0;
        position: absolute;
        white-space: nowrap;
        width: 1px;
    }
</style>
