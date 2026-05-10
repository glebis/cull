<script lang="ts">
    import '../app.css';
    import TabBar from '$lib/components/TabBar.svelte';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import StatusBar from '$lib/components/StatusBar.svelte';
    import Grid from '$lib/components/Grid.svelte';
    import Compare from '$lib/components/Compare.svelte';
    import Loupe from '$lib/components/Loupe.svelte';
    import Canvas from '$lib/components/Canvas.svelte';
    import EmbeddingExplorer from '$lib/components/EmbeddingExplorer.svelte';
    import UpdateBanner from '$lib/components/UpdateBanner.svelte';
    import CommandBar from '$lib/components/CommandBar.svelte';
    import Export from '$lib/components/Export.svelte';
    import Toast from '$lib/components/Toast.svelte';
    import ImportBanner from '$lib/components/ImportBanner.svelte';
    import LineageView from '$lib/components/LineageView.svelte';
    import Tinder from '$lib/components/Tinder.svelte';
    import McpSettings from '$lib/components/McpSettings.svelte';
    import JobProgressPanel from '$lib/components/JobProgressPanel.svelte';
    import { handleKeydown } from '$lib/keys';
    import { totalCount, images, focusedIndex, viewMode, sidebarVisible, zenMode, activeFolder, minSizeFilter, activeCollection, collections, showToast, settingsOpen, searchOpen } from '$lib/stores';
    import { getImageCount, listImages, listImagesByFolder, listImagesFiltered, listCollectionImages, trashImages, deleteImagesPermanently } from '$lib/api';
    import { initDeepLink } from '$lib/deeplink';
    import { initMenu } from '$lib/menu';
    import { saveAppState, restoreAppStateBeforeImages, applyRestoredViewState } from '$lib/persistence';
    import { listen } from '@tauri-apps/api/event';
    import { onMount } from 'svelte';

    let dragOver = $state(false);

    let immersive = $derived($viewMode === 'loupe' || $viewMode === 'compare');
    let noSidebar = $derived(immersive || !$sidebarVisible);

    async function loadImages() {
        const count = await getImageCount();
        totalCount.set(count);
        const collection = $activeCollection;
        if (collection !== null) {
            const imgs = await listCollectionImages(collection);
            images.set(imgs);
            focusedIndex.set(0);
            return;
        }
        if (count > 0) {
            const folder = $activeFolder;
            const minSize = $minSizeFilter;
            let imgs;
            if (folder !== null) {
                imgs = await listImagesByFolder(folder, 100000, 0);
                // Client-side size filter when combined with folder
                if (minSize > 0) {
                    imgs = imgs.filter(img => img.image.width >= minSize && img.image.height >= minSize);
                }
            } else if (minSize > 0) {
                imgs = await listImagesFiltered(minSize, minSize, 100000, 0);
            } else {
                imgs = await listImages(100000, 0);
            }
            images.set(imgs);
            focusedIndex.set(0);
        }
    }

    async function handleTrash() {
        const imgs = $images;
        const idx = $focusedIndex;
        const img = imgs[idx];
        if (!img) return;
        const count = await trashImages([img.image.id]);
        if (count > 0) {
            const name = img.path.split('/').pop() ?? '';
            showToast(`Moved to Trash`, { detail: name, type: 'info', duration: 5000 });
            images.update(list => list.filter((_, i) => i !== idx));
            focusedIndex.update(i => Math.min(i, $images.length - 1));
            totalCount.update(c => c - 1);
        }
    }

    async function handlePermanentDelete() {
        const imgs = $images;
        const idx = $focusedIndex;
        const img = imgs[idx];
        if (!img) return;
        const name = img.path.split('/').pop() ?? '';
        if (!confirm(`Permanently delete "${name}"? This cannot be undone.`)) return;
        const count = await deleteImagesPermanently([img.image.id]);
        if (count > 0) {
            showToast(`Deleted permanently`, { detail: name, type: 'warning', duration: 5000 });
            images.update(list => list.filter((_, i) => i !== idx));
            focusedIndex.update(i => Math.min(i, $images.length - 1));
            totalCount.update(c => c - 1);
        }
    }

    onMount(() => {
        const init = async () => {
            const restored = restoreAppStateBeforeImages();
            await loadImages();
            applyRestoredViewState(restored);
            await initDeepLink();
        };
        init().catch(e => console.error('Failed to initialize app:', e));
        initMenu().catch(e => console.error('Failed to init menu:', e));

        const dragUnlisten = listen<boolean>('drag-hover', (event) => {
            dragOver = event.payload;
        });

        window.addEventListener('trash-focused-image', handleTrash);
        window.addEventListener('delete-focused-image', handlePermanentDelete);

        let first = true;
        const unsub = minSizeFilter.subscribe(() => {
            if (first) { first = false; return; }
            loadImages().catch(e => console.error('Failed to reload images with filter:', e));
        });

        const saveTimer = setInterval(saveAppState, 5000);
        const handleBeforeUnload = () => saveAppState();
        window.addEventListener('beforeunload', handleBeforeUnload);

        return () => {
            unsub();
            dragUnlisten.then(fn => fn());
            window.removeEventListener('trash-focused-image', handleTrash);
            window.removeEventListener('delete-focused-image', handlePermanentDelete);
            clearInterval(saveTimer);
            window.removeEventListener('beforeunload', handleBeforeUnload);
            saveAppState();
        };
    });
