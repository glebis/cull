import {
    getScopedEmbeddingPage,
    listScopedImageIds,
    type EmbeddingPage,
} from './api';
import type { LibraryScope } from './library-scope';

export function getEmbeddingPageForScope(
    scope: LibraryScope,
    model: string,
    limit: number,
    offset: number,
): Promise<EmbeddingPage> {
    return getScopedEmbeddingPage(scope, model, limit, offset);
}

export async function getEmbeddingCountForScope(
    scope: LibraryScope,
    model: string,
): Promise<number> {
    const page = await getEmbeddingPageForScope(scope, model, 1, 0);
    return page.total;
}

export function listImageIdsForScope(scope: LibraryScope): Promise<string[]> {
    return listScopedImageIds(scope);
}
