<script lang="ts">
    import { open } from '@tauri-apps/plugin-dialog';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { totalCount, images, focusedIndex, folders, activeFolder, minSizeFilter, collections, activeCollection, collectMode, collectModeTarget, smartCollections, activeSmartCollection } from '$lib/stores';
    import { importFolder as apiImportFolder, listImages, listImagesByFolder, listImagesFiltered, getImageCount, listFolders, deleteFolder as apiDeleteFolder, listCollections, createCollection, listCollectionImages, deleteCollectionApi, listSmartCollections, evaluateSmartCollection } from '$lib/api';
    import type { SmartCollection } from '$lib/api';
    import { onMount } from 'svelte';
    import { get } from 'svelte/store';

    let importing = $state(false);
    let importCurrent = $state(0);
    let importTotal = $state(0);
    let lastResult = $state('');
    let foldersExpanded = $state(true);

    interface DisplayFolder {
        name: string;
        disambig: string; // parent context shown when names collide
        fullPath: string;
        count: number;
    }

    // Build a flat, disambiguated folder list
    function buildDisplayFolders(flatFolders: [string, number][]): DisplayFolder[] {
        const result: DisplayFolder[] = flatFolders.map(([fullPath, count]) => {
            const parts = fullPath.split('/').filter(p => p.length > 0);
            const name = parts[parts.length - 1] || fullPath;
            return { name, disambig: '', fullPath, count };
        });

        // Find duplicate names and add parent path to disambiguate
        const byName = new Map<string, DisplayFolder[]>();
        for (const f of result) {
            const group = byName.get(f.name) || [];
            group.push(f);
            byName.set(f.name, group);
        }
        for (const [, group] of byName) {
            if (group.length <= 1) continue;
            for (const f of group) {
                const parts = f.fullPath.split('/').filter(p => p.length > 0);
                // Show up to 2 parent segments for context
                const contextParts = parts.slice(Math.max(0, parts.length - 3), parts.length - 1);
                f.disambig = contextParts.join('/');
            }
        }

        result.sort((a, b) => a.name.localeCompare(b.name));
        return result;
    }

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
    });

    function folderName(path: string): string {
        const parts = path.split('/');
        return parts[parts.length - 1] || path;
    }

    async function selectSmartCollection(sc: SmartCollection) {
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
            lastResult = `+${result.imported} imported, ${result.skipped} skipped`;
            if (result.errors.length > 0) {
                lastResult += `, ${result.errors.length} errors`;
            }
            await refreshImages();
        } catch (e) {
            lastResult = `Error: ${e}`;
        } finally {
            unlisten();
            importing = false;
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
                {#each displayFolders as folder}
                    <div class="folder-row" class:active={$activeFolder === folder.fullPath}>
                        <button class="section-item" onclick={() => selectFolder(folder.fullPath)} title={folder.fullPath}>
                            <span class="icon">&#9656;</span>
                            <span class="folder-label">
                                {folder.name}
                                {#if folder.disambig}
                                    <span class="folder-disambig">{folder.disambig}</span>
                                {/if}
                            </span>
                            <span class="count">({folder.count})</span>
                        </button>
                        <button class="delete-btn" onclick={(e: Event) => handleDeleteFolder(e, folder.fullPath)} title="Remove folder">&times;</button>
                    </div>
                {/each}
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
            </button>
        {/each}
    </div>
    {/if}

    <div class="section">
        <div class="section-header">
            COLLECTIONS
            <button class="new-collection-btn" onclick={handleNewCollection} title="New Collection">+</button>
        </div>
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
                    <button class="delete-btn" onclick={(e: Event) => handleDeleteCollection(e, id, name)} title="Delete collection">&times;</button>
                </div>
            {/each}
        {/if}
    </div>

    <div class="sidebar-footer" aria-live="polite">
        {#if lastResult}
            <div class="import-result">{lastResult}</div>
        {/if}
        <button class="import-btn" onclick={handleImportFolder} disabled={importing}>
            {importing ? (importTotal > 0 ? `Importing ${importCurrent}/${importTotal}...` : 'Scanning...') : '+ Import Folder'}
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
        padding: 4px 8px;
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
        padding: 2px 4px;
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
        padding: 4px 8px;
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
        display: flex;
        flex-direction: column;
        min-width: 0;
        overflow: hidden;
    }
    .folder-disambig {
        font-size: 9px;
        color: var(--text-secondary);
        opacity: 0.5;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        line-height: 1.2;
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
        padding: 2px 6px;
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
</style>
