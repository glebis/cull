<script lang="ts">
    import { get } from 'svelte/store';
    import { activeReferencedFolder } from '$lib/stores';
    import { setSourceRecursiveDefault } from '$lib/api';
    import { loadNextReferencedPage, openReferencedSourceFolder, referencedFolderPage, referencedSourceIndexing, referencedSources } from '$lib/referenced-sources';
    async function toggleRecursive() {
        const scope = get(activeReferencedFolder);
        const source = get(referencedSources).find(item => item.id === scope?.source_id);
        if (!scope || !source) return;
        const recursive = !scope.recursive;
        await setSourceRecursiveDefault(source.id, recursive);
        source.recursive_default = recursive;
        referencedSources.update(items => [...items]);
        await openReferencedSourceFolder(source, scope.relative_path, recursive);
    }
</script>

{#if $activeReferencedFolder}
    <div class="source-toolbar" data-testid="referenced-source-toolbar">
        <div class="breadcrumb"><span class="source-name">{$activeReferencedFolder.source_name}</span>{#if $activeReferencedFolder.relative_path}<span class="separator">/</span><span>{$activeReferencedFolder.relative_path}</span>{/if}</div>
        <button class:active={$activeReferencedFolder.recursive} onclick={toggleRecursive}>{$activeReferencedFolder.recursive ? 'Including subfolders' : 'Current folder'}</button>
        {#if $referencedSourceIndexing}<span class="indexing">Reading previews…</span>{/if}
        {#if $referencedFolderPage?.next_cursor && !$referencedSourceIndexing}<button onclick={loadNextReferencedPage}>Load next</button>{/if}
    </div>
{/if}

<style>
    .source-toolbar { display: flex; align-items: center; gap: var(--spacing); min-height: 34px; padding: 0 calc(var(--spacing) * 1.5); border-bottom: 1px solid var(--border); background: var(--surface); color: var(--text-secondary); font-size: 11px; }
    .breadcrumb { display: flex; gap: 5px; min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
    .source-name { color: var(--text); } .separator { color: var(--text-secondary); }
    button { border: 1px solid var(--border); border-radius: var(--radius); padding: 4px 7px; background: transparent; color: var(--text-secondary); font: inherit; cursor: pointer; }
    button:hover, button.active { color: var(--text); border-color: var(--blue); }
    .indexing { margin-left: auto; color: var(--blue); }
</style>
