import { get } from 'svelte/store';
import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSmartCollection,
    focusedIndex,
    gridScrollTop,
    imageLoadState,
    images,
    importBatchFilter,
    importBatchImageIds,
    minSizeFilter,
    showMissing,
    showRejected,
    totalCount,
} from './stores';
import {
    evaluateSmartCollection,
    getBatchImages,
    getImageCount,
    listCollectionImages,
    listImagesByDetectedClass,
    listImages,
    listImagesByFolder,
    listImagesFiltered,
    type ImageWithFile,
} from './api';
import { formatLibraryLoadError } from './library-view-state';
import { currentLibraryScope, libraryScopeKey, type LibraryScope } from './library-scope';

export const IMAGE_PAGE_SIZE = 200;
const MAX_SCOPE_CACHE_ENTRIES = 5;
const LIBRARY_LOAD_TIMEOUT_MS = 15_000;

export interface ImageLoadOptions {
    resetFocus?: boolean;
    force?: boolean;
    minItems?: number;
    invalidateCache?: boolean;
    throwOnError?: boolean;
}

interface PageResult {
    items: ImageWithFile[];
    rawCount: number;
}

interface CachedScopeState {
    items: ImageWithFile[];
    nextOffset: number;
    hasMore: boolean;
    focusedIndex: number;
    scrollTop: number;
}

async function withLibraryLoadTimeout<T>(operation: Promise<T>): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<never>((_, reject) => {
        timer = setTimeout(() => {
            reject(new Error('Library load timed out after 15 seconds'));
        }, LIBRARY_LOAD_TIMEOUT_MS);
    });

    try {
        return await Promise.race([operation, timeout]);
    } finally {
        if (timer !== undefined) clearTimeout(timer);
    }
}

let activeScopeKey = '';
let nextOffset = 0;
let hasMore = false;
let loading = false;
let loadingMore = false;
let loadError: string | null = null;
let loadedOnce = false;
let requestSeq = 0;
const scopeCache = new Map<string, CachedScopeState>();

function scopeKey(scope: LibraryScope): string {
    const missingKey = get(showMissing) ? 'with-missing' : 'without-missing';
    return `${libraryScopeKey(scope)}:${missingKey}`;
}

function applyMissingFilter(items: ImageWithFile[]): ImageWithFile[] {
    if (get(showMissing)) return items;
    return items.filter(img => !img.missing_at);
}

