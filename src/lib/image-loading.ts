import { get } from 'svelte/store';
import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSmartCollection,
    focusedIndex,
    imageLoadState,
    images,
    minSizeFilter,
    showMissing,
    totalCount,
} from './stores';
import {
    evaluateSmartCollection,
    getImageCount,
    listCollectionImages,
    listImagesByDetectedClass,
    listImages,
    listImagesByFolder,
    listImagesFiltered,
    type ImageWithFile,
} from './api';

export const IMAGE_PAGE_SIZE = 200;

type ImageScope =
    | { type: 'smart'; id: string; filterJson: string }
    | { type: 'collection'; id: string }
    | { type: 'detected-class'; className: string }
    | { type: 'folder'; folder: string; minSize: number }
    | { type: 'filtered'; minSize: number }
    | { type: 'all' };

interface PageResult {
    items: ImageWithFile[];
    rawCount: number;
}

let activeScopeKey = '';
let nextOffset = 0;
let hasMore = false;
let loading = false;
let loadingMore = false;
let requestSeq = 0;

function currentScope(): ImageScope {
    const smart = get(activeSmartCollection);
    if (smart?.filter_json) {
        return { type: 'smart', id: smart.id, filterJson: smart.filter_json };
    }

    const collection = get(activeCollection);
    if (collection) return { type: 'collection', id: collection };

    const detectedClass = get(activeDetectedClass);
    if (detectedClass) return { type: 'detected-class', className: detectedClass };

    const folder = get(activeFolder);
    const minSize = get(minSizeFilter);
    if (folder) return { type: 'folder', folder, minSize };
    if (minSize > 0) return { type: 'filtered', minSize };
    return { type: 'all' };
}

function scopeKey(scope: ImageScope): string {
    switch (scope.type) {
        case 'smart':
            return `smart:${scope.id}:${scope.filterJson}`;
        case 'collection':
            return `collection:${scope.id}`;
        case 'detected-class':
            return `detected-class:${scope.className}`;
        case 'folder':
            return `folder:${scope.folder}:${scope.minSize}`;
        case 'filtered':
            return `filtered:${scope.minSize}`;
        case 'all':
            return 'all';
    }
}

function applyMissingFilter(items: ImageWithFile[]): ImageWithFile[] {
    if (get(showMissing)) return items;
    return items.filter(img => !img.missing_at);
}

async function fetchPage(scope: ImageScope, offset: number, limit: number): Promise<PageResult> {
    switch (scope.type) {
        case 'smart': {
            const items = await evaluateSmartCollection(scope.filterJson, limit, offset);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
        case 'collection': {
            const items = await listCollectionImages(scope.id, limit, offset);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
        case 'detected-class': {
            const items = await listImagesByDetectedClass(scope.className, limit, offset);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
        case 'folder': {
            const items = await listImagesByFolder(scope.folder, limit, offset);
            const filtered = scope.minSize > 0
                ? items.filter(img => img.image.width >= scope.minSize && img.image.height >= scope.minSize)
                : items;
            return { items: applyMissingFilter(filtered), rawCount: items.length };
        }
        case 'filtered': {
            const items = await listImagesFiltered(scope.minSize, scope.minSize, limit, offset);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
        case 'all': {
            const items = await listImages(limit, offset);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
    }
}

function setLoadState() {
    imageLoadState.set({ loading, loadingMore, hasMore });
}

export function resetImagePaging() {
    activeScopeKey = '';
    nextOffset = 0;
    hasMore = false;
    loading = false;
    loadingMore = false;
    requestSeq++;
    setLoadState();
}

export function clearImageScope() {
    activeSmartCollection.set(null);
    activeCollection.set(null);
    activeDetectedClass.set(null);
    activeFolder.set(null);
    minSizeFilter.set(0);
}

export async function refreshImageCount() {
    totalCount.set(await getImageCount());
}

export async function loadAllImages(options: { resetFocus?: boolean } = {}) {
    clearImageScope();
    await loadImagesForCurrentScope(options);
}

export async function loadImagesForCurrentScope(options: { resetFocus?: boolean } = {}) {
    const resetFocus = options.resetFocus ?? true;
    const scope = currentScope();
    const key = scopeKey(scope);
    const seq = ++requestSeq;

    activeScopeKey = key;
    nextOffset = 0;
    hasMore = false;
    loading = true;
    loadingMore = false;
    setLoadState();

    refreshImageCount().catch(e => console.error('Failed to refresh image count:', e));

    try {
        const page = await fetchPage(scope, 0, IMAGE_PAGE_SIZE);
        if (seq !== requestSeq || key !== activeScopeKey) return;

        images.set(page.items);
        nextOffset = IMAGE_PAGE_SIZE;
        hasMore = page.rawCount === IMAGE_PAGE_SIZE;
        if (resetFocus) focusedIndex.set(0);
    } finally {
        if (seq === requestSeq && key === activeScopeKey) {
            loading = false;
            setLoadState();
        }
    }
}

export async function loadMoreImagesForCurrentScope() {
    const scope = currentScope();
    const key = scopeKey(scope);
    if (key !== activeScopeKey) {
        await loadImagesForCurrentScope({ resetFocus: false });
        return;
    }
    if (!hasMore || loading || loadingMore) return;

    const offset = nextOffset;
    const seq = requestSeq;
    loadingMore = true;
    setLoadState();

    try {
        const page = await fetchPage(scope, offset, IMAGE_PAGE_SIZE);
        if (seq !== requestSeq || key !== activeScopeKey) return;

        nextOffset += IMAGE_PAGE_SIZE;
        hasMore = page.rawCount === IMAGE_PAGE_SIZE;
        if (page.items.length > 0) {
            images.update(existing => {
                const seen = new Set(existing.map(img => img.image.id));
                const appended = page.items.filter(img => !seen.has(img.image.id));
                return appended.length > 0 ? [...existing, ...appended] : existing;
            });
        }
    } finally {
        if (seq === requestSeq && key === activeScopeKey) {
            loadingMore = false;
            setLoadState();
        }
    }
}

export async function loadImagesUntil(
    predicate: (image: ImageWithFile) => boolean,
    maxPages = 20,
): Promise<number> {
    for (let page = 0; page <= maxPages; page++) {
        const foundIndex = get(images).findIndex(predicate);
        if (foundIndex >= 0) return foundIndex;
        if (!hasMore || loading || loadingMore) return -1;
        await loadMoreImagesForCurrentScope();
    }
    return get(images).findIndex(predicate);
}
