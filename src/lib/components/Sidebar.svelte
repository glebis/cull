<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { open } from '@tauri-apps/plugin-dialog';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { totalCount, folders, activeFolder, minSizeFilter, collections, activeCollection, activeDetectedClass, detectedClasses as detectedClassesStore, collectMode, collectModeTarget, smartCollections, activeSmartCollection, showToast, pinnedCollection, pinnedCollections, showMissing, requestTextInput, requestConfirm, clipboardMonitorStatus, exportFolderOpen } from '$lib/stores';
    import { importFolder as apiImportFolder, getImageCount, listFolders, deleteFolder as apiDeleteFolder, listCollections, createCollection, renameCollectionApi, deleteCollectionApi, listCollectionImages, listSmartCollections, countByDetectedClass, regenerateThumbnails, rescanSources, getClipboardMonitorStatus, startClipboardMonitor, stopClipboardMonitor, setClipboardMonitorCaptureExistingOnStart, moveClipboardCaptureFolder, publishClipboardCollection } from '$lib/api';
    import { loadImagesForCurrentScope } from '$lib/image-loading';
    import type { ClipboardMonitorStatus, ClipboardPublishResult, ImageWithFile, SmartCollection } from '$lib/api';
    import { applyClipboardMonitorCollection } from '$lib/clipboard-monitor';
    import { safeAssetPreviewPath } from '$lib/view-utils';
    import { onDestroy, onMount } from 'svelte';
    import { get } from 'svelte/store';

    let importing = $state(false);
    let importCurrent = $state(0);
    let importTotal = $state(0);
    let lastResult = $state('');
    let lastResultKind = $state<'success' | 'error'>('success');

    function setLastResult(text: string, kind: 'success' | 'error' = 'success') {
        lastResult = text;
        lastResultKind = kind;
    }
    let regenerating = $state(false);
    let regenProgress = $state({ current: 0, total: 0 });
    let rescanning = $state(false);
    let foldersExpanded = $state(true);
    let clipboardStatus = $state<ClipboardMonitorStatus | null>(null);
    let clipboardMoving = $state(false);
    let clipboardPublishing = $state(false);
    let clipboardPublishResult = $state<ClipboardPublishResult | null>(null);
    let collectionPreview = $state<{
        collectionId: string;
        name: string;
        count: number;
        images: ImageWithFile[];
        loading: boolean;
        x: number;
        y: number;
    } | null>(null);
    let collectionContextMenu = $state<{
        collectionId: string;
        name: string;
        count: number;
        x: number;
        y: number;
    } | null>(null);
    let collectionPreviewTimer: ReturnType<typeof setTimeout> | null = null;
    let collectionPreviewRequest = 0;

    function setClipboardStatus(status: ClipboardMonitorStatus | null) {
        clipboardStatus = status;
        clipboardMonitorStatus.set(status);
    }

    import { buildDisplayFolders, buildPinnedCollectionRows, formatSidebarCount } from '$lib/sidebar-utils';
    import SessionSwitcher from './SessionSwitcher.svelte';
    import { activeCanvas, activeSession, navigateTo, sessionCanvases } from '$lib/stores';
    import { createCanvas, type Canvas } from '$lib/api';

    let displayFolders = $derived(buildDisplayFolders($folders));
    let displayCollections = $derived(buildPinnedCollectionRows($collections, $pinnedCollections));

    function clearCollectionPreviewTimer() {
        if (!collectionPreviewTimer) return;
        clearTimeout(collectionPreviewTimer);
        collectionPreviewTimer = null;
    }

    function collectionPreviewSrc(item: ImageWithFile): string {
        const path = safeAssetPreviewPath(item, { displayPx: 76, dpr: typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1 });
        return path ? convertFileSrc(path) : '';
    }

    function scheduleCollectionPreview(event: MouseEvent | FocusEvent, collectionId: string, name: string, count: number) {
        clearCollectionPreviewTimer();
        collectionPreviewRequest += 1;
        if (count <= 0) {
            collectionPreview = null;
            return;
        }

        const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
        const x = rect.right + 8;
        const y = Math.max(8, Math.min(rect.top, window.innerHeight - 172));
        const requestId = collectionPreviewRequest;

        collectionPreviewTimer = setTimeout(async () => {
            collectionPreview = { collectionId, name, count, images: [], loading: true, x, y };
            try {
                const images = await listCollectionImages(collectionId, 4, 0);
                if (requestId !== collectionPreviewRequest) return;
                collectionPreview = { collectionId, name, count, images, loading: false, x, y };
            } catch (e) {
                if (requestId !== collectionPreviewRequest) return;
                collectionPreview = null;
                console.error('Failed to load collection preview:', e);
            }
        }, 1000);
    }

    function hideCollectionPreview(collectionId?: string) {
        clearCollectionPreviewTimer();
        collectionPreviewRequest += 1;
        if (!collectionId || collectionPreview?.collectionId === collectionId) {
            collectionPreview = null;
        }
    }

    function openCollectionContextMenu(event: MouseEvent, collectionId: string, name: string, count: number) {
        event.preventDefault();
        event.stopPropagation();
        hideCollectionPreview(collectionId);
        collectionContextMenu = {
            collectionId,
            name,
            count,
            x: Math.min(event.clientX, window.innerWidth - 208),
            y: Math.min(event.clientY, window.innerHeight - 224),
        };
    }

    function closeCollectionContextMenu() {
        collectionContextMenu = null;
    }

    onDestroy(() => {
        clearCollectionPreviewTimer();
        window.removeEventListener('detected-classes-changed', handleDetectedClassesChanged);
    });

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
        window.addEventListener('detected-classes-changed', handleDetectedClassesChanged);
        loadDetectedClasses().catch(e => console.error('Failed to load detected classes:', e));
    });

    function folderName(path: string): string {
        const parts = path.split('/');
        return parts[parts.length - 1] || path;
    }

    function pinCollection(collectionId: string) {
        pinnedCollections.update(ids => ids.includes(collectionId) ? ids : [...ids, collectionId]);
        pinnedCollection.set(collectionId);
        showToast('Collection pinned', { detail: 'New imports will be added here', type: 'info', duration: 5000 });
    }

    function unpinCollection(collectionId: string) {
        let nextIds: string[] = [];
        pinnedCollections.update(ids => {
            nextIds = ids.filter(id => id !== collectionId);
            return nextIds;
        });
        if (get(pinnedCollection) === collectionId) {
            pinnedCollection.set(nextIds[nextIds.length - 1] ?? null);
        }
        showToast('Collection unpinned', { type: 'info', duration: 3000 });
    }

    function togglePinnedCollection(collectionId: string) {
        if (get(pinnedCollections).includes(collectionId)) {
            unpinCollection(collectionId);
        } else {
            pinCollection(collectionId);
        }
    }

    async function handleRenameCollection(collectionId: string, currentName: string) {
        closeCollectionContextMenu();
        const name = await requestTextInput({
            title: 'Rename Collection',
            label: 'Collection name',
            initialValue: currentName,
            placeholder: 'Collection name',
            confirmLabel: 'Rename',
        });
        if (!name || !name.trim() || name.trim() === currentName) return;
        try {
            await renameCollectionApi(collectionId, name.trim());
            collections.set(await listCollections());
            showToast('Collection renamed', { type: 'success', duration: 3000 });
        } catch (e) {
            console.error('Failed to rename collection:', e);
            showToast('Failed to rename collection', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleExportCollection(collectionId: string) {
        closeCollectionContextMenu();
        await selectCollection(collectionId);
        exportFolderOpen.set(true);
    }

    async function copyCollectionId(collectionId: string) {
        closeCollectionContextMenu();
        try {
            await navigator.clipboard.writeText(collectionId);
            showToast('Collection ID copied', { type: 'success', duration: 2500 });
        } catch (e) {
            showToast('Copy failed', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    function setCollectTarget(collectionId: string, name: string) {
        closeCollectionContextMenu();
        collectMode.set(true);
        collectModeTarget.set(collectionId);
        showToast('Collect mode enabled', { detail: name, type: 'info', duration: 5000 });
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
            showToast('Failed to create collection', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleDeleteCollection(event: Event, collectionId: string, collectionName: string) {
        event.stopPropagation();
        closeCollectionContextMenu();
        const confirmed = await requestConfirm({
            title: 'Delete Collection',
            description: `Delete collection "${collectionName}"? Images stay in the library.`,
            confirmLabel: 'Delete',
            danger: true,
        });
        if (!confirmed) return;
        try {
            await deleteCollectionApi(collectionId);
            pinnedCollections.update(ids => ids.filter(id => id !== collectionId));
            if (get(pinnedCollection) === collectionId) {
                const nextPinned = get(pinnedCollections);
                pinnedCollection.set(nextPinned[nextPinned.length - 1] ?? null);
            }
            if (get(activeCollection) === collectionId) {
                activeCollection.set(null);
                activeDetectedClass.set(null);
                await loadImagesForCurrentScope({ force: true, invalidateCache: true });
            }
            const c = await listCollections();
            collections.set(c);
        } catch (e) {
            console.error('Failed to delete collection:', e);
            showToast('Failed to delete collection', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleDeleteFolder(event: Event, folder: string) {
        event.stopPropagation();
        const name = folderName(folder);
        const confirmed = await requestConfirm({
            title: 'Remove Folder from Library',
            description: `Remove "${name}" from the library? Cull records for images that only exist in this folder will be removed. Original files stay on disk.`,
            confirmLabel: 'Remove Folder',
            danger: true,
        });
        if (!confirmed) return;
        try {
            const count = await apiDeleteFolder(folder);
            setLastResult(`Removed ${count} images from "${name}"`);
            if (get(activeFolder) === folder) {
                activeFolder.set(null);
            }
            await refreshImages();
        } catch (e) {
            setLastResult(`Error: ${e}`, 'error');
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

    async function copyPublishUrl() {
        if (!clipboardPublishResult) return;
        try {
            await navigator.clipboard.writeText(clipboardPublishResult.url);
            showToast('Link copied', { detail: clipboardPublishResult.url, type: 'success', duration: 4000 });
        } catch (e) {
            showToast('Copy failed', { detail: String(e), type: 'error', duration: 8000 });
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
            setLastResult(`Detected sources for ${count} images`);
            await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        } catch (e) {
            setLastResult(`Rescan error: ${e}`, 'error');
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
            setLastResult(`Regenerated ${count} thumbnails`);
        } catch (e) {
            setLastResult(`Thumbnail error: ${e}`, 'error');
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
        setLastResult('');

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
            let summary = `+${result.imported} imported, ${result.skipped} skipped`;
            if (result.errors.length > 0) {
                summary += `, ${result.errors.length} errors`;
            }
            setLastResult(summary, result.errors.length > 0 ? 'error' : 'success');
            showToast(`Imported "${folderName}"`, {
                detail: summary,
                type: 'success',
                duration: 8000,
            });
            await refreshImages();
        } catch (e) {
            setLastResult(`Error: ${e}`, 'error');
            showToast('Import failed', { detail: String(e), type: 'error', duration: 10000 });
        } finally {
            unlisten();
            importing = false;
        }
    }

    let detectedClasses = $state<[string, number][]>([]);

    function handleDetectedClassesChanged() { void loadDetectedClasses(); }

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

<svelte:window
    onclick={closeCollectionContextMenu}
    onkeydown={(e) => { if (e.key === 'Escape') { closeCollectionContextMenu(); hideCollectionPreview(); } }}
/>

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
                <div aria-label="Folder hierarchy">
                {#each displayFolders as folder}
                    <div class="folder-row" class:active={$activeFolder === folder.fullPath} style="padding-left: {folder.depth * 12}px">
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
        <div class="section-header">
            COLLECTIONS
            <button class="new-collection-btn" onclick={handleNewCollection} title="New Collection" aria-label="New collection">+</button>
        </div>
        {#if $collectMode && $collectModeTarget}
            <div class="collect-indicator">Collecting into: {$collections.find(c => c[0] === $collectModeTarget)?.[1] ?? '...'}</div>
        {/if}
        {#if $collections.length === 0}
            <div class="section-empty">No collections yet</div>
        {:else}
            {#each displayCollections as [id, name, count]}
                {@const pinned = $pinnedCollections.includes(id)}
                <div
                    class="folder-row collection-row"
                    class:active={$activeCollection === id}
                    class:pinned
                    onmouseenter={(e) => scheduleCollectionPreview(e, id, name, count)}
                    onmouseleave={() => hideCollectionPreview(id)}
                    onfocusin={(e) => scheduleCollectionPreview(e, id, name, count)}
                    onfocusout={() => hideCollectionPreview(id)}
                    oncontextmenu={(e) => openCollectionContextMenu(e, id, name, count)}
                    role="group"
                    aria-label={`Collection actions: ${name}`}
                >
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
                        class:active={pinned}
                        onclick={(e: Event) => { e.stopPropagation(); togglePinnedCollection(id); }}
                        title={pinned ? 'Unpin collection' : 'Pin collection'}
                        aria-label={pinned ? `Unpin collection: ${name}` : `Pin collection: ${name}`}
                        aria-pressed={pinned}
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
            <div class="section-meta">Access: {clipboardStatus.access_status}</div>
            <div class="section-meta" title={clipboardStatus.capture_dir}>
                Folder: {clipboardStatus.capture_dir.split('/').pop() || clipboardStatus.capture_dir}
            </div>
            {#if clipboardStatus.collection_name}
                <div class="section-meta">Collection: {clipboardStatus.collection_name} · {clipboardStatus.captured_count}</div>
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
                <button
                    class="publish-url"
                    onclick={copyPublishUrl}
                    title={`Copy link: ${clipboardPublishResult.url}`}
                >{clipboardPublishResult.url}</button>
            {/if}
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
        {#if detectedClasses.length > 0}
            <div class="detected-header">DETECTED OBJECTS</div>
            {#each detectedClasses as [cls, count]}
                <button class="section-item detected-class" class:active={$activeDetectedClass === cls} onclick={() => filterByClass(cls)}>
                    <span class="class-tag">{cls}</span>
                    <span class="count">{formatSidebarCount(count)}</span>
                </button>
            {/each}
        {/if}
    </div>
    </div>

    {#if collectionPreview}
        <div
            class="collection-preview-popover"
            style="left: {collectionPreview.x}px; top: {collectionPreview.y}px;"
            aria-hidden="true"
        >
            <div class="collection-preview-header">
                <span>{collectionPreview.name}</span>
                <span>{formatSidebarCount(collectionPreview.count)}</span>
            </div>
            {#if collectionPreview.loading}
                <div class="collection-preview-loading">Loading...</div>
            {:else if collectionPreview.images.length > 0}
                <div class="collection-preview-grid">
                    {#each collectionPreview.images as item}
                        {@const src = collectionPreviewSrc(item)}
                        <div class="collection-preview-thumb">
                            {#if src}
                                <img src={src} alt="" loading="lazy" />
                            {/if}
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    {/if}

    {#if collectionContextMenu}
        <div
            class="collection-context-menu"
            style="left: {collectionContextMenu.x}px; top: {collectionContextMenu.y}px;"
            role="menu"
            tabindex="-1"
        >
            <div class="context-menu-header">{collectionContextMenu.name}</div>
            <button type="button" role="menuitem" onclick={() => { selectCollection(collectionContextMenu!.collectionId); closeCollectionContextMenu(); }}>Open Collection</button>
            <button type="button" role="menuitem" onclick={() => handleRenameCollection(collectionContextMenu!.collectionId, collectionContextMenu!.name)}>Rename...</button>
            <button type="button" role="menuitem" onclick={() => handleExportCollection(collectionContextMenu!.collectionId)} disabled={collectionContextMenu.count === 0}>Export to Folder...</button>
            <button type="button" role="menuitem" onclick={() => setCollectTarget(collectionContextMenu!.collectionId, collectionContextMenu!.name)}>Use for Collect Mode</button>
            <button type="button" role="menuitem" onclick={() => togglePinnedCollection(collectionContextMenu!.collectionId)}>
                {$pinnedCollections.includes(collectionContextMenu.collectionId) ? 'Unpin Collection' : 'Pin Collection'}
            </button>
            <button type="button" role="menuitem" onclick={() => copyCollectionId(collectionContextMenu!.collectionId)}>Copy Collection ID</button>
            <button type="button" role="menuitem" class="danger" onclick={(e) => handleDeleteCollection(e, collectionContextMenu!.collectionId, collectionContextMenu!.name)}>Delete Collection...</button>
        </div>
    {/if}

    <div class="sidebar-footer" aria-live="polite" aria-busy={importing || regenerating || rescanning}>
        {#if lastResult}
            <div class="import-result" class:error={lastResultKind === 'error'}>{lastResult}</div>
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
                    aria-label={regenerating ? `Regenerating thumbnails ${regenProgress.current} of ${regenProgress.total}` : 'Rebuild thumbnails'}
                >
                    {regenerating ? `${regenProgress.current}/${regenProgress.total}` : 'Rebuild thumbnails'}
                </button>
                <button
                    class="import-btn secondary"
                    onclick={handleRescan}
                    disabled={importing || regenerating || rescanning}
                    aria-label={rescanning ? 'Rescanning sources' : 'Rescan sources'}
                >
                    {rescanning ? 'Scanning' : 'Rescan sources'}
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
    .publish-url {
        background: none;
        border: none;
        color: var(--blue);
        cursor: pointer;
        display: block;
        font-family: inherit;
        font-size: 10px;
        max-width: 100%;
        overflow: hidden;
        padding: 2px 8px;
        text-align: left;
        text-decoration: underline;
        text-overflow: ellipsis;
        white-space: nowrap;
        width: 100%;
    }
    .publish-url:hover {
        color: var(--text);
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
        flex-wrap: wrap;
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
    .import-result.error {
        color: var(--red);
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
        padding: 2px 6px;
        white-space: normal;
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
    .collection-row.pinned .section-item {
        color: var(--text);
    }
    .pin-btn {
        align-items: center;
        background: none;
        border: none;
        color: var(--text);
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
        color: var(--text);
    }
    .pin-btn.active {
        color: var(--text);
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
    .collection-preview-popover {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 12px 32px color-mix(in srgb, var(--bg) 80%, transparent);
        padding: 8px;
        position: fixed;
        width: 176px;
        z-index: var(--z-context-menu);
    }
    .collection-preview-header {
        align-items: center;
        color: var(--text-secondary);
        display: flex;
        font-size: 10px;
        gap: 8px;
        justify-content: space-between;
        margin-bottom: 6px;
        min-width: 0;
    }
    .collection-preview-header span:first-child {
        color: var(--text);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .collection-preview-loading {
        color: var(--text-secondary);
        font-size: 10px;
        min-height: 72px;
        padding-top: 28px;
        text-align: center;
    }
    .collection-preview-grid {
        display: grid;
        gap: 4px;
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .collection-preview-thumb {
        aspect-ratio: 1;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        overflow: hidden;
    }
    .collection-preview-thumb img {
        display: block;
        height: 100%;
        object-fit: cover;
        width: 100%;
    }
    .collection-context-menu {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 12px 32px color-mix(in srgb, var(--bg) 80%, transparent);
        display: grid;
        min-width: 200px;
        padding: 4px;
        position: fixed;
        z-index: var(--z-context-menu);
    }
    .context-menu-header {
        color: var(--text-secondary);
        font-size: 10px;
        overflow: hidden;
        padding: 6px 8px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .collection-context-menu button {
        background: none;
        border: none;
        border-radius: var(--radius);
        color: var(--text);
        cursor: pointer;
        font-family: inherit;
        font-size: 12px;
        padding: 6px 8px;
        text-align: left;
    }
    .collection-context-menu button:hover:not(:disabled),
    .collection-context-menu button:focus-visible {
        background: var(--border);
    }
    .collection-context-menu button:disabled {
        color: var(--text-secondary);
        cursor: default;
    }
    .collection-context-menu button.danger {
        color: var(--red);
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
