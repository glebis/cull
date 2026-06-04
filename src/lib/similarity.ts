// Semantic similarity search: find images visually similar to a target using
// stored CLIP/DINOv2 embeddings, and load them into the current view ordered by
// descending similarity. The backend (find_similar_images) does the vector math.

import { findSimilarImages, getImagesByIds, type ImageWithFile } from './api';
import { clearImageScope, resetImagePaging } from './image-loading';
import { focusedIndex, images } from './stores';

// Reorder fetched images to match the ranked similarity order. Images missing
// from the ranking sort to the end (Infinity), preserving determinism.
export function orderBySimilarity(rankedIds: string[], fetched: ImageWithFile[]): ImageWithFile[] {
    const order = new Map(rankedIds.map((id, index) => [id, index]));
    return [...fetched].sort(
        (a, b) => (order.get(a.image.id) ?? Infinity) - (order.get(b.image.id) ?? Infinity),
    );
}

// Run a similarity search for `imageId` and replace the current view with the
// ranked results. Returns the number of similar images loaded.
export async function loadSimilarImages(imageId: string, topK = 20): Promise<number> {
    const results = await findSimilarImages(imageId, topK);
    const rankedIds = results.map(([id]) => id);
    if (rankedIds.length === 0) return 0;

    const fetched = await getImagesByIds(rankedIds);
    const similar = orderBySimilarity(rankedIds, fetched);
    if (similar.length > 0) {
        clearImageScope();
        resetImagePaging();
        images.set(similar);
        focusedIndex.set(0);
    }
    return similar.length;
}
