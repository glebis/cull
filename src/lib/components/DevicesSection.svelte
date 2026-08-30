<script lang="ts">
    import { onDestroy, onMount } from 'svelte';
    import { get } from 'svelte/store';
    import { activeReferencedFolder, showToast } from '$lib/stores';
    import { listSourceFolders, type ReferencedSource } from '$lib/api';
    import { initializeReferencedSources, openReferencedSourceFolder, referencedSources } from '$lib/referenced-sources';
    let childFolders = $state<string[]>([]);
    let loadingFolders = $state(false);
    let dispose: (() => void) | null = null;
    onMount(async () => {
        try { dispose = await initializeReferencedSources(); }
        catch (error) { console.warn('Referenced sources unavailable', error); }
    });
    onDestroy(() => dispose?.());
    function sourceIcon(source: ReferencedSource) { return source.source_kind === 'sd_card' ? '▣' : '◆'; }
    async function openSource(source: ReferencedSource, relativePath = '') {
        await openReferencedSourceFolder(source, relativePath);
        if (source.offline_at) { childFolders = []; return; }
        loadingFolders = true;
        try { childFolders = await listSourceFolders(source.id, relativePath); }
        catch (error) {
            childFolders = [];
            showToast('Could not read folders on the device', { detail: error instanceof Error ? error.message : String(error), type: 'error' });
        } finally { loadingFolders = false; }
    }
    async function openParent(source: ReferencedSource) {
        const current = get(activeReferencedFolder)?.relative_path ?? '';
        await openSource(source, current.split('/').filter(Boolean).slice(0, -1).join('/'));
    }
</script>

<div class="section devices-section" data-testid="devices-section">
    <div class="section-header">DEVICES</div>
    {#if $referencedSources.length === 0}<div class="empty">Plug in an SD card or drive</div>{/if}
    {#each $referencedSources as source (source.id)}
        <button class="section-item device" class:active={$activeReferencedFolder?.source_id === source.id} class:offline={!!source.offline_at} onclick={() => openSource(source)} title={source.offline_at ? `Reconnect ${source.display_name} to open originals` : source.last_mount_path ?? source.display_name}>
            <span class="device-icon">{sourceIcon(source)}</span><span class="item-label">{source.display_name}</span><span class="status">{source.offline_at ? 'offline' : 'connected'}</span>
        </button>
        {#if $activeReferencedFolder?.source_id === source.id && !source.offline_at}
            {#if $activeReferencedFolder.relative_path}<button class="section-item child" onclick={() => openParent(source)}><span class="device-icon">↰</span><span class="item-label">Parent folder</span></button>{/if}
            {#each childFolders as folder (folder)}
                <button class="section-item child" onclick={() => openSource(source, folder)}><span class="device-icon">▸</span><span class="item-label">{folder.split('/').pop()}</span></button>
            {/each}
            {#if loadingFolders}<div class="empty child">Reading folders…</div>{/if}
        {/if}
    {/each}
</div>

<style>
    .devices-section { border-bottom: 1px solid var(--border); padding-bottom: var(--spacing); }
    .device-icon { width: 16px; color: var(--blue); flex: 0 0 auto; }
    .device.offline { color: var(--text-secondary); }
    .device.offline .device-icon { color: var(--orange); }
    .status { margin-left: auto; color: var(--text-secondary); font-size: 10px; }
    .child { padding-left: 28px; }
    .empty { color: var(--text-secondary); font-size: 11px; padding: 6px 12px; }
</style>
