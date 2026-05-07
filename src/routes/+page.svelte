<script lang="ts">
    import '../app.css';
    import TabBar from '$lib/components/TabBar.svelte';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import StatusBar from '$lib/components/StatusBar.svelte';
    import Grid from '$lib/components/Grid.svelte';
    import Compare from '$lib/components/Compare.svelte';
    import Loupe from '$lib/components/Loupe.svelte';
    import EmbeddingExplorer from '$lib/components/EmbeddingExplorer.svelte';
    import { handleKeydown } from '$lib/keys';
    import { totalCount, images, focusedIndex, viewMode, sidebarVisible, zenMode, activeFolder, minSizeFilter, activeCollection, collections } from '$lib/stores';
    import { getImageCount, listImages, listImagesByFolder, listImagesFiltered, listCollectionImages } from '$lib/api';
    import { initDeepLink } from '$lib/deeplink';
    import { onMount } from 'svelte';

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

    onMount(() => {
        loadImages().catch(e => console.error('Failed to load images on mount:', e));
        initDeepLink().catch(e => console.error('Failed to init deep link:', e));

        let first = true;
        const unsub = minSizeFilter.subscribe(() => {
            if (first) { first = false; return; }
            loadImages().catch(e => console.error('Failed to reload images with filter:', e));
        });

        return unsub;
    });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app-shell" class:no-sidebar={noSidebar} class:zen={$zenMode}>
    {#if !$zenMode}
        <TabBar />
    {/if}
    {#if !noSidebar && !$zenMode}
        <Sidebar />
    {/if}
    {#if $viewMode === 'grid'}
        <Grid />
    {:else if $viewMode === 'compare'}
        <Compare />
    {:else if $viewMode === 'loupe'}
        <Loupe />
    {:else if $viewMode === 'embeddings'}
        <EmbeddingExplorer />
    {:else}
        <div class="placeholder">
            <span class="placeholder-label">{$viewMode}</span>
            <span class="placeholder-text">Coming soon</span>
        </div>
    {/if}
    {#if !$zenMode}
        <StatusBar />
    {/if}
</div>

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
</style>
