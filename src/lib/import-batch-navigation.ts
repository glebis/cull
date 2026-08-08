import { get } from 'svelte/store';
import {
    focusedImageOverride,
    focusedIndex,
    images,
    importBatchFilter,
    importBatchImageIds,
} from './stores';
import {
    clearImageScope,
    invalidateImageCache,
    loadImagesForCurrentScope,
    resetImagePaging,
} from './image-loading';

export async function activateImportBatch(batchId: string) {
    invalidateImageCache();
    clearImageScope();
    resetImagePaging();
    importBatchFilter.set(batchId);
    await loadImagesForCurrentScope({ force: true, invalidateCache: true });
    importBatchImageIds.set(get(images).map(item => item.image.id));
    focusedImageOverride.set(null);
    focusedIndex.set(0);
}
