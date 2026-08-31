<script lang="ts">
    import { onDestroy, onMount } from 'svelte';
    import { get } from 'svelte/store';
    import { activeReferencedFolder, showToast } from '$lib/stores';
    import { listSourceFolders, type ReferencedSource } from '$lib/api';
    import { initializeReferencedSources, openReferencedSourceFolder, referencedSources } from '$lib/referenced-sources';
    import ActionMenu from './ActionMenu.svelte';
    import { buildReferencedFolderContextActions } from '$lib/sidebar-context-actions';

    interface Props {
        onimportfolder?: (folder: string) => void | Promise<void>;
        onrevealfolder?: (folder: string) => void | Promise<void>;
        oncopypath?: (folder: string) => void | Promise<void>;
    }

    let { onimportfolder, onrevealfolder, oncopypath }: Props = $props();
    let childFolders = $state<string[]>([]);
    let loadingFolders = $state(false);
    let dispose: (() => void) | null = null;
    let folderMenu = $state<{
        source: ReferencedSource;
        relativePath: string;
        folder: string;
        name: string;
        x: number;
        y: number;
        opener: HTMLElement | null;
    } | null>(null);
    let connectedSources = $derived($referencedSources.filter(source => !source.offline_at));
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

    function absoluteFolderPath(source: ReferencedSource, relativePath: string): string | null {
        const root = source.last_mount_path?.replace(/[\\/]+$/, '');
        if (!root) return null;
        const parts = relativePath.split(/[\\/]/).filter(part => part && part !== '.');
        if (parts.includes('..')) return null;
        const separator = root.includes('\\') ? '\\' : '/';
        return [root, ...parts].join(separator);
    }

    function menuPoint(event: MouseEvent | KeyboardEvent, anchor: HTMLElement) {
        if (event instanceof MouseEvent && (event.clientX || event.clientY)) return { x: event.clientX, y: event.clientY };
        const rect = anchor.getBoundingClientRect();
        return { x: rect.left + 16, y: rect.bottom };
    }

    function openFolderMenu(
        event: MouseEvent | KeyboardEvent,
        source: ReferencedSource,
        relativePath: string,
        name: string,
        anchor = event.currentTarget as HTMLElement,
    ) {
        event.preventDefault();
        event.stopPropagation();
        const folder = absoluteFolderPath(source, relativePath);
        if (!folder) {
            showToast('Could not open folder actions', { detail: 'The device has no mounted path.', type: 'error' });
            return;
        }
        folderMenu = { source, relativePath, folder, name, ...menuPoint(event, anchor), opener: anchor };
    }

    function openFolderMenuFromButton(event: MouseEvent, source: ReferencedSource, relativePath: string, name: string) {
        const anchor = event.currentTarget as HTMLElement;
        const rect = anchor.getBoundingClientRect();
        const contextEvent = new MouseEvent('contextmenu', { clientX: rect.right, clientY: rect.bottom });
        openFolderMenu(contextEvent, source, relativePath, name, anchor);
    }

    function handleFolderMenuKey(event: KeyboardEvent, source: ReferencedSource, relativePath: string, name: string) {
        if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return;
        openFolderMenu(event, source, relativePath, name, event.currentTarget as HTMLElement);
    }

    async function importReferencedFolder(folder: string) {
        if (onimportfolder) await onimportfolder(folder);
    }

    async function revealReferencedFolder(folder: string) {
        if (onrevealfolder) await onrevealfolder(folder);
    }

    async function copyReferencedFolderPath(folder: string) {
        if (oncopypath) {
            await oncopypath(folder);
            return;
        }
        try {
            await navigator.clipboard.writeText(folder);
            showToast('Folder path copied', { type: 'success', duration: 2500 });
        } catch (error) {
            showToast('Copy failed', { detail: String(error), type: 'error', duration: 8000 });
        }
    }

    let folderMenuItems = $derived.by(() => {
        const target = folderMenu;
        return target
        ? buildReferencedFolderContextActions({
            folder: target.folder,
            onOpen: () => openSource(target.source, target.relativePath),
            onReveal: revealReferencedFolder,
            onImport: importReferencedFolder,
            onCopyPath: copyReferencedFolderPath,
        })
        : [];
    });
</script>