async function fetchPage(scope: LibraryScope, offset: number, limit: number): Promise<PageResult> {
    const includeRejected = scope.include_rejected;
    switch (scope.type) {
        case 'import_batch': {
            const items = offset === 0 ? await getBatchImages(scope.batch_id, includeRejected) : [];
            return { items: applyMissingFilter(items), rawCount: 0 };
        }
        case 'smart': {
            const items = await evaluateSmartCollection(scope.filter_json, limit, offset, includeRejected);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
        case 'collection': {
            const items = await listCollectionImages(scope.id, limit, offset, includeRejected);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
        case 'detected_class': {
            const items = await listImagesByDetectedClass(scope.class_name, limit, offset, includeRejected);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
        case 'folder': {
            const items = await listImagesByFolder(scope.path, limit, offset, includeRejected);
            const filtered = scope.min_size > 0
                ? items.filter(img => img.image.width >= scope.min_size && img.image.height >= scope.min_size)
                : items;
            return { items: applyMissingFilter(filtered), rawCount: items.length };
        }
        case 'filtered': {
            const items = await listImagesFiltered(scope.min_size, scope.min_size, limit, offset, includeRejected);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
        case 'all': {
            const items = await listImages(limit, offset, includeRejected);
            return { items: applyMissingFilter(items), rawCount: items.length };
        }
    }
}

function setLoadState() {
    imageLoadState.set({ loading, loadingMore, hasMore, error: loadError, loaded: loadedOnce });
}

function rememberScopeState(key = activeScopeKey) {
    if (!key) return;
    const cached: CachedScopeState = {
        items: get(images),
        nextOffset,
        hasMore,
        focusedIndex: get(focusedIndex),
        scrollTop: get(gridScrollTop),
    };
    scopeCache.delete(key);
    scopeCache.set(key, cached);
    while (scopeCache.size > MAX_SCOPE_CACHE_ENTRIES) {
        const oldest = scopeCache.keys().next().value;
        if (!oldest) break;
        scopeCache.delete(oldest);
    }
}

images.subscribe(() => {
    rememberScopeState();
});

focusedIndex.subscribe(() => {
    rememberScopeState();
});

gridScrollTop.subscribe(() => {
    rememberScopeState();
});

export function resetImagePaging() {
    activeScopeKey = '';
    nextOffset = 0;
    hasMore = false;
    loading = false;
    loadingMore = false;
    loadError = null;
    requestSeq++;
    setLoadState();
}

export function invalidateImageCache() {
    scopeCache.clear();
}

export function clearImageScope() {
    importBatchFilter.set(null);
    importBatchImageIds.set([]);
    activeSmartCollection.set(null);
    activeCollection.set(null);
    activeDetectedClass.set(null);
    activeFolder.set(null);
    minSizeFilter.set(0);
}

export async function refreshImageCount() {
    totalCount.set(await getImageCount(get(showRejected)));
}

export async function loadAllImages(options: ImageLoadOptions = {}) {
    clearImageScope();
    await loadImagesForCurrentScope(options);
}

export async function loadImagesForCurrentScope(options: ImageLoadOptions = {}) {
    const resetFocus = options.resetFocus ?? true;
    const force = options.force ?? false;
    const minItems = Math.max(0, options.minItems ?? 0);
    const scope = currentLibraryScope();
    const key = scopeKey(scope);
    const seq = ++requestSeq;

    if (options.invalidateCache) {
        invalidateImageCache();
    }

    activeScopeKey = key;
    nextOffset = 0;
    hasMore = false;
    loading = true;
    loadingMore = false;
    loadError = null;
    setLoadState();

    const cached = !force ? scopeCache.get(key) : undefined;
    if (cached && cached.items.length >= minItems) {
        nextOffset = cached.nextOffset;
        hasMore = cached.hasMore;
        images.set(cached.items);
        if (scope.type === 'import_batch') {
            importBatchImageIds.set(cached.items.map(item => item.image.id));
        }
        if (resetFocus) focusedIndex.set(cached.focusedIndex);
        gridScrollTop.set(cached.scrollTop);
        loading = false;
        loadedOnce = true;
        setLoadState();
        return;
    }

    refreshImageCount().catch(e => console.error('Failed to refresh image count:', e));

    try {
        const loaded: ImageWithFile[] = [];
        let offset = 0;
        let lastRawCount = 0;

        do {
            const page = await withLibraryLoadTimeout(fetchPage(scope, offset, IMAGE_PAGE_SIZE));
            if (seq !== requestSeq || key !== activeScopeKey) return;

            lastRawCount = page.rawCount;
            const seen = new Set(loaded.map(img => img.image.id));
            loaded.push(...page.items.filter(img => !seen.has(img.image.id)));
            offset += IMAGE_PAGE_SIZE;
        } while (lastRawCount === IMAGE_PAGE_SIZE && loaded.length < minItems);

        images.set(loaded);
        if (scope.type === 'import_batch') {
            importBatchImageIds.set(loaded.map(item => item.image.id));
        }
        nextOffset = offset;
        hasMore = lastRawCount === IMAGE_PAGE_SIZE;
        loadedOnce = true;
        if (resetFocus) {
            focusedIndex.set(0);
            gridScrollTop.set(0);
        }
        rememberScopeState(key);
    } catch (e) {
        const isCurrentRequest = seq === requestSeq && key === activeScopeKey;
        if (isCurrentRequest) {
            loadError = formatLibraryLoadError(e);
            console.error('Failed to load images:', e);
        }
        if (options.throwOnError && isCurrentRequest) throw e;
    } finally {
        if (seq === requestSeq && key === activeScopeKey) {
            loading = false;
            setLoadState();
        }
    }
}

export async function loadMoreImagesForCurrentScope() {
    const scope = currentLibraryScope();
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
        const page = await withLibraryLoadTimeout(fetchPage(scope, offset, IMAGE_PAGE_SIZE));
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
        rememberScopeState(key);
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