</script>

<svelte:window onkeydown={handleKeydown} />

<UpdateBanner />
<div class="app-shell" class:no-sidebar={noSidebar} class:zen={$zenMode}>
    {#if !$zenMode}
        <TabBar />
    {/if}
    {#if !noSidebar && !$zenMode}
        <Sidebar />
    {/if}
    <ImportBanner />
    {#if $viewMode === 'grid'}
        <div class="main-with-commandbar">
            <div class="command-bar-area">
                <CommandBar />
            </div>
            <Grid />
        </div>
    {:else if $viewMode === 'compare'}
        <Compare />
    {:else if $viewMode === 'loupe'}
        <Loupe />
    {:else if $viewMode === 'embeddings'}
        <EmbeddingExplorer />
    {:else if $viewMode === 'export'}
        <Export />
    {:else if $viewMode === 'lineage'}
        <LineageView />
    {:else if $viewMode === 'canvas'}
        <Canvas />
    {:else if $viewMode === 'tinder'}
        <Tinder />
    {:else}
        <div class="placeholder">
            <span class="placeholder-label">{$viewMode}</span>
            <span class="placeholder-text">Coming soon</span>
        </div>
    {/if}
    {#if !$zenMode}
        <StatusBar />
    {/if}

    <Toast />

    {#if dragOver}
        <div class="drop-overlay">
            <div class="drop-label">Drop to import</div>
        </div>
    {/if}
</div>

<JobProgressPanel />

{#if $settingsOpen}
    <McpSettings onclose={() => settingsOpen.set(false)} />
{/if}

<style>
    .app-shell {
        display: grid;
        grid-template-areas:
            "tabbar tabbar"
            "sidebar main"
            "statusbar statusbar";
        grid-template-rows: 40px 1fr 32px;
        grid-template-columns: 220px 1fr;
        height: 100vh;
        width: 100vw;
        background: var(--bg);
    }
    .app-shell.no-sidebar {
        grid-template-areas:
            "tabbar"
            "main"
            "statusbar";
        grid-template-columns: 1fr;
    }
    .app-shell.zen {
        grid-template-areas: "main";
        grid-template-rows: 1fr;
        grid-template-columns: 1fr;
    }
    .placeholder {
        grid-area: main;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
        color: var(--text-secondary);
    }
    .placeholder-label {
        text-transform: uppercase;
        font-size: 14px;
        color: var(--text-secondary);
        font-weight: 700;
    }
    .placeholder-text {
        font-size: 12px;
        opacity: 0.5;
    }
    .drop-overlay {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.7);
        border: 3px solid var(--accent, #4a9eff);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 9999;
        pointer-events: none;
    }
    .drop-label {
        font-size: 18px;
        font-weight: 700;
        color: var(--accent, #4a9eff);
        text-transform: uppercase;
        letter-spacing: 2px;
    }
    .main-with-commandbar {
        grid-area: main;
        display: flex;
        flex-direction: column;
        overflow: hidden;
    }
    .main-with-commandbar :global(.grid-container) {
        grid-area: unset;
        flex: 1;
        min-height: 0;
    }
    .command-bar-area {
        padding: 8px 12px 0;
        flex-shrink: 0;
    }
</style>
