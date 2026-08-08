import { getBatchImages } from './api';
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
    resetImagePaging,
} from './image-loading';

export async function activateImportBatch(batchId: string) {
    const batchImages = await getBatchImages(batchId);
    invalidateImageCache();
    clearImageScope();
    resetImagePaging();
    images.set(batchImages);
    importBatchFilter.set(batchId);
    importBatchImageIds.set(batchImages.map(item => item.image.id));
    focusedImageOverride.set(null);
    focusedIndex.set(0);
}
