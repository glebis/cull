<script lang="ts">
    import { importBatchFilter, importBatchImageIds, images, focusedIndex, pinnedCollection, collections, activeCollection, showToast } from '$lib/stores';
    import { listImages, createCollection, addToCollection, listCollections, getBatchImages } from '$lib/api';
    import { get } from 'svelte/store';

    let count = $derived($importBatchImageIds.length);
    let visible = $derived($importBatchFilter !== null && count > 0);

    async function showAll() {
        importBatchFilter.set(null);
        importBatchImageIds.set([]);
        const allImgs = await listImages(100000, 0);
        images.set(allImgs);
        focusedIndex.set(0);
    }

    async function saveAsCollection() {
        const batchId = get(importBatchFilter);
        if (!batchId) return;

        const name = window.prompt('Collection name:', `Import ${new Date().toLocaleString()}`);
        if (!name || !name.trim()) return;

        try {
            const collectionId = await createCollection(name.trim());
            const ids = get(importBatchImageIds);
            await addToCollection(collectionId, ids);

            // Pin as active
            pinnedCollection.set(collectionId);
            activeCollection.set(collectionId);

            // Refresh collections list
            const c = await listCollections();
            collections.set(c);

            importBatchFilter.set(null);
            importBatchImageIds.set([]);

            showToast(`Collection "${name.trim()}" created`, { type: 'success', duration: 5000 });
        } catch (e) {
            console.error('Failed to save collection:', e);
            showToast('Failed to create collection', { type: 'error' });
        }
    }
</script>

{#if visible}
<div class="import-banner">
    <span class="count">{count} images imported</span>
    <button class="banner-action primary" onclick={saveAsCollection}>Save as collection</button>
    <button class="banner-action" onclick={showAll}>Show all</button>
</div>
{/if}

<style>
    .import-banner {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 6px 16px;
        background: var(--bg-elevated, #2a2a3e);
        border-bottom: 1px solid var(--border, #333);
        font-size: 13px;
        z-index: 10;
    }
    .count {
        color: var(--accent, #8cc63f);
        font-weight: 600;
    }
    .banner-action {
        background: none;
        border: 1px solid var(--border, #444);
        color: var(--text-secondary, #aaa);
        padding: 3px 10px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 12px;
    }
    .banner-action:hover {
        background: var(--bg-hover, #333);
        color: var(--text-primary, #eee);
    }
    .banner-action.primary {
        border-color: var(--accent, #8cc63f);
        color: var(--accent, #8cc63f);
    }
</style>
