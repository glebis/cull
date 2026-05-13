<script lang="ts">
    import { open } from '@tauri-apps/plugin-dialog';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { totalCount, images, focusedIndex, folders, activeFolder, minSizeFilter, collections, activeCollection, collectMode, collectModeTarget, smartCollections, activeSmartCollection, showToast, pinnedCollection, showMissing } from '$lib/stores';
    import { importFolder as apiImportFolder, listImages, listImagesByFolder, listImagesFiltered, getImageCount, listFolders, deleteFolder as apiDeleteFolder, listCollections, createCollection, listCollectionImages, deleteCollectionApi, listSmartCollections, evaluateSmartCollection, isYoloAvailable, isNudenetAvailable, downloadYoloModel, downloadNudenetModel, getDetectionCount, searchByDetectedClass, detectObjects, detectNsfw, getImagesByIds, regenerateThumbnails, rescanSources, checkOllama, analyzeImages, getVisionCount } from '$lib/api';
    import type { SmartCollection } from '$lib/api';
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

    import { buildDisplayFolders } from '$lib/sidebar-utils';
    import SessionSwitcher from './SessionSwitcher.svelte';
    import { activeSession, sessionCanvases } from '$lib/stores';
    import { createCanvas } from '$lib/api';

    let displayFolders = $derived(buildDisplayFolders($folders));

    onMount(async () => {
        try {
            const f = await listFolders();
            folders.set(f);
        } catch (e) {
            console.error('Failed to load folders:', e);
        }
        try {
            const c = await listCollections();
            collections.set(c);
        } catch (e) {
            console.error('Failed to load collections:', e);
        }
        try {
            const sc = await listSmartCollections();
            smartCollections.set(sc);
        } catch (e) {
            console.error('Failed to load smart collections:', e);
        }
        loadAiState().catch(e => console.error('Failed to load AI state:', e));
    });

    function folderName(path: string): string {
        const parts = path.split('/');
        return parts[parts.length - 1] || path;
    }

    function pinCollection(collectionId: string) {
        pinnedCollection.set(collectionId);
        showToast('Collection pinned — new imports will be added here', { type: 'info', duration: 5000 });
    }

    function unpinCollection() {
        pinnedCollection.set(null);
        showToast('Collection unpinned', { type: 'info', duration: 3000 });
    }

    async function selectSmartCollection(sc: SmartCollection) {
        activeSession.set(null);
        sessionCanvases.set([]);
        activeSmartCollection.set(sc);
        activeFolder.set(null);
        activeCollection.set(null);
        if (sc.filter_json) {
            try {
                const results = await evaluateSmartCollection(sc.filter_json);
                images.set(results);
                focusedIndex.set(0);
            } catch (e) {
                console.error('Failed to evaluate smart collection:', e);
            }
        }
    }

    async function selectFolder(folder: string | null) {
        activeSession.set(null);
        sessionCanvases.set([]);
        activeFolder.set(folder);
        activeCollection.set(null);
        activeSmartCollection.set(null);
        try {
            if (folder === null) {
                const imgs = await listImages(100000, 0);
                images.set(imgs);
            } else {
                const imgs = await listImagesByFolder(folder, 100000, 0);
                images.set(imgs);
            }
            focusedIndex.set(0);
        } catch (e) {
            console.error('Failed to load images for folder:', e);
        }
    }

    async function selectCollection(collectionId: string) {
        activeSession.set(null);
        sessionCanvases.set([]);
        activeCollection.set(collectionId);
        activeFolder.set(null);
        activeSmartCollection.set(null);
        try {
            const imgs = await listCollectionImages(collectionId);
            images.set(imgs);
            focusedIndex.set(0);
        } catch (e) {
            console.error('Failed to load collection images:', e);
        }
    }

    async function handleNewCollection() {
        const name = window.prompt('Collection name:');
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
                const imgs = await listImages(100000, 0);
                images.set(imgs);
                focusedIndex.set(0);
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
        if (!window.confirm(`Delete folder "${name}" and its unique images from library?`)) return;
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

    // AI Models state
    let aiExpanded = $state(true);
    let yoloReady = $state(false);
    let nudenetReady = $state(false);
    let yoloDownloading = $state(false);
    let nudenetDownloading = $state(false);
    let yoloDownloadPct = $state(0);
    let nudenetDownloadPct = $state(0);
    let yoloProcessed = $state(0);
    let nudenetProcessed = $state(0);
    let selectedYoloVariant = $state('medium');
    let detectedClasses = $state<[string, number][]>([]);
    let detectingBatch = $state(false);
    let ollamaModels = $state<string[]>([]);
    let ollamaReady = $derived(ollamaModels.length > 0);
    let visionProcessed = $state(0);
    let analyzingBatch = $state(false);

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
            const allImgs = await listImages(100000, 0);
            const allIds = allImgs.map(i => i.image.id);
            await analyzeImages(allIds);
            await loadAiState();
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
                const matches = await searchByDetectedClass(cls, 1);
                if (matches.length > 0) {
                    const count = await searchByDetectedClass(cls, 100000);
                    results.push([cls, count.length]);
                }
            } catch (_) {}
        }
        results.sort((a, b) => b[1] - a[1]);
        detectedClasses = results;
    }

    async function handleDownloadYolo() {
        yoloDownloading = true;
        yoloDownloadPct = 0;
        const unlisten = await listen<{ downloaded: number; total: number }>('yolo-download-progress', (e) => {
            if (e.payload.total > 0) yoloDownloadPct = Math.round((e.payload.downloaded / e.payload.total) * 100);
        });
        try {
            await downloadYoloModel(selectedYoloVariant);
            yoloReady = true;
        } catch (e) {
            console.error('YOLO download error:', e);
        } finally {
            unlisten();
            yoloDownloading = false;
        }
    }

    async function handleDownloadNudenet() {
        nudenetDownloading = true;
        nudenetDownloadPct = 0;
        const unlisten = await listen<{ downloaded: number; total: number }>('nudenet-download-progress', (e) => {
            if (e.payload.total > 0) nudenetDownloadPct = Math.round((e.payload.downloaded / e.payload.total) * 100);
        });
        try {
            await downloadNudenetModel();
            nudenetReady = true;
        } catch (e) {
            console.error('NudeNet download error:', e);
        } finally {
            unlisten();
            nudenetDownloading = false;
        }
    }

    async function handleDetectRemaining() {
        if (detectingBatch) return;
        detectingBatch = true;
        try {
            const allImgs = await listImages(100000, 0);
            const allIds = allImgs.map(i => i.image.id);
            if (yoloReady) await detectObjects(allIds, selectedYoloVariant);
            if (nudenetReady) await detectNsfw(allIds);
            await loadAiState();
        } catch (e) {
            console.error('Batch detection error:', e);
        } finally {
            detectingBatch = false;
        }
    }

    async function filterByClass(className: string) {
        try {
            const matches = await searchByDetectedClass(className, 100000);
            const ids = matches.map(m => m[0]);
            if (ids.length > 0) {
                const imgs = await getImagesByIds(ids);
                images.set(imgs);
                focusedIndex.set(0);
                activeSession.set(null);
                sessionCanvases.set([]);
                activeFolder.set(null);
                activeCollection.set(null);
            }
        } catch (e) {
            console.error('Filter by class error:', e);
        }
    }

    async function refreshImages() {
        const count = await getImageCount();
        totalCount.set(count);
        const currentFolder = get(activeFolder);
        if (currentFolder === null) {
            const imgs = await listImages(100000, 0);
            images.set(imgs);
        } else {
            const imgs = await listImagesByFolder(currentFolder, 100000, 0);
            images.set(imgs);
        }
        focusedIndex.set(0);
        // Refresh folders too
        try {
            const f = await listFolders();
            folders.set(f);
        } catch (_) {}
    }
