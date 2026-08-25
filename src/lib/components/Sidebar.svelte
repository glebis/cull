<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { open } from '@tauri-apps/plugin-dialog';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { totalCount, folders, activeFolder, minSizeFilter, collections, activeCollection, activeDetectedClass, detectedClasses as detectedClassesStore, collectMode, collectModeTarget, smartCollections, activeSmartCollection, showToast, pinnedCollection, pinnedCollections, showMissing, showRejected, sidebarHideEmpty, requestTextInput, requestConfirm, clipboardMonitorStatus, exportFolderOpen, exportFolderSmartCollection } from '$lib/stores';
    import { importFolder as apiImportFolder, getImageCount, listFolders, deleteFolder as apiDeleteFolder, renameFolder as apiRenameFolder, listCollections, createCollection, createCollectionWithImages, renameCollectionApi, deleteCollectionApi, listCollectionImages, listSmartCollections, updateSmartCollectionApi, deleteSmartCollectionApi, countByDetectedClass, listDetectedClasses, getClipboardMonitorStatus, startClipboardMonitor, stopClipboardMonitor, setClipboardMonitorCaptureExistingOnStart, moveClipboardCaptureFolder, publishClipboardCollection } from '$lib/api';
    import { loadImagesForCurrentScope } from '$lib/image-loading';
    import type { ClipboardMonitorStatus, ClipboardPublishResult, FilterNode, ImageWithFile, SmartCollection } from '$lib/api';
    import { applyClipboardMonitorCollection } from '$lib/clipboard-monitor';
    import { safeAssetPreviewPath } from '$lib/view-utils';
    import { revealItemInDir } from '@tauri-apps/plugin-opener';
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
    type SidebarContextTarget =
        | { kind: 'folder'; folder: string; name: string; renamable: boolean; removable: boolean; x: number; y: number; opener: HTMLElement | null }
        | { kind: 'collection'; collectionId: string; name: string; count: number; x: number; y: number; opener: HTMLElement | null }
        | { kind: 'smart'; collection: SmartCollection; x: number; y: number; opener: HTMLElement | null }
        | { kind: 'canvas'; canvas: Canvas; x: number; y: number; opener: HTMLElement | null };
    let sidebarContextMenu = $state<SidebarContextTarget | null>(null);
    let smartCollectionEditor = $state<SmartCollection | null>(null);
    let smartCollectionDraft = $state<FilterNode | null>(null);
    let collectionPreviewTimer: ReturnType<typeof setTimeout> | null = null;
    let collectionPreviewRequest = 0;
    let browseCountsRequest = 0;

    function setClipboardStatus(status: ClipboardMonitorStatus | null) {
        clipboardStatus = status;
        clipboardMonitorStatus.set(status);
    }

    import { buildDisplayFolders, buildPinnedCollectionRows, formatSidebarCount, formatFolderCount, visibleFolderRows, matchesSidebarFilter, prunePinnedIds, recordRecentScope, markRecentScopeVisited, pruneRecentScopes, ancestorFolderPaths, kindLabel, type CollectionRow, type RecentScope } from '$lib/sidebar-utils';

    function prunePinsToExistingCollections(rows: CollectionRow[]) {
        const kept = prunePinnedIds(get(pinnedCollections), rows);
        if (kept.length !== get(pinnedCollections).length) {
            pinnedCollections.set(kept);
        }
        if (get(pinnedCollection) && !kept.includes(get(pinnedCollection)!)) {
            pinnedCollection.set(kept[kept.length - 1] ?? null);
        }
    }
    import SessionSwitcher from './SessionSwitcher.svelte';
    import { activeCanvas, activeSession, navigateTo, sessions, sessionCanvases, expandedFolders, sidebarSectionsCollapsed, sidebarFilter, pendingGridSearch, recentScopes } from '$lib/stores';
    import { createCanvas, deleteCanvas, addToCollection, listImagesByFolder, type Canvas } from '$lib/api';
    import { reconcileRenamedCanvas, reconcileRenamedSession, renamedFolderPath } from '$lib/folder-rename-state';
    import { withCanvasPathMigrationBarrier } from '$lib/canvas-save-coordinator';
    import ActionMenu from './ActionMenu.svelte';
    import ModalDialog from './ModalDialog.svelte';
    import RuleBuilder from './RuleBuilder.svelte';
    import { buildCanvasContextActions, buildCollectionContextActions, buildFolderContextActions, buildSmartCollectionContextActions } from '$lib/sidebar-context-actions';

    // "All Images" is only the active scope when nothing else narrows it —
    // including a detected-class filter, which used to leave both All Images
    // and the class row looking unselected/selected at the same time.
    let allImagesActive = $derived(
        $activeFolder === null &&
        $activeCollection === null &&
        $activeSmartCollection === null &&
        $activeDetectedClass === null
    );

    let displayFolders = $derived(buildDisplayFolders($folders));
    let displayCollections = $derived(
        buildPinnedCollectionRows($collections, $pinnedCollections)
            .filter(([, name]) => matchesSidebarFilter(name, $sidebarFilter))
            // imageview-1i2k.3: hide-empty drops zero-count collections.
            .filter(([, , count]) => !$sidebarHideEmpty || count > 0)
    );

    // Section collapse. The store holds the COLLAPSED ids, so a section added
    // later defaults to open for users who already have persisted state.
    function isSectionCollapsed(collapsed: Set<string>, id: string): boolean {
        return collapsed.has(id);
    }
    function toggleSection(id: string) {
        sidebarSectionsCollapsed.update(set => {
            const next = new Set(set);
            if (next.has(id)) next.delete(id); else next.add(id);
            return next;
        });
    }

    // "Recent Imports" is a seeded smart collection, but sixth in a 22-row list
    // is not where you look after an import. Promote it next to All Images and
    // drop it from the SMART list so it appears exactly once.
    const RECENT_IMPORTS_NAME = 'Recent Imports';
    let recentImportsCollection = $derived(
        $smartCollections.find(sc => sc.name === RECENT_IMPORTS_NAME && (sc.image_count ?? 0) > 0) ?? null
    );

    // The rows actually on screen. Both the render loop and the keyboard
    // handler read this, so arrow keys can never focus a hidden row.
    // imageview-1i2k.3: hide-empty drops folders whose whole subtree is empty
    // (subtreeCount covers descendants, so a kept parent never strands one).
    let visibleFolders = $derived(
        visibleFolderRows(displayFolders, $expandedFolders, $sidebarFilter)
            .filter(f => !$sidebarHideEmpty || f.subtreeCount > 0)
    );
    let treeFocusIndex = $state(0);
    // While filtering, the tree auto-reveals matches and their subtrees, so
    // expansion controls are inert and say so rather than looking broken.
    let filterActive = $derived($sidebarFilter.trim() !== '');
    // Filtering or collapsing can shrink the list below the remembered index.
    // Clamping in a $derived (rather than writing back to treeFocusIndex from
    // an effect) keeps exactly one row tabbable without a self-referential
    // effect, so Tab can always re-enter the tree.
    let treeTabIndex = $derived(
        visibleFolders.length === 0 ? -1 : Math.min(treeFocusIndex, visibleFolders.length - 1)
    );

    function toggleFolderExpanded(path: string) {
        // While a filter is active the tree ignores expansion entirely, so a
        // toggle here would silently rewrite persisted state the user cannot
        // see — they would only discover it after clearing the filter.
        if (get(sidebarFilter).trim()) return;
        expandedFolders.update(set => {
            const next = new Set(set);
            if (next.has(path)) next.delete(path); else next.add(path);
            return next;
        });
    }

    function focusTreeRow(index: number) {
        const clamped = Math.max(0, Math.min(index, visibleFolders.length - 1));
        treeFocusIndex = clamped;
        // The row must exist in the DOM before it can take focus; Svelte has
        // already flushed by the time a keydown handler runs for rows that were
        // visible, and expand/collapse re-renders synchronously via $derived.
        queueMicrotask(() => {
            const el = document.querySelector<HTMLElement>(`[data-tree-row="${clamped}"]`);
            el?.focus();
        });
    }

    function handleTreeKeydown(event: KeyboardEvent) {
        const rows = visibleFolders;
        if (rows.length === 0) return;
        const i = Math.min(treeFocusIndex, rows.length - 1);
        const row = rows[i];

        if (isContextMenuKey(event)) {
            const treeItem = event.target instanceof HTMLElement
                ? event.target.closest<HTMLElement>('[data-tree-row]')
                : null;
            openFolderContextMenu(
                event,
                row.fullPath,
                row.name,
                row.fullPath !== '/',
                !row.isGroup && row.fullPath !== '/',
                treeItem,
            );
            return;
        }

        switch (event.key) {
            case 'Enter':
            case ' ':
                // Focus lives on the treeitem, not the inner button, so the
                // row has to activate itself.
                event.preventDefault();
                selectFolder(row.fullPath);
                break;
            case 'ArrowDown':
                event.preventDefault();
                focusTreeRow(i + 1);
                break;
            case 'ArrowUp':
                event.preventDefault();
                focusTreeRow(i - 1);
                break;
            case 'Home':
                event.preventDefault();
                focusTreeRow(0);
                break;
            case 'End':
                event.preventDefault();
                focusTreeRow(rows.length - 1);
                break;
            case 'ArrowRight':
                event.preventDefault();
                // Standard tree behaviour: open a closed node, then step into it.
                // Filtering already reveals every subtree, so there is nothing
                // to open and Right simply steps in.
                if (row.hasChildren && !filterActive && !get(expandedFolders).has(row.fullPath)) {
                    toggleFolderExpanded(row.fullPath);
                } else if (row.hasChildren) {
                    focusTreeRow(i + 1);
                }
                break;
            case 'ArrowLeft':
                event.preventDefault();
                if (row.hasChildren && !filterActive && get(expandedFolders).has(row.fullPath)) {
                    toggleFolderExpanded(row.fullPath);
                } else {
                    // Jump to the nearest shallower row — the visual parent.
                    for (let k = i - 1; k >= 0; k--) {
                        if (rows[k].depth < row.depth) { focusTreeRow(k); break; }
                    }
                }
                break;
            case 'Delete':
                if (!row.isGroup) {
                    event.preventDefault();
                    void handleDeleteFolder(row.fullPath);
                }
                break;
        }
    }

    // A running monitor forces the section open — a background capture that the
    // user cannot see the state of is worse than the space it costs.
    let clipboardCollapsed = $derived(
        isSectionCollapsed($sidebarSectionsCollapsed, 'clipboard') && !clipboardStatus?.running
    );

    // The RECENT rail: rows re-resolve their names from the live lists at
    // render (renames follow automatically), and dead targets are pruned when
    // browse counts refresh.
    let recentScopeRows = $derived(
        $recentScopes.map(s => {
            if (s.kind === 'folder') return { ...s, name: folderName(s.id) };
            if (s.kind === 'collection') {
                return { ...s, name: $collections.find(([id]) => id === s.id)?.[1] ?? s.name };
            }
            return { ...s, name: $smartCollections.find(sc => sc.id === s.id)?.name ?? s.name };
        })
    );

    async function openRecentScope(scopeRow: RecentScope) {
        if (scopeRow.kind === 'folder') { await selectFolder(scopeRow.id); return; }
        if (scopeRow.kind === 'collection') { await selectCollection(scopeRow.id); return; }
        const sc = get(smartCollections).find(item => item.id === scopeRow.id);
        if (sc) await selectSmartCollection(sc);
    }

    // The tree is alphabetical, so a fresh import lands wherever its name
    // falls — expand its ancestor rows and scroll it into view instead of
    // trusting persistence-as-punishment.
    function revealImportedFolderInTree(path: string) {
        const rows = buildDisplayFolders(get(folders));
        const nextExpanded = new Set([...get(expandedFolders), ...ancestorFolderPaths(rows, path)]);
        expandedFolders.set(nextExpanded);
        foldersExpanded = true;
        const idx = visibleFolderRows(rows, nextExpanded, get(sidebarFilter)).findIndex(r => r.fullPath === path);
        if (idx < 0) return;
        treeFocusIndex = idx;
        queueMicrotask(() => {
            const reduce = typeof window !== 'undefined'
                && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
            const el = document.querySelector<HTMLElement>(`[data-tree-row="${idx}"]`);
            el?.scrollIntoView({ block: 'nearest', behavior: reduce ? 'auto' : 'smooth' });
        });
    }

    // Smart collections seed ~22 presets. Hide empty built-ins to keep the list
    // proportional to the library, but retain user collections so their rules
    // can still be edited when they currently match nothing.
    let visibleSmartCollections = $derived(
        $smartCollections.filter(sc =>
            ((sc.image_count ?? 0) > 0 || !sc.is_preset) &&
            sc.id !== recentImportsCollection?.id &&
            matchesSidebarFilter(sc.name, $sidebarFilter)
        )
    );

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
                const images = await listCollectionImages(collectionId, 4, 0, $showRejected);
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

    function contextPoint(event: MouseEvent | KeyboardEvent, anchor?: HTMLElement | null): { x: number; y: number } {
        if (event instanceof MouseEvent && event.type === 'contextmenu') {
            return { x: event.clientX, y: event.clientY };
        }
        const target = anchor ?? event.currentTarget as HTMLElement | null;
        const rect = target?.getBoundingClientRect();
        return rect
            ? { x: rect.left + Math.min(32, rect.width / 2), y: rect.top + Math.min(24, rect.height) }
            : { x: 16, y: 16 };
    }

    function isContextMenuKey(event: KeyboardEvent): boolean {
        return event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10');
    }

    function contextOpener(event: MouseEvent | KeyboardEvent, anchor?: HTMLElement | null): HTMLElement | null {
        if (anchor) return anchor;
        if (event.target instanceof HTMLElement) {
            const interactive = event.target.closest<HTMLElement>('button, [href], input, select, textarea, [tabindex]');
            if (interactive) return interactive;
        }
        return event.currentTarget as HTMLElement | null;
    }

    function openCollectionContextMenu(event: MouseEvent | KeyboardEvent, collectionId: string, name: string, count: number) {
        event.preventDefault();
        event.stopPropagation();
        hideCollectionPreview(collectionId);
        sidebarContextMenu = {
            kind: 'collection', collectionId, name, count,
            ...contextPoint(event),
            opener: contextOpener(event),
        };
    }

    function openFolderContextMenu(event: MouseEvent | KeyboardEvent, folder: string, name: string, renamable: boolean, removable: boolean, anchor?: HTMLElement | null) {
        event.preventDefault();
        event.stopPropagation();
        sidebarContextMenu = {
            kind: 'folder', folder, name, renamable, removable,
            ...contextPoint(event, anchor),
            opener: contextOpener(event, anchor),
        };
    }

    function openSmartCollectionContextMenu(event: MouseEvent | KeyboardEvent, collection: SmartCollection) {
        event.preventDefault();
        event.stopPropagation();
        sidebarContextMenu = {
            kind: 'smart', collection,
            ...contextPoint(event),
            opener: contextOpener(event),
        };
    }

    function openCanvasContextMenu(event: MouseEvent | KeyboardEvent, canvas: Canvas) {
        event.preventDefault();
        event.stopPropagation();
        sidebarContextMenu = {
            kind: 'canvas', canvas,
            ...contextPoint(event),
            opener: contextOpener(event),
        };
    }

    function closeSidebarContextMenu() {
        sidebarContextMenu = null;
    }

    onDestroy(() => {
        clearCollectionPreviewTimer();
        window.removeEventListener('detected-classes-changed', handleDetectedClassesChanged);
        window.removeEventListener('cull:decision-changed', handleDecisionChanged);
    });

    onMount(async () => {
        try {
            setClipboardStatus(await getClipboardMonitorStatus());
        } catch (e) {
            console.error('Failed to load clipboard monitor status:', e);
        }
        try {
            await listen('clipboard-monitor:capture', async () => {
                setClipboardStatus(await getClipboardMonitorStatus());
                const c = await listCollections($showRejected);
                collections.set(c);
                if (clipboardStatus?.collection_id && get(activeCollection) === clipboardStatus.collection_id) {
                    await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
                }
            });
        } catch (e) {
            console.error('Failed to listen for clipboard monitor captures:', e);
        }
        window.addEventListener('detected-classes-changed', handleDetectedClassesChanged);
        window.addEventListener('cull:decision-changed', handleDecisionChanged);
    });

    async function refreshBrowseCounts(includeRejected: boolean) {
        const request = ++browseCountsRequest;
        try {
            const [count, nextFolders, nextCollections, nextSmartCollections, nextDetectedClasses] = await Promise.all([
                getImageCount(includeRejected),
                listFolders(includeRejected),
                listCollections(includeRejected),
                listSmartCollections(includeRejected),
                listDetectedClasses(includeRejected),
            ]);
            if (request !== browseCountsRequest) return;
            totalCount.set(count);
            folders.set(nextFolders);
            collections.set(nextCollections);
            prunePinsToExistingCollections(nextCollections);
            smartCollections.set(nextSmartCollections);
            recentScopes.set(pruneRecentScopes(
                get(recentScopes),
                new Set(nextFolders.map(([p]) => p)),
                new Set(nextCollections.map(([id]) => id)),
                new Set(nextSmartCollections.map(sc => sc.id)),
            ));
            detectedClasses = nextDetectedClasses;
            detectedClassesStore.set(nextDetectedClasses);
        } catch (e) {
            if (request === browseCountsRequest) {
                console.error('Failed to refresh library counts:', e);
                showToast('Failed to refresh library counts', {
                    detail: String(e),
                    type: 'error',
                    duration: 8000,
                });
            }
        }
    }

    function handleDecisionChanged() { void refreshBrowseCounts($showRejected); }

    $effect(() => { void refreshBrowseCounts($showRejected); });

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
        closeSidebarContextMenu();
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
            collections.set(await listCollections($showRejected));
            showToast('Collection renamed', { type: 'success', duration: 3000 });
        } catch (e) {
            console.error('Failed to rename collection:', e);
            showToast('Failed to rename collection', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleExportCollection(collectionId: string) {
        closeSidebarContextMenu();
        await selectCollection(collectionId);
        exportFolderOpen.set(true);
    }

    async function listAllCollectionImageIds(collectionId: string): Promise<string[]> {
        const pageSize = 500;
        const ids: string[] = [];
        for (let offset = 0; ; offset += pageSize) {
            const page = await listCollectionImages(collectionId, pageSize, offset, true);
            ids.push(...page.map(item => item.image.id));
            if (page.length < pageSize) break;
        }
        return [...new Set(ids)];
    }

    async function duplicateCollection(collectionId: string, currentName: string) {
        const name = await requestTextInput({
            title: 'Duplicate Collection',
            label: 'New collection name',
            initialValue: `${currentName} Copy`,
            confirmLabel: 'Duplicate',
        });
        if (!name?.trim()) return;
        let ids: string[];
        try {
            ids = await listAllCollectionImageIds(collectionId);
            await createCollectionWithImages(name.trim(), ids);
        } catch (e) {
            showToast('Could not duplicate collection', { detail: String(e), type: 'error', duration: 10000 });
            return;
        }
        showToast(`Duplicated “${currentName}”`, {
            detail: `${ids.length} image${ids.length === 1 ? '' : 's'}`,
            type: 'success',
        });
        try {
            collections.set(await listCollections($showRejected));
        } catch (e) {
            showToast('Collection duplicated, but the sidebar could not refresh', {
                detail: String(e), type: 'warning', duration: 10000,
            });
        }
    }

    async function publishCollection(collectionId: string) {
        try {
            const result = await publishClipboardCollection(collectionId);
            clipboardPublishResult = result;
            try {
                await navigator.clipboard.writeText(result.url);
                showToast('Collection published; link copied', { detail: result.url, type: 'success', duration: 10000 });
            } catch (e) {
                showToast('Collection published', { detail: `${result.url} · Copy failed: ${e}`, type: 'warning', duration: 10000 });
            }
        } catch (e) {
            showToast('Could not publish collection', { detail: String(e), type: 'error', duration: 10000 });
        }
    }

    async function copyCollectionId(collectionId: string) {
        closeSidebarContextMenu();
        try {
            await navigator.clipboard.writeText(collectionId);
            showToast('Collection ID copied', { type: 'success', duration: 2500 });
        } catch (e) {
            showToast('Copy failed', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    function setCollectTarget(collectionId: string, name: string) {
        closeSidebarContextMenu();
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
        recentScopes.update(list => recordRecentScope(list, { kind: 'smart', id: sc.id, name: sc.name, ts: Date.now() }));
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
        if (folder) {
            recentScopes.update(list => recordRecentScope(list, { kind: 'folder', id: folder, name: folderName(folder), ts: Date.now() }));
        }
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
        const name = get(collections).find(([id]) => id === collectionId)?.[1] ?? 'Collection';
        recentScopes.update(list => recordRecentScope(list, { kind: 'collection', id: collectionId, name, ts: Date.now() }));
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
            const c = await listCollections($showRejected);
            collections.set(c);
        } catch (e) {
            console.error('Failed to create collection:', e);
            showToast('Failed to create collection', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleDeleteCollection(collectionId: string, collectionName: string) {
        closeSidebarContextMenu();
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
            const c = await listCollections($showRejected);
            collections.set(c);
        } catch (e) {
            console.error('Failed to delete collection:', e);
            showToast('Failed to delete collection', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleDeleteFolder(folder: string) {
        closeSidebarContextMenu();
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

    async function handleRenameFolder(folder: string, currentName: string) {
        closeSidebarContextMenu();
        const name = await requestTextInput({
            title: 'Rename Folder',
            label: 'Folder name',
            initialValue: currentName,
            placeholder: 'Folder name',
            confirmLabel: 'Rename',
        });
        if (!name || !name.trim() || name.trim() === currentName) return;
        try {
            const result = await withCanvasPathMigrationBarrier(async () => {
                const result = await apiRenameFolder(folder, name.trim());
                activeFolder.update(path => path ? renamedFolderPath(path, result.oldPath, result.newPath) : null);
                expandedFolders.update(paths => new Set(
                    [...paths].map(path => renamedFolderPath(path, result.oldPath, result.newPath))
                ));
                sessions.update(items => items.map(session => reconcileRenamedSession(session, result.oldPath, result.newPath)));
                activeSession.update(session => session ? reconcileRenamedSession(session, result.oldPath, result.newPath) : null);
                sessionCanvases.update(canvases => canvases.map(canvas => reconcileRenamedCanvas(canvas, result.oldPath, result.newPath)));
                activeCanvas.update(canvas => canvas ? reconcileRenamedCanvas(canvas, result.oldPath, result.newPath) : null);
                return result;
            });
            await refreshBrowseCounts($showRejected);
            if (get(activeFolder)) {
                await loadImagesForCurrentScope({ force: true, invalidateCache: true });
            }
            showToast('Folder renamed', {
                detail: `${result.oldPath} → ${result.newPath}`,
                type: 'success',
                duration: 5000,
            });
        } catch (e) {
            console.error('Failed to rename folder:', e);
            showToast('Failed to rename folder', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function revealFolder(folder: string) {
        try {
            await revealItemInDir(folder);
        } catch (e) {
            showToast('Could not reveal folder in Finder', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function rescanFolder(folder: string) {
        if (importing) return;
        importing = true;
        importCurrent = 0;
        importTotal = 0;
        try {
            const result = await apiImportFolder(folder);
            const summary = `Rescanned “${folderName(folder)}”: ${result.imported} imported, ${result.skipped} unchanged`;
            setLastResult(result.errors.length > 0 ? `${summary}, ${result.errors.length} errors` : summary, result.errors.length > 0 ? 'error' : 'success');
            await refreshImages();
        } catch (e) {
            setLastResult(`Rescan failed: ${e}`, 'error');
        } finally {
            importing = false;
        }
    }

    async function beginEditSmartCollection(id: string) {
        const collection = get(smartCollections).find(item => item.id === id);
        if (!collection?.filter_json || collection.is_preset) return;
        try {
            smartCollectionDraft = JSON.parse(collection.filter_json) as FilterNode;
            smartCollectionEditor = collection;
        } catch (e) {
            showToast('Smart collection rules could not be opened', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    function closeSmartCollectionEditor() {
        smartCollectionEditor = null;
        smartCollectionDraft = null;
    }

    async function saveSmartCollectionRules() {
        const collection = smartCollectionEditor;
        const draft = smartCollectionDraft;
        if (!collection || !draft) return;
        try {
            await updateSmartCollectionApi(
                collection.id,
                collection.name,
                JSON.stringify(draft),
                collection.nl_query ?? undefined,
            );
        } catch (e) {
            showToast('Could not update smart collection', { detail: String(e), type: 'error', duration: 10000 });
            return;
        }
        closeSmartCollectionEditor();
        try {
            const updated = await listSmartCollections($showRejected);
            smartCollections.set(updated);
            const nextActive = updated.find(item => item.id === collection.id) ?? null;
            if (get(activeSmartCollection)?.id === collection.id && nextActive) {
                activeSmartCollection.set(nextActive);
                await loadImagesForCurrentScope({ force: true, invalidateCache: true });
            }
            showToast('Smart collection rules updated', { type: 'success' });
        } catch (e) {
            showToast('Smart collection updated, but the view could not refresh', {
                detail: String(e), type: 'warning', duration: 10000,
            });
        }
    }

    async function deleteSmartCollection(id: string, name: string) {
        const confirmed = await requestConfirm({
            title: 'Delete Smart Collection',
            description: `Delete smart collection “${name}”? Images stay in the library.`,
            confirmLabel: 'Delete',
            danger: true,
        });
        if (!confirmed) return;
        try {
            await deleteSmartCollectionApi(id);
        } catch (e) {
            showToast('Could not delete smart collection', { detail: String(e), type: 'error', duration: 10000 });
            return;
        }
        const refreshErrors: string[] = [];
        if (get(activeSmartCollection)?.id === id) {
            activeSmartCollection.set(null);
            try {
                await loadImagesForCurrentScope({ force: true, invalidateCache: true });
            } catch (e) {
                refreshErrors.push(String(e));
            }
        }
        try {
            smartCollections.set(await listSmartCollections($showRejected));
        } catch (e) {
            refreshErrors.push(String(e));
        }
        if (refreshErrors.length === 0) {
            showToast('Smart collection deleted', { type: 'success' });
        } else {
            showToast('Smart collection deleted, but the view could not refresh', {
                detail: refreshErrors.join(' · '), type: 'warning', duration: 10000,
            });
        }
    }

    async function handleToggleClipboardMonitor() {
        const wasRunning = clipboardStatus?.running ?? false;
        try {
            const nextStatus = wasRunning
                ? await stopClipboardMonitor()
                : await startClipboardMonitor(null);
            setClipboardStatus(nextStatus);
            const c = await listCollections($showRejected);
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

    async function handleImportFolder() {
        const selected = await open({ directory: true, multiple: false });
        if (!selected) return;

        importing = true;
        const progressId = crypto.randomUUID();
        importCurrent = 0;
        importTotal = 0;
        setLastResult('');

        // Listen for progress events
        let lastRefresh = 0;
        const unlisten: UnlistenFn = await listen<{ progress_id?: string; current: number; total: number; filename: string }>(
            'import-progress',
            async (event) => {
                if (event.payload.progress_id !== progressId) return;
                importCurrent = event.payload.current;
                importTotal = event.payload.total;

                // Refresh image count every 20 imports
                if (importCurrent - lastRefresh >= 20) {
                    lastRefresh = importCurrent;
                    const count = await getImageCount($showRejected);
                    totalCount.set(count);
                }
            }
        );

        try {
            const result = await apiImportFolder(selected as string, null, progressId);
            const folderName = (selected as string).split('/').filter(Boolean).pop() ?? selected;
            let summary = `+${result.imported} imported, ${result.skipped} skipped`;
            if (result.errors.length > 0) {
                summary += `, ${result.errors.length} errors`;
            }
            if (result.cancelled) {
                setLastResult(`Cancelled: ${summary}`);
                showToast('Import cancelled', { detail: summary, type: 'warning', duration: 8000 });
                await refreshImages();
                return;
            }
            setLastResult(summary, result.errors.length > 0 ? 'error' : 'success');
            const importedFolder = selected as string;
            recentScopes.update(list => recordRecentScope(list, {
                kind: 'folder',
                id: importedFolder,
                // the local const above already holds the basename
                name: folderName,
                ts: Date.now(),
                fresh: true,
            }));
            showToast(`Imported "${folderName}"`, {
                detail: summary,
                type: 'success',
                duration: 8000,
                // "Where did what I just imported go?" is the question every
                // import ends on; answer it in the toast instead of making the
                // user hunt for the folder in the tree. The RECENT rail above
                // keeps the same answer after the toast expires.
                actions: result.imported > 0
                    ? [{ label: 'View imported', onclick: () => { selectFolder(importedFolder); } }]
                    : undefined,
            });
            await refreshImages();
            revealImportedFolderInTree(importedFolder);
        } catch (e) {
            setLastResult(`Error: ${e}`, 'error');
            showToast('Import failed', { detail: String(e), type: 'error', duration: 10000 });
        } finally {
            unlisten();
            importing = false;
        }
    }

    let detectedClasses = $state<[string, number][]>([]);

    // imageview-1i2k.2: the filter is the sidebar's one search — it narrows
    // every scope list, including the ones it used to skip silently.
    // Canvases: only the active session's are loaded (listCanvases is
    // per-session), so matches cover that set.
    let visibleDetectedClasses = $derived(
        detectedClasses.filter(([cls]) => matchesSidebarFilter(cls, $sidebarFilter))
    );
    let matchingCanvases = $derived(
        filterActive ? $sessionCanvases.filter(c => matchesSidebarFilter(c.name, $sidebarFilter)) : []
    );

    // Enter promotes the scope filter to a grid search: the same text runs
    // through the CommandBar NL parser, which applies it to the grid.
    function promoteFilterToGrid() {
        const query = get(sidebarFilter).trim();
        if (!query) return;
        pendingGridSearch.set(query);
        navigateTo('grid');
    }

    function handleDetectedClassesChanged() { void loadDetectedClasses(); }

    async function loadDetectedClasses(includeRejected = $showRejected) {
        try {
            const next = await listDetectedClasses(includeRejected);
            if (includeRejected !== $showRejected) return;
            detectedClasses = next;
            detectedClassesStore.set(next);
        } catch (_) {}
    }

    async function filterByClass(className: string) {
        try {
            const count = await countByDetectedClass(className, $showRejected);
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
        const count = await getImageCount($showRejected);
        totalCount.set(count);
        await loadImagesForCurrentScope({ force: true, invalidateCache: true });
        // Refresh folders too
        try {
            const f = await listFolders($showRejected);
            folders.set(f);
        } catch (_) {}
    }

    async function copyFolderPath(folder: string) {
        try {
            await navigator.clipboard.writeText(folder);
            showToast('Folder path copied', { type: 'success', duration: 2500 });
        } catch (e) {
            showToast('Copy failed', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function listAllFolderImageIds(folder: string): Promise<string[]> {
        const pageSize = 500;
        const ids: string[] = [];
        for (let offset = 0; ; offset += pageSize) {
            const page = await listImagesByFolder(folder, pageSize, offset, $showRejected);
            ids.push(...page.map(item => item.image.id));
            if (page.length < pageSize) break;
        }
        return [...new Set(ids)];
    }

    async function addFolderToCollection(folder: string, collectionId: string) {
        let ids: string[];
        try {
            ids = await listAllFolderImageIds(folder);
            if (ids.length === 0) {
                showToast('Folder contains no images to add', { type: 'info', duration: 3500 });
                return;
            }
            await addToCollection(collectionId, ids);
            const collectionName = get(collections).find(([id]) => id === collectionId)?.[1] ?? 'collection';
            showToast(`Added ${ids.length} image${ids.length === 1 ? '' : 's'} to ${collectionName}`, { type: 'success' });
        } catch (e) {
            showToast('Could not add folder to collection', { detail: String(e), type: 'error', duration: 10000 });
            return;
        }
        try {
            collections.set(await listCollections($showRejected));
        } catch (e) {
            showToast('Images added, but the sidebar could not refresh', {
                detail: String(e), type: 'warning', duration: 10000,
            });
        }
    }

    async function createCollectionFromFolder(folder: string) {
        const name = await requestTextInput({
            title: 'New Collection from Folder',
            label: 'Collection name',
            initialValue: folderName(folder),
            confirmLabel: 'Create and Add',
        });
        if (!name?.trim()) return;
        let ids: string[];
        try {
            ids = await listAllFolderImageIds(folder);
            if (ids.length > 0) {
                await createCollectionWithImages(name.trim(), ids);
            } else {
                await createCollection(name.trim());
            }
            showToast(`Created collection “${name.trim()}”`, {
                detail: `${ids.length} image${ids.length === 1 ? '' : 's'} added`,
                type: 'success',
            });
        } catch (e) {
            showToast('Could not create collection from folder', { detail: String(e), type: 'error', duration: 10000 });
            return;
        }
        try {
            collections.set(await listCollections($showRejected));
        } catch (e) {
            showToast('Collection created, but the sidebar could not refresh', {
                detail: String(e), type: 'warning', duration: 10000,
            });
        }
    }

    async function exportSmartCollection(id: string) {
        const collection = get(smartCollections).find(item => item.id === id);
        if (!collection) return;
        exportFolderSmartCollection.set(collection);
        exportFolderOpen.set(true);
    }

    async function deleteCanvasFromSidebar(canvasId: string, name: string) {
        const confirmed = await requestConfirm({
            title: 'Delete Canvas',
            description: `Delete canvas “${name}”? Images and files stay in the session.`,
            confirmLabel: 'Delete Canvas',
            danger: true,
        });
        if (!confirmed) return;
        try {
            await deleteCanvas(canvasId);
            sessionCanvases.update(items => items.filter(item => item.id !== canvasId));
            if (get(activeCanvas)?.id === canvasId) {
                activeCanvas.set(null);
            }
            showToast('Canvas deleted', { type: 'success' });
        } catch (e) {
            showToast('Could not delete canvas', { detail: String(e), type: 'error', duration: 10000 });
        }
    }

    let sidebarContextItems = $derived.by(() => {
        const target = sidebarContextMenu;
        if (!target) return [];
        if (target.kind === 'canvas') {
            return buildCanvasContextActions({
                canvasId: target.canvas.id,
                name: target.canvas.name,
                onOpen: (id) => {
                    const canvas = get(sessionCanvases).find(item => item.id === id);
                    if (canvas) selectCanvas(canvas);
                },
                onDelete: deleteCanvasFromSidebar,
            });
        }
        if (target.kind === 'folder') {
            return buildFolderContextActions({
                folder: target.folder,
                name: folderName(target.folder),
                renamable: target.renamable,
                removable: target.removable,
                collections: $collections,
                onOpen: selectFolder,
                onReveal: revealFolder,
                onRename: handleRenameFolder,
                onRescan: rescanFolder,
                onAddToCollection: addFolderToCollection,
                onCreateCollection: createCollectionFromFolder,
                onCopyPath: copyFolderPath,
                onRemove: handleDeleteFolder,
            });
        }
        if (target.kind === 'collection') {
            return buildCollectionContextActions({
                collectionId: target.collectionId,
                name: target.name,
                count: target.count,
                pinned: $pinnedCollections.includes(target.collectionId),
                onOpen: selectCollection,
                onRename: handleRenameCollection,
                onDuplicate: duplicateCollection,
                onExport: handleExportCollection,
                onPublish: publishCollection,
                onCollect: setCollectTarget,
                onTogglePin: togglePinnedCollection,
                onCopyId: copyCollectionId,
                onDelete: handleDeleteCollection,
            });
        }
        const collection = target.collection;
        return buildSmartCollectionContextActions({
            id: collection.id,
            name: collection.name,
            count: collection.image_count ?? 0,
            isPreset: collection.is_preset,
            onOpen: async (id) => {
                const next = get(smartCollections).find(item => item.id === id);
                if (next) await selectSmartCollection(next);
            },
            onEdit: beginEditSmartCollection,
            onExport: exportSmartCollection,
            onDelete: deleteSmartCollection,
        });
    });
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
                <div
                    class="folder-row canvas-row"
                    class:active={$activeCanvas?.id === canvas.id}
                    oncontextmenu={(event) => openCanvasContextMenu(event, canvas)}
                    role="group"
                    aria-label={`Canvas actions: ${canvas.name}`}
                >
                    <button
                        class="section-item"
                        onclick={() => selectCanvas(canvas)}
                        onkeydown={(event) => { if (isContextMenuKey(event)) openCanvasContextMenu(event, canvas); }}
                        aria-current={$activeCanvas?.id === canvas.id ? 'true' : undefined}
                    >
                        <span class="item-label">{canvas.name}</span>
                        <span class="count">{canvas.canvas_type}</span>
                    </button>
                    <button
                        class="menu-btn"
                        onclick={(event) => openCanvasContextMenu(event, canvas)}
                        title="Canvas actions"
                        aria-label={`Canvas actions: ${canvas.name}`}
                        aria-haspopup="menu"
                    >…</button>
                </div>
            {/each}
        </div>
    {/if}

    <div class="sidebar-filter">
        <input
            type="search"
            class="sidebar-filter-input"
            placeholder="Filter scopes — Enter searches images"
            aria-label="Filter sidebar scopes; press Enter to search images"
            bind:value={$sidebarFilter}
            onkeydown={(e: KeyboardEvent) => {
                if (e.key === 'Escape') { e.stopPropagation(); sidebarFilter.set(''); }
                // imageview-1i2k.2: one adaptive search — Enter hands the
                // scope query to the grid (CommandBar NL parser).
                if (e.key === 'Enter') promoteFilterToGrid();
            }}
        />
    </div>

    {#if matchingCanvases.length > 0}
    <div class="section">
        <div class="section-header">CANVASES</div>
        {#each matchingCanvases as canvas (canvas.id)}
            <button
                class="section-item"
                class:active={$activeCanvas?.id === canvas.id}
                onclick={() => selectCanvas(canvas)}
            >
                <span class="item-label">{canvas.name}</span>
                <span class="scope-kind">Canvas</span>
            </button>
        {/each}
    </div>
    {/if}

    {#if recentScopeRows.length > 0}
    <div class="section recent-rail">
        <div class="section-header">RECENT</div>
        {#each recentScopeRows as scopeRow (scopeRow.kind + ':' + scopeRow.id)}
            <button
                class="section-item recent-scope"
                class:fresh={scopeRow.fresh}
                onclick={() => openRecentScope(scopeRow)}
                title={scopeRow.kind === 'folder' ? scopeRow.id : scopeRow.name}
                aria-label={scopeRow.fresh
                    ? `${scopeRow.name}, ${kindLabel(scopeRow.kind)}, just imported`
                    : `${scopeRow.name}, ${kindLabel(scopeRow.kind)}`}
            >
                <span class="item-label">{scopeRow.name}</span>
                {#if scopeRow.fresh}
                    <span class="just-imported">new</span>
                {:else}
                    <span class="scope-kind">{kindLabel(scopeRow.kind)}</span>
                {/if}
            </button>
        {/each}
    </div>
    {/if}

    <div class="section">
        <div class="section-header">LIBRARY</div>
        <button
            class="section-item"
            class:active={allImagesActive}
            onclick={() => selectFolder(null)}
            aria-current={allImagesActive ? 'true' : undefined}
        >
            <span class="item-label">All Images</span>
            <span class="count">{formatSidebarCount($totalCount)}</span>
        </button>

        {#if recentImportsCollection}
            <button
                class="section-item"
                class:active={$activeSmartCollection?.id === recentImportsCollection.id}
                onclick={() => selectSmartCollection(recentImportsCollection!)}
                aria-current={$activeSmartCollection?.id === recentImportsCollection.id ? 'true' : undefined}
                title="Images imported in the last 7 days"
            >
                <span class="item-label">Recent Imports</span>
                <span class="count">{formatSidebarCount(recentImportsCollection.image_count)}</span>
            </button>
        {/if}

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
                <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
                <div
                    class="folder-tree"
                    role="tree"
                    tabindex="-1"
                    aria-label="Folder hierarchy"
                    onkeydown={handleTreeKeydown}
                >
                {#each visibleFolders as folder, i (folder.fullPath)}
                    {@const isExpanded = $expandedFolders.has(folder.fullPath)}
                    <div
                        class="folder-row"
                        class:active={$activeFolder === folder.fullPath}
                        style="padding-left: {folder.depth * 12}px"
                        role="treeitem"
                        aria-level={folder.depth + 1}
                        aria-keyshortcuts={folder.isGroup ? undefined : 'Delete'}
                        aria-selected={$activeFolder === folder.fullPath}
                        aria-expanded={folder.hasChildren ? (isExpanded || filterActive) : undefined}
                        aria-label={`${folder.name}, ${folder.subtreeCount} images`}
                        data-tree-row={i}
                        tabindex={i === treeTabIndex ? 0 : -1}
                        onfocusin={() => treeFocusIndex = i}
                        oncontextmenu={(e) => openFolderContextMenu(e, folder.fullPath, folder.name, folder.fullPath !== '/', !folder.isGroup && folder.fullPath !== '/')}
                    >
                        {#if folder.hasChildren}
                            <button
                                class="twisty"
                                tabindex="-1"
                                disabled={filterActive}
                                onclick={(e: Event) => { e.stopPropagation(); toggleFolderExpanded(folder.fullPath); }}
                                aria-label={`${isExpanded || filterActive ? 'Collapse' : 'Expand'} ${folder.name}`}
                                title={filterActive ? 'Expansion is disabled while filtering' : undefined}
                            >{isExpanded || filterActive ? '▾' : '▸'}</button>
                        {:else}
                            <span class="twisty-spacer" aria-hidden="true"></span>
                        {/if}
                        <button
                            class="section-item"
                            class:folder-group={folder.isGroup}
                            tabindex="-1"
                            onclick={() => selectFolder(folder.fullPath)}
                            title={folder.fullPath}
                            aria-current={$activeFolder === folder.fullPath ? 'true' : undefined}
                        >
                            <span class="folder-label">{folder.name}</span>
                            <span
                                class="count"
                                title={folder.count === folder.subtreeCount
                                    ? undefined
                                    : `${folder.count} directly in this folder, ${folder.subtreeCount} including subfolders`}
                            >{formatFolderCount(folder.count, folder.subtreeCount)}</span>
                        </button>
                        <button
                            class="menu-btn"
                            tabindex="-1"
                            onclick={(event) => openFolderContextMenu(event, folder.fullPath, folder.name, folder.fullPath !== '/', !folder.isGroup && folder.fullPath !== '/')}
                            title="Folder actions"
                            aria-label={`Folder actions: ${folder.name}`}
                            aria-haspopup="menu"
                        >…</button>
                    </div>
                {/each}
                {#if visibleFolders.length === 0}
                    <div class="section-empty">No folders match "{$sidebarFilter}"</div>
                {/if}
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
                        onkeydown={(event) => { if (isContextMenuKey(event)) openCollectionContextMenu(event, id, name, count); }}
                        aria-current={$activeCollection === id ? 'true' : undefined}
                    >
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
                        class="menu-btn"
                        onclick={(event) => openCollectionContextMenu(event, id, name, count)}
                        title="Collection actions"
                        aria-label={`Collection actions: ${name}`}
                        aria-haspopup="menu"
                    >…</button>
                </div>
            {/each}
        {/if}
    </div>

    {#if visibleSmartCollections.length > 0}
    {@const smartCollapsed = isSectionCollapsed($sidebarSectionsCollapsed, 'smart')}
    <div class="section">
        <button
            class="folders-toggle"
            onclick={() => toggleSection('smart')}
            aria-expanded={!smartCollapsed}
        >
            <span class="toggle-arrow">{smartCollapsed ? '▸' : '▾'}</span>
            <span class="folders-toggle-label">Smart</span>
            <span class="count">{formatSidebarCount(visibleSmartCollections.length)}</span>
        </button>
        {#if !smartCollapsed}
            {#each visibleSmartCollections as sc}
                <button class="section-item"
                    class:active={$activeSmartCollection?.id === sc.id}
                    onclick={() => selectSmartCollection(sc)}
                    oncontextmenu={(event) => openSmartCollectionContextMenu(event, sc)}
                    onkeydown={(event) => { if (isContextMenuKey(event)) openSmartCollectionContextMenu(event, sc); }}
                    aria-current={$activeSmartCollection?.id === sc.id ? 'true' : undefined}>
                    <span class="item-label">{sc.name}</span>
                    <span class="count">{formatSidebarCount(sc.image_count)}</span>
                </button>
            {/each}
        {/if}
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
        <label class="show-missing-toggle">
            <input type="checkbox" bind:checked={$sidebarHideEmpty} />
            Hide empty folders &amp; collections
        </label>
        {#if visibleDetectedClasses.length > 0}
            <div class="detected-header">DETECTED OBJECTS</div>
            {#each visibleDetectedClasses as [cls, count]}
                <button
                    class="section-item detected-class"
                    class:active={$activeDetectedClass === cls}
                    onclick={() => filterByClass(cls)}
                    aria-current={$activeDetectedClass === cls ? 'true' : undefined}
                >
                    <span class="class-tag">{cls}</span>
                    <span class="count">{formatSidebarCount(count)}</span>
                </button>
            {/each}
        {/if}
    </div>

    <div class="section clipboard-monitor">
        <button
            class="folders-toggle"
            onclick={() => toggleSection('clipboard')}
            aria-expanded={!clipboardCollapsed}
        >
            <span class="toggle-arrow">{clipboardCollapsed ? '▸' : '▾'}</span>
            <span class="folders-toggle-label">Clipboard Monitor</span>
            {#if clipboardStatus?.running}
                <span class="count running-dot" title="Monitor running" role="img" aria-label="Monitor running"></span>
            {/if}
        </button>
        {#if !clipboardCollapsed}
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

    {#if sidebarContextMenu}
        {#key sidebarContextMenu.kind === 'folder'
            ? `folder:${sidebarContextMenu.folder}`
            : sidebarContextMenu.kind === 'collection'
                ? `collection:${sidebarContextMenu.collectionId}`
                : sidebarContextMenu.kind === 'canvas'
                    ? `canvas:${sidebarContextMenu.canvas.id}`
                    : `smart:${sidebarContextMenu.collection.id}`}
            <ActionMenu
                title={sidebarContextMenu.kind === 'smart'
                    ? sidebarContextMenu.collection.name
                    : sidebarContextMenu.kind === 'canvas'
                        ? sidebarContextMenu.canvas.name
                        : sidebarContextMenu.name}
                x={sidebarContextMenu.x}
                y={sidebarContextMenu.y}
                items={sidebarContextItems}
                opener={sidebarContextMenu.opener}
                onclose={closeSidebarContextMenu}
            />
        {/key}
    {/if}

    <div class="sidebar-footer" aria-live="polite" aria-busy={importing}>
        {#if lastResult}
            <div class="import-result" class:error={lastResultKind === 'error'}>{lastResult}</div>
        {/if}
        {#if importing}
            <div class="sr-only">
                {importTotal > 0 ? `Importing ${importCurrent} of ${importTotal}` : 'Scanning folder'}
            </div>
        {/if}
        <button
            class="import-btn"
            onclick={handleImportFolder}
            disabled={importing}
            aria-label={importing ? 'Importing folder' : 'Import folder'}
            title="Import folder"
        >
            <span aria-hidden="true">+</span>
        </button>
    </div>
</aside>

{#if smartCollectionEditor && smartCollectionDraft}
    <ModalDialog
        titleId="smart-collection-editor-title"
        descriptionId="smart-collection-editor-description"
        onclose={closeSmartCollectionEditor}
    >
        <div class="smart-editor-dialog">
            <h2 id="smart-collection-editor-title">Edit {smartCollectionEditor.name}</h2>
            <p id="smart-collection-editor-description">Adjust the rules that decide which images appear in this smart collection.</p>
            <RuleBuilder
                filter={smartCollectionDraft}
                onchange={(next) => smartCollectionDraft = next}
            />
            <div class="smart-editor-actions">
                <button type="button" onclick={closeSmartCollectionEditor}>Cancel</button>
                <button type="button" class="primary" data-modal-initial-focus onclick={saveSmartCollectionRules}>Save Rules</button>
            </div>
        </div>
    </ModalDialog>
{/if}

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
        /* imageview-1i2k.7: the sidebar is all captions (≤11px), where
           --text-secondary fails APCA (~Lc 40 on --surface). Everywhere in
           the sidebar, secondary text uses the caption-bright variant. */
        --text-secondary: var(--text-caption);
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
    .sidebar-filter {
        padding: var(--spacing) var(--spacing) 0;
    }
    .sidebar-filter-input {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font-family: inherit;
        font-size: 11px;
        min-height: 28px;
        padding: 4px 8px;
        width: 100%;
    }
    .sidebar-filter-input::placeholder {
        color: var(--text-secondary);
    }
    .sidebar-filter-input:focus {
        border-color: var(--blue);
        outline: none;
    }
    .twisty {
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        flex: none;
        font-family: inherit;
        font-size: 8px;
        line-height: 1;
        padding: 0;
        position: relative;
        width: 14px;
    }
    /* imageview-1i2k.8: the glyph is ~14×8px; the tap target must not be.
       A negative-inset pseudo-element grows the hit area to the 24px floor
       without changing the visual size. */
    .twisty::after {
        content: '';
        position: absolute;
        inset: -8px -5px;
    }
    .twisty:hover {
        color: var(--text);
    }
    .twisty-spacer {
        flex: none;
        width: 14px;
    }
    /* imageview-1i2k.4: CSS-drawn dot, same geometric family as the pin —
       no text-glyph dialects. */
    .running-dot {
        align-self: center;
        background: var(--green);
        border-radius: 50%;
        height: 6px;
        width: 6px;
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
        position: relative;
    }
    /* imageview-1i2k.8: ~19px-tall chip, 24px-tall tap target. */
    .preset-btn::after {
        content: '';
        position: absolute;
        inset: -3px 0;
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
        /* imageview-1i2k.5: green means positive state (success, live). An
           active collect mode is not a success — orange marks modes. */
        color: var(--orange);
        padding: 2px 8px 4px;
        font-style: italic;
    }
    .section-empty {
        font-size: 11px;
        color: var(--text-secondary);
        padding: 4px 8px;
        font-style: italic;
    }
    .recent-rail {
        padding-bottom: 0;
    }
    .section-item .scope-kind {
        color: var(--text-secondary);
        font-size: 11px;
        margin-left: auto;
        flex: none;
    }
    /* Fresh = imported, not visited yet. A hairline + tint, cleared by the     */
    /* visit itself — never by a timer, so it can't expire unseen. Deliberately */
    /* distinct from .active (current scope): this row is a pointer, not state. */
    .recent-scope.fresh {
        background: color-mix(in srgb, var(--blue) 6%, transparent);
        box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--blue) 45%, var(--border));
    }
    .recent-scope .just-imported {
        color: var(--blue);
        flex: none;
        font-size: 11px;
        margin-left: auto;
    }
    .sidebar-footer {
        align-items: center;
        display: flex;
        flex-direction: column;
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
    .import-btn {
        align-items: center;
        background: color-mix(in srgb, var(--blue) 15%, transparent);
        border: 1px solid var(--border);
        border-radius: 50%;
        color: var(--blue);
        cursor: pointer;
        display: inline-flex;
        font-family: var(--font);
        font-size: 20px;
        font-weight: 400;
        height: 32px;
        justify-content: center;
        line-height: 1;
        padding: 0 0 2px;
        transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease, transform 0.15s ease;
        width: 32px;
    }
    .import-btn:hover:not(:disabled) {
        background: color-mix(in srgb, var(--blue) 25%, transparent);
        border-color: var(--blue);
    }
    .import-btn:active:not(:disabled) {
        transform: scale(0.96);
    }
    .import-btn:focus-visible {
        border-color: var(--blue);
        outline: 1px solid var(--blue);
        outline-offset: 2px;
    }
    .import-btn:disabled {
        opacity: 0.5;
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
    /* imageview-1i2k.4: one geometric language. The pin was a two-piece
       pushpin (dialect of its own); now a single rotated square — outline
       when unpinned, filled when pinned. */
    .generated-pin {
        border: 1px solid currentColor;
        border-radius: 1px;
        display: inline-block;
        height: 8px;
        transform: rotate(45deg);
        width: 8px;
    }
    .pin-btn.active .generated-pin {
        background: currentColor;
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
    .menu-btn {
        align-items: center;
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        display: inline-flex;
        flex-shrink: 0;
        font: inherit;
        font-size: 15px;
        height: 24px;
        justify-content: center;
        line-height: 1;
        margin-right: 4px;
        opacity: 0;
        padding: 0;
        pointer-events: none;
        width: 24px;
    }
    .folder-row:hover .menu-btn,
    .folder-row:focus-within .menu-btn {
        opacity: 1;
        pointer-events: auto;
    }
    .menu-btn:hover,
    .menu-btn:focus-visible {
        color: var(--text);
        outline: 1px solid var(--blue);
    }
    .smart-editor-dialog {
        min-width: min(620px, 80vw);
        padding: calc(var(--spacing) * 2);
    }
    .smart-editor-dialog h2 {
        color: var(--text);
        font-size: 16px;
        margin: 0 0 var(--spacing);
    }
    .smart-editor-dialog p {
        color: var(--text-secondary);
        font-size: 12px;
        line-height: 1.5;
        margin: 0 0 calc(var(--spacing) * 2);
    }
    .smart-editor-actions {
        display: flex;
        gap: var(--spacing);
        justify-content: flex-end;
        margin-top: calc(var(--spacing) * 2);
    }
    .smart-editor-actions button {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        cursor: pointer;
        font: inherit;
        padding: 7px 12px;
    }
    .smart-editor-actions button.primary {
        border-color: var(--blue);
        color: var(--blue);
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
