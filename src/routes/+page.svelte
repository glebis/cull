<script lang="ts">
    import '../app.css';
    import TabBar from '$lib/components/TabBar.svelte';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import StatusBar from '$lib/components/StatusBar.svelte';
    import Grid from '$lib/components/Grid.svelte';
    import { handleKeydown } from '$lib/keys';
    import { totalCount, images, focusedIndex } from '$lib/stores';
    import { getImageCount, listImages } from '$lib/api';
    import { onMount } from 'svelte';

    onMount(async () => {
        try {
            const count = await getImageCount();
            totalCount.set(count);
            if (count > 0) {
                const imgs = await listImages(10000, 0);
                images.set(imgs);
                focusedIndex.set(0);
            }
        } catch (e) {
            console.error('Failed to load images on mount:', e);
        }
    });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app-shell">
    <TabBar />
    <Sidebar />
    <Grid />
    <StatusBar />
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
</style>
