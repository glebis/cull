<script lang="ts">
    import { open } from '@tauri-apps/plugin-dialog';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { totalCount, images, focusedIndex, folders, activeFolder } from '$lib/stores';
    import { importFolder as apiImportFolder, listImages, listImagesByFolder, getImageCount, listFolders, deleteFolder as apiDeleteFolder } from '$lib/api';
    import { onMount } from 'svelte';
    import { get } from 'svelte/store';

    let importing = $state(false);
    let importCurrent = $state(0);
    let importTotal = $state(0);
    let lastResult = $state('');

    onMount(async () => {
        try {
            const f = await listFolders();
            folders.set(f);
        } catch (e) {
            console.error('Failed to load folders:', e);
        }
    });

    function folderName(path: string): string {
        const parts = path.split('/');
        return parts[parts.length - 1] || path;
    }

    async function selectFolder(folder: string | null) {
        activeFolder.set(folder);
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
        <button class="section-item" class:active={$activeFolder === null} onclick={() => selectFolder(null)}>
            <span class="icon">&#9632;</span>
            All Images
            <span class="count">({$totalCount})</span>
        </button>
        {#each $folders as [path, count]}
            <div class="folder-row" class:active={$activeFolder === path}>
                <button class="section-item" onclick={() => selectFolder(path)}>
                    <span class="icon">&#9656;</span>
                    {folderName(path)}
                    <span class="count">({count})</span>
                </button>
                <button class="delete-btn" onclick={(e: Event) => handleDeleteFolder(e, path)} title="Remove folder">&times;</button>
            </div>
        {/each}
    </div>

    <div class="section">
        <div class="section-header">PROJECTS</div>
        <div class="section-empty">No projects yet</div>
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