{#if connectedSources.length > 0}
<div class="devices-section" data-testid="devices-section">
    {#each connectedSources as source (source.id)}
        <div class="folder-entry">
            <button
                class="section-item folder-open device"
                class:active={$activeReferencedFolder?.source_id === source.id}
                class:offline={!!source.offline_at}
                onclick={() => openSource(source)}
                oncontextmenu={(event) => openFolderMenu(event, source, '', source.display_name)}
                onkeydown={(event) => handleFolderMenuKey(event, source, '', source.display_name)}
                title={source.offline_at ? `Reconnect ${source.display_name} to open originals` : source.last_mount_path ?? source.display_name}
            >
                <span class="device-icon" aria-hidden="true">{sourceIcon(source)}</span><span class="item-label">{source.display_name}</span><span class="status">{source.offline_at ? 'offline' : 'connected'}</span>
            </button>
            <button class="folder-actions" aria-label={`Actions for ${source.display_name}`} title={`Actions for ${source.display_name}`} onclick={(event) => openFolderMenuFromButton(event, source, '', source.display_name)}>…</button>
        </div>
        {#if $activeReferencedFolder?.source_id === source.id && !source.offline_at}
            {#if $activeReferencedFolder.relative_path}
                {@const parentPath = $activeReferencedFolder.relative_path.split('/').filter(Boolean).slice(0, -1).join('/')}
                <div class="folder-entry child-entry">
                    <button class="section-item folder-open child" onclick={() => openParent(source)} oncontextmenu={(event) => openFolderMenu(event, source, parentPath, 'Parent folder')} onkeydown={(event) => handleFolderMenuKey(event, source, parentPath, 'Parent folder')}><span class="device-icon" aria-hidden="true">↰</span><span class="item-label">Parent folder</span></button>
                    <button class="folder-actions" aria-label="Actions for Parent folder" title="Actions for Parent folder" onclick={(event) => openFolderMenuFromButton(event, source, parentPath, 'Parent folder')}>…</button>
                </div>
            {/if}
            {#each childFolders as folder (folder)}
                {@const childName = folder.split('/').pop() ?? folder}
                <div class="folder-entry child-entry">
                    <button class="section-item folder-open child" onclick={() => openSource(source, folder)} oncontextmenu={(event) => openFolderMenu(event, source, folder, childName)} onkeydown={(event) => handleFolderMenuKey(event, source, folder, childName)}><span class="device-icon" aria-hidden="true">▸</span><span class="item-label">{childName}</span></button>
                    <button class="folder-actions" aria-label={`Actions for ${childName}`} title={`Actions for ${childName}`} onclick={(event) => openFolderMenuFromButton(event, source, folder, childName)}>…</button>
                </div>
            {/each}
            {#if loadingFolders}<div class="empty child">Reading folders…</div>{/if}
        {/if}
    {/each}
</div>
{/if}

{#if folderMenu}
    <ActionMenu title={folderMenu.name} x={folderMenu.x} y={folderMenu.y} items={folderMenuItems} opener={folderMenu.opener} onclose={() => folderMenu = null} />
{/if}

<style>
    .devices-section { border-bottom: 1px solid var(--border); padding: var(--spacing); }
    .section-item {
        align-items: center;
        background: none;
        border: none;
        border-radius: var(--radius);
        color: inherit;
        cursor: pointer;
        display: flex;
        font-family: inherit;
        font-size: 12px;
        gap: 6px;
        min-height: 28px;
        overflow: hidden;
        padding: 6px 8px;
        text-align: left;
        width: 100%;
    }
    .section-item:hover { background: var(--border); }
    .section-item.active { background: color-mix(in srgb, var(--blue) 10%, transparent); color: var(--blue); }
    .device-icon { width: 16px; color: var(--blue); flex: 0 0 auto; }
    .device.offline { color: var(--text-secondary); }
    .device.offline .device-icon { color: var(--orange); }
    .status { margin-left: auto; color: var(--text-secondary); font-size: 10px; }
    .child { padding-left: 28px; }
    .folder-entry { align-items: center; display: flex; min-width: 0; }
    .folder-open { flex: 1 1 auto; min-width: 0; width: auto; }
    .folder-actions {
        align-items: center;
        background: none;
        border: none;
        border-radius: var(--radius);
        color: var(--text-secondary);
        cursor: pointer;
        display: flex;
        flex: 0 0 28px;
        font-family: inherit;
        height: 28px;
        justify-content: center;
        padding: 0;
    }
    .folder-actions:hover, .folder-actions:focus-visible { background: var(--border); color: var(--text); }
    .empty { color: var(--text-secondary); font-size: 11px; padding: 6px 12px; }
</style>
