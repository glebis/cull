<script lang="ts">
    import { open } from '@tauri-apps/plugin-dialog';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { totalCount, images, focusedIndex } from '$lib/stores';
    import { importFolder as apiImportFolder, listImages, getImageCount } from '$lib/api';

    let importing = $state(false);
    let importCurrent = $state(0);
    let importTotal = $state(0);
    let lastResult = $state('');

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
        const imgs = await listImages(10000, 0);
        images.set(imgs);
        focusedIndex.set(0);
    }
</script>

<div class="sidebar">
    <div class="section">
        <div class="section-header">LIBRARY</div>
        <div class="section-item active">
            <span class="icon">&#9632;</span>
            All Images
            <span class="count">({$totalCount})</span>
        </div>
    </div>

    <div class="section">
        <div class="section-header">PROJECTS</div>
        <div class="section-empty">No projects yet</div>
    </div>

    <div class="sidebar-footer">
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
