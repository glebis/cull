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

export async function getImageCountForScope(scope: LibraryScope): Promise<number> {
    const page = await listScopedImageIds(scope, 1, 0);
    return page.total;
}

export async function listImageIdsForScope(scope: LibraryScope): Promise<string[]> {
    const ids: string[] = [];
    let offset = 0;
    while (true) {
        const page = await listScopedImageIds(scope, 100, offset);
        ids.push(...page.ids);
        if (!page.has_more) return ids;
        offset += page.ids.length;
        if (page.ids.length === 0) {
            throw new Error('Scoped image pagination returned an empty page before completion');
        }
    }
}
