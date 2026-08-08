import {
    findSimilarImagesInScope,
    getImagesByIds,
    type ImageWithFile,
} from './api';
import type { LibraryScope } from './library-scope';

export interface EmbeddingNeighbor {
    image: ImageWithFile;
    score: number;
}

export async function loadEmbeddingNeighbors(
    scope: LibraryScope,
    imageId: string,
    model: string,
    limit = 6,
): Promise<EmbeddingNeighbor[]> {
    const ranked = await findSimilarImagesInScope(scope, imageId, limit, model);
    const images = await getImagesByIds(ranked.map(([id]) => id));
    const byId = new Map(images.map(image => [image.image.id, image]));
    return ranked.flatMap(([id, score]) => {
        const image = byId.get(id);
        return image ? [{ image, score }] : [];
    });
}