</script>

<div class="sidebar">
    <SessionSwitcher />

    {#if $activeSession}
        <div class="section">
            <div class="section-header">
                <span>CANVASES</span>
                <button class="section-action" onclick={async () => {
                    if ($activeSession) {
                        const canvas = await createCanvas($activeSession.id, 'New Canvas', 'manual');
                        sessionCanvases.update(c => [...c, canvas]);
                    }
                }}>+</button>
            </div>
            {#each $sessionCanvases as canvas}
                <button class="section-item">
                    <span>{canvas.name}</span>
                    <span class="count">{canvas.canvas_type}</span>
                </button>
            {/each}
        </div>
    {/if}

    <div class="section">
        <div class="section-header">LIBRARY</div>
        <button class="section-item" class:active={$activeFolder === null && $activeCollection === null && $activeSmartCollection === null} onclick={() => selectFolder(null)}>
            <span class="icon">&#9632;</span>
            All Images
            <span class="count">({$totalCount})</span>
        </button>

        {#if displayFolders.length > 0}
            <button class="folders-toggle" onclick={() => foldersExpanded = !foldersExpanded}>
                <span class="toggle-arrow">{foldersExpanded ? '▾' : '▸'}</span>
                <span class="folders-toggle-label">Folders</span>
                <span class="count">({displayFolders.length})</span>
            </button>

            {#if foldersExpanded}
                <div role="tree" aria-label="Folder hierarchy">
                {#each displayFolders as folder}
                    <div class="folder-row" class:active={$activeFolder === folder.fullPath} style="padding-left: {folder.depth * 12}px" role="treeitem" aria-level={folder.depth + 1} {...(folder.hasChildren && folder.count === 0 ? { 'aria-expanded': 'true' } : {})}>
                        {#if folder.count > 0}
                            <button class="section-item" onclick={() => selectFolder(folder.fullPath)} title={folder.fullPath}>
                                <span class="icon">{folder.hasChildren ? '▾' : '▸'}</span>
                                <span class="folder-label">{folder.name}</span>
                                <span class="count">({folder.count})</span>
                            </button>
                            <button class="delete-btn" onclick={(e: Event) => handleDeleteFolder(e, folder.fullPath)} title="Remove folder">&times;</button>
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
        <button class="folders-toggle" onclick={() => aiExpanded = !aiExpanded}>
            <span class="toggle-arrow">{aiExpanded ? '▾' : '▸'}</span>
            <span class="folders-toggle-label">AI MODELS</span>
        </button>

        {#if aiExpanded}
            <div class="ai-models-content">
                <div class="model-row">
                    <span class="model-name">YOLO</span>
                    {#if yoloDownloading}
                        <span class="model-status downloading">
                            {yoloDownloadPct}%
                        </span>
                    {:else if yoloReady}
                        <span class="model-status ready">ready</span>
                    {:else}
                        <span class="model-status missing">missing</span>
                    {/if}
                </div>

                {#if yoloDownloading}
                    <div class="progress-bar">
                        <div class="progress-fill" style="width: {yoloDownloadPct}%"></div>
                    </div>
                {/if}

                {#if !yoloReady && !yoloDownloading}
                    <div class="model-download-row">
                        <select class="variant-select" bind:value={selectedYoloVariant}>
                            <option value="nano">nano 6MB</option>
                            <option value="small">small 22MB</option>
                            <option value="medium">medium 50MB</option>
                        </select>
                        <button class="download-btn" onclick={handleDownloadYolo}>dl</button>
                    </div>
                {/if}

                <div class="model-row">
                    <span class="model-name">NudeNet</span>
                    {#if nudenetDownloading}
                        <span class="model-status downloading">
                            {nudenetDownloadPct}%
                        </span>
                    {:else if nudenetReady}
                        <span class="model-status ready">ready</span>
                    {:else}
                        <span class="model-status missing">missing</span>
                    {/if}
                </div>

                {#if nudenetDownloading}
                    <div class="progress-bar">
                        <div class="progress-fill" style="width: {nudenetDownloadPct}%"></div>
                    </div>
                {/if}

                {#if !nudenetReady && !nudenetDownloading}
                    <button class="download-btn full-width" onclick={handleDownloadNudenet}>download 12MB</button>
                {/if}

                <div class="model-row">
                    <span class="model-name">Ollama</span>
                    {#if ollamaReady}
                        <span class="model-status ready">{ollamaModels.length} models</span>
                    {:else}
                        <span class="model-status missing">offline</span>
                    {/if}
                </div>

                {#if yoloReady || nudenetReady}
                    <div class="processed-row">
                        <span class="processed-label">Detection</span>
                        <span class="processed-count">{yoloProcessed}/{$totalCount}</span>
                    </div>
                    {#if yoloProcessed < $totalCount}
                        <button class="detect-btn" onclick={handleDetectRemaining} disabled={detectingBatch}>
                            {detectingBatch ? 'detecting...' : `> detect remaining ${$totalCount - yoloProcessed}`}
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
                            {analyzingBatch ? 'analyzing...' : `> analyze remaining ${$totalCount - visionProcessed}`}
                        </button>
                    {/if}
                {/if}

                {#if detectedClasses.length > 0}
                    <div class="detected-header">DETECTED</div>
                    {#each detectedClasses as [cls, count]}
                        <button class="section-item detected-class" onclick={() => filterByClass(cls)}>
                            <span class="class-tag">{cls}</span>
                            <span class="count">{count}</span>
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
                onclick={() => selectSmartCollection(sc)}>
                <span class="icon">&#9733;</span>
                {sc.name}
                <span class="count">({sc.image_count ?? 0})</span>
            </button>
        {/each}
    </div>
    {/if}

    <div class="section">
        <div class="section-header">
            COLLECTIONS
            <button class="new-collection-btn" onclick={handleNewCollection} title="New Collection">+</button>
        </div>
        {#if $pinnedCollection}
            {@const pinnedName = $collections.find(([id]) => id === $pinnedCollection)?.[1] ?? 'Unknown'}
            <div class="pinned-indicator">
                <span class="pin-icon">📌</span>
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
                    <button class="section-item" onclick={() => selectCollection(id)}>
                        <span class="icon">&#9671;</span>
                        {name}
                        <span class="count">({count})</span>
                    </button>
                    <button
                        class="pin-btn"
                        class:active={$pinnedCollection === id}
                        onclick={(e: Event) => { e.stopPropagation(); $pinnedCollection === id ? unpinCollection() : pinCollection(id); }}
                        title={$pinnedCollection === id ? 'Unpin' : 'Pin as active'}
                    >
                        {$pinnedCollection === id ? '📌' : '📎'}
                    </button>
                    <button class="delete-btn" onclick={(e: Event) => handleDeleteCollection(e, id, name)} title="Delete collection">&times;</button>
                </div>
            {/each}
        {/if}
    </div>

    <div class="sidebar-footer" aria-live="polite">
        {#if lastResult}
            <div class="import-result">{lastResult}</div>
        {/if}
        <button class="import-btn" onclick={handleImportFolder} disabled={importing || regenerating}>
            {importing ? (importTotal > 0 ? `Importing ${importCurrent}/${importTotal}...` : 'Scanning...') : '+ Import Folder'}
        </button>
        <button class="import-btn secondary" onclick={handleRegenerateThumbnails} disabled={importing || regenerating}>
            {regenerating ? `Thumbnails ${regenProgress.current}/${regenProgress.total}...` : 'Regenerate Thumbnails'}
        </button>
        <button class="import-btn secondary" onclick={handleRescan} disabled={importing || regenerating || rescanning}>
            {rescanning ? 'Scanning sources...' : 'Rescan Sources'}
        </button>
    </div>
</div>

<style>
    .sidebar {
        width: 220px;
        background: var(--surface);
        border-right: 1px solid var(--border);
        display: flex;
        flex-direction: column;
        grid-area: sidebar;
        overflow-y: auto;
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
    }
    .section-item:hover {
        background: var(--border);
    }
    .section-item.active {
        background: rgba(122, 162, 247, 0.1);
        color: var(--blue);
    }
    .icon {
        font-size: 8px;
    }
    .count {
        color: var(--text-secondary);
        margin-left: auto;
        font-size: 11px;
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
        background: rgba(122, 162, 247, 0.1);
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
        display: none;
        margin-right: 4px;
        font-size: 14px;
        line-height: 1;
        color: var(--text-secondary);
        cursor: pointer;
        flex-shrink: 0;
        background: none;
        border: none;
        padding: 6px 6px;
        font-family: inherit;
    }
    .folder-row:hover .delete-btn {
        display: inline;
    }
    .delete-btn:hover {
        color: var(--red, #f7768e);
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
    }
    .folders-toggle:hover {
        color: var(--text-primary, #cdd6f4);
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
        background: rgba(122, 162, 247, 0.15);
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
        color: var(--text-primary, #cdd6f4);
    }
    .show-missing-toggle input {
        accent-color: var(--blue);
    }
    .new-collection-btn {
        margin-left: auto;
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 14px;
        font-weight: 700;
        padding: 0 2px;
        line-height: 1;
        font-family: inherit;
    }
    .new-collection-btn:hover {
        color: var(--blue);
    }
    .collect-indicator {
        font-size: 10px;
        color: var(--green, #9ece6a);
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
    }
    .import-result {
        font-size: 10px;
        color: var(--green);
        margin-bottom: 6px;
        word-break: break-word;
    }
    .import-btn {
        width: 100%;
        background: rgba(122, 162, 247, 0.15);
        color: var(--blue);
        border: 1px solid var(--border);
        font-family: var(--font);
        font-size: 12px;
        padding: 6px 12px;
        border-radius: var(--radius);
        cursor: pointer;
        transition: all 0.15s;
    }
    .import-btn:hover:not(:disabled) {
        background: rgba(122, 162, 247, 0.25);
        border-color: var(--blue);
    }
    .import-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .import-btn.secondary {
        background: rgba(122, 162, 247, 0.08);
        font-size: 10px;
        padding: 4px 8px;
        margin-top: 4px;
    }
    /* AI Models section */
    .ai-models-content {
        padding: 0 8px;
    }
    .model-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 3px 0;
        font-size: 11px;
    }
    .model-name {
        color: var(--text-primary, #e0e0e0);
        font-weight: 600;
    }
    .model-status {
        font-size: 10px;
    }
    .model-status.ready {
        color: var(--green, #9ece6a);
    }
    .model-status.missing {
        color: var(--text-secondary, #565f89);
    }
    .model-status.downloading {
        color: var(--orange, #e0af68);
    }
    .progress-bar {
        height: 3px;
        background: var(--border, #1a1a2e);
        border-radius: 2px;
        margin: 2px 0 4px;
        overflow: hidden;
    }
    .progress-fill {
        height: 100%;
        background: var(--blue, #7aa2f7);
        transition: width 0.3s;
    }
    .model-download-row {
        display: flex;
        gap: 4px;
        margin: 2px 0 4px;
    }
    .variant-select {
        flex: 1;
        font-size: 10px;
        padding: 2px 4px;
        background: var(--bg, #08080c);
        color: var(--text-primary, #e0e0e0);
        border: 1px solid var(--border, #1a1a2e);
        border-radius: var(--radius, 4px);
        font-family: inherit;
    }
    .download-btn {
        font-size: 10px;
        padding: 2px 6px;
        background: rgba(122, 162, 247, 0.15);
        color: var(--blue, #7aa2f7);
        border: 1px solid var(--border, #1a1a2e);
        border-radius: var(--radius, 4px);
        cursor: pointer;
        font-family: inherit;
    }
    .download-btn:hover {
        background: rgba(122, 162, 247, 0.25);
        border-color: var(--blue, #7aa2f7);
    }
    .download-btn.full-width {
        width: 100%;
        margin: 2px 0 4px;
    }
    .processed-row {
        display: flex;
        justify-content: space-between;
        font-size: 10px;
        color: var(--text-secondary, #565f89);
        padding: 4px 0 2px;
    }
    .processed-label {
        color: var(--text-secondary, #565f89);
    }
    .processed-count {
        color: var(--text-primary, #e0e0e0);
    }
    .detect-btn {
        width: 100%;
        font-size: 10px;
        padding: 3px 6px;
        background: none;
        color: var(--blue, #7aa2f7);
        border: none;
        cursor: pointer;
        font-family: inherit;
        text-align: left;
    }
    .detect-btn:hover:not(:disabled) {
        color: var(--text-primary, #e0e0e0);
    }
    .detect-btn:disabled {
        color: var(--text-secondary, #565f89);
        cursor: not-allowed;
    }
    .detected-header {
        font-size: 9px;
        font-weight: 700;
        color: var(--text-secondary, #565f89);
        letter-spacing: 0.1em;
        padding: 6px 0 2px;
    }
    .detected-class {
        padding: 2px 0;
    }
    .class-tag {
        color: var(--purple, #bb9af7);
    }
    .pinned-indicator {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 6px 12px;
        margin: 4px 8px;
        background: var(--bg-elevated, #2a2a3e);
        border-radius: 6px;
        border: 1px solid var(--accent, #8cc63f);
        font-size: 12px;
    }
    .pin-icon { font-size: 14px; }
    .pin-name { color: var(--text-primary, #eee); flex: 1; }
    .pin-action {
        background: none;
        border: none;
        color: var(--text-secondary, #888);
        cursor: pointer;
        font-size: 11px;
        font-family: inherit;
    }
    .pin-action:hover { color: var(--text-primary, #eee); }
    .pin-btn {
        background: none;
        border: none;
        cursor: pointer;
        font-size: 11px;
        opacity: 0.4;
        padding: 4px 6px;
    }
    .pin-btn:hover, .pin-btn.active { opacity: 1; }
</style>
