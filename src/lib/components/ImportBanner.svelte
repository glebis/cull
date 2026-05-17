<script lang="ts">
    import { importBatchFilter, importBatchImageIds, pinnedCollection, collections, activeCollection, activeFolder, activeSmartCollection, activeDetectedClass, showToast, requestTextInput } from '$lib/stores';
    import { createCollection, addToCollection, listCollections, getBatchImages, getGenerationRun } from '$lib/api';
    import { invalidateImageCache, loadAllImages } from '$lib/image-loading';
    import { generateImportCollectionName, type ImportCollectionNameItem } from '$lib/collection-name';
    import { get } from 'svelte/store';

    let count = $derived($importBatchImageIds.length);
    let visible = $derived($importBatchFilter !== null && count > 0);

    async function showAll() {
        importBatchFilter.set(null);
        importBatchImageIds.set([]);
        await loadAllImages({ force: true, invalidateCache: true });
    }

    async function saveAsCollection() {
        const batchId = get(importBatchFilter);
        if (!batchId) return;

        const initialValue = await buildDefaultCollectionName(batchId);
        const name = await requestTextInput({
            title: 'Save Import as Collection',
            label: 'Collection name',
            initialValue,
            confirmLabel: 'Save',
        });
        if (!name || !name.trim()) return;

        try {
            const collectionId = await createCollection(name.trim());
            const ids = get(importBatchImageIds);
            await addToCollection(collectionId, ids);
            invalidateImageCache();

            // Pin as active
            pinnedCollection.set(collectionId);
            activeCollection.set(collectionId);
            activeFolder.set(null);
            activeSmartCollection.set(null);
            activeDetectedClass.set(null);

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

    async function buildDefaultCollectionName(batchId: string): Promise<string> {
        const now = new Date();
        try {
            const batchImages = await getBatchImages(batchId);
            const generationPrompts = new Map<string, string | null>();
            const promptCandidates = batchImages
                .filter(img => !img.image.ai_prompt)
                .slice(0, 12);

            await Promise.all(promptCandidates.map(async (img) => {
                try {
                    const run = await getGenerationRun(img.image.id);
                    generationPrompts.set(img.image.id, run?.prompt ?? null);
                } catch {
                    generationPrompts.set(img.image.id, null);
                }
            }));

            const items: ImportCollectionNameItem[] = batchImages.map(img => ({
                path: img.path,
                aiPrompt: img.image.ai_prompt,
                generationPrompt: generationPrompts.get(img.image.id) ?? null,
                importedAt: img.image.imported_at,
            }));
            return generateImportCollectionName(items);
        } catch (e) {
            console.warn('Failed to build import collection name:', e);
            return generateImportCollectionName([], { now });
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
