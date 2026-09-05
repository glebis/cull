// Paged loading for Selection Mode scopes. Mirrors the paging and per-scope
// view memory in image-loading.ts, but the two Selection scopes (Source and
// Shortlist) keep their own caches, focus and scroll positions, and the pages
// come from the backend-resolved selection commands rather than the library
// scope resolver.
//
// The shared `images`/`focusedIndex`/`gridScrollTop` stores are filled so
// Grid, Loupe, Compare, ratings, decisions and accessibility behavior behave
// exactly as they do for library scopes.

import { get } from 'svelte/store';
import {
    focusedIndex,
    gridScrollTop,
    imageLoadState,
    images,
    minSizeFilter,
    selectionRun,
    selectionScope,
    selectionScopeTotal,
    selectionSourceSearch,
    showMissing,
    showRejected,
} from './stores';
import {
    listSelectionShortlist,
    listSelectionSource,
    type ImageWithFile,
    type SelectionPageFilters,
} from './api';
import { IMAGE_PAGE_SIZE } from './image-loading';

export { IMAGE_PAGE_SIZE as SELECTION_PAGE_SIZE };

const MAX_SCOPE_CACHE_ENTRIES = 5;

export interface SelectionLoadOptions {
    resetFocus?: boolean;
    force?: boolean;
    minItems?: number;
}

interface CachedSelectionScope {
    items: ImageWithFile[];
    nextOffset: number;
    hasMore: boolean;
    focusedIndex: number;
    scrollTop: number;
    total: number;
    /** True when the cached items may no longer match the backend (decision
     *  changes, membership changes made elsewhere). The remembered focus and
     *  scroll stay valid — only the content is refetched. */
    stale?: boolean;
}

let activeKey = '';
let nextOffset = 0;
let hasMore = false;
let loading = false;
let loadingMore = false;
let loadError: string | null = null;
let loadedOnce = false;
let requestSeq = 0;
const scopeCache = new Map<string, CachedSelectionScope>();

function currentRunId(): string | null {
    const run = get(selectionRun);
    return run && run.status === 'active' ? run.id : null;
}

function applyMissingFilter(items: ImageWithFile[]): ImageWithFile[] {
    if (get(showMissing)) return items;
    return items.filter(img => !img.missing_at);
}

function filtersForCurrentScope(): SelectionPageFilters {
    const scope = get(selectionScope);
    // The Shortlist view must keep showing every member — a shortlisted image
    // that was later rejected stays visible with both states. Only the Source
    // view layers the ordinary rejected/missing/size filters and text search.
    if (scope === 'shortlist') {
        return { includeRejected: true };
    }
    const query = get(selectionSourceSearch)?.trim() || null;
    const minSize = get(minSizeFilter) > 0 ? get(minSizeFilter) : null;
    return {
        query,
        minSize,
        includeRejected: get(showRejected),
    };
}

export function selectionScopeKey(): string {
    const runId = currentRunId();
    const scope = get(selectionScope);
    if (!runId) return '';
    if (scope === 'shortlist') return `selection:${runId}:shortlist`;
    const filters = filtersForCurrentScope();
    const missingKey = get(showMissing) ? 'with-missing' : 'without-missing';
    const rejectedKey = filters.includeRejected ? 'with-rejected' : 'without-rejected';
    const minSizeKey = filters.minSize ?? 0;
    const queryKey = filters.query ?? '';
    return `selection:${runId}:source:${missingKey}:${rejectedKey}:${minSizeKey}:${queryKey}`;
}

function rememberScopeState(key = activeKey) {
    if (!key) return;
    const cached: CachedSelectionScope = {
        items: get(images),
        nextOffset,
        hasMore,
        focusedIndex: get(focusedIndex),
        scrollTop: get(gridScrollTop),
        total: get(selectionScopeTotal) ?? 0,
    };
    scopeCache.delete(key);
    scopeCache.set(key, cached);
    while (scopeCache.size > MAX_SCOPE_CACHE_ENTRIES) {
        const oldest = scopeCache.keys().next().value;
        if (!oldest) break;
        scopeCache.delete(oldest);
    }
}

images.subscribe(() => rememberScopeState());
focusedIndex.subscribe(() => rememberScopeState());
gridScrollTop.subscribe(() => rememberScopeState());

function setLoadState() {
    imageLoadState.set({ loading, loadingMore, hasMore, error: loadError, loaded: loadedOnce });
}

/** Detach paging from the view. Cached entries survive so leaving and
 *  resuming a run restores Source and Shortlist independently. */
export function resetSelectionPaging() {
    activeKey = '';
    nextOffset = 0;
    hasMore = false;
    loading = false;
    loadingMore = false;
    loadError = null;
    requestSeq++;
}

/** True when the current scope key has a cached view (so a scope switch can
 *  restore its remembered focus and scroll instead of resetting them). */
export function hasCachedSelectionScope(): boolean {
    return scopeCache.has(selectionScopeKey());
}

export function invalidateSelectionCache(runId?: string) {
    if (!runId) {
        scopeCache.clear();
        return;
    }
    for (const key of [...scopeCache.keys()]) {
        if (key.startsWith(`selection:${runId}:`)) scopeCache.delete(key);
    }
}

/** Flag cached content for refetch while keeping each scope's remembered
 *  focus and scroll. Used when decisions change: membership, include-rejected
 *  filtering, and the displayed decisions are all backend-derived. */
export function markSelectionCachesStale(runId?: string) {
    for (const [key, cached] of [...scopeCache.entries()]) {
        if (runId && !key.startsWith(`selection:${runId}:`)) continue;
        scopeCache.set(key, { ...cached, stale: true });
    }
}

/** Flag only the shortlist pages of one run: membership changed elsewhere
 *  (undo/redo, proposals). Source pages are untouched. */
export function markSelectionShortlistStale(runId: string) {
    for (const [key, cached] of [...scopeCache.entries()]) {
        if (!key.startsWith(`selection:${runId}:shortlist`)) continue;
        scopeCache.set(key, { ...cached, stale: true });
    }
}

/** Repaint one image's review state inside every cached page (ratings do not
 *  affect membership or filtering, so no refetch is needed). */
export function updateSelectionCacheItem(imageId: string, update: (item: ImageWithFile) => ImageWithFile) {
    for (const [key, cached] of [...scopeCache.entries()]) {
        const index = cached.items.findIndex(item => item.image.id === imageId);
        if (index < 0) continue;
        const items = [...cached.items];
        items[index] = update(items[index]);
        scopeCache.set(key, { ...cached, items });
    }
}

async function fetchSelectionPage(
    runId: string,
    offset: number,
    limit: number,
    filters: SelectionPageFilters,
) {
    if (get(selectionScope) === 'shortlist') {
        return listSelectionShortlist(runId, offset, limit, filters);
    }
    return listSelectionSource(runId, offset, limit, filters);
}

/** Load (or restore) the current Selection scope into the shared view stores. */
export async function loadSelectionScope(options: SelectionLoadOptions = {}) {
    const runId = currentRunId();
    if (!runId) return;

    const resetFocus = options.resetFocus ?? true;
    const force = options.force ?? false;
    const minItems = Math.max(0, options.minItems ?? 0);
    const key = selectionScopeKey();
    if (!key) return;
    const seq = ++requestSeq;

    activeKey = key;
    nextOffset = 0;
    hasMore = false;
    loading = true;
    loadingMore = false;
    loadError = null;
    setLoadState();

    const cached = !force ? scopeCache.get(key) : undefined;
    if (cached && !cached.stale && cached.items.length >= minItems) {
        nextOffset = cached.nextOffset;
        hasMore = cached.hasMore;
        selectionScopeTotal.set(cached.total);
        images.set(cached.items);
        // A cached scope restores its own remembered view position regardless
        // of resetFocus: each scope's memory is the point of the cache.
        focusedIndex.set(cached.focusedIndex);
        gridScrollTop.set(cached.scrollTop);
        loading = false;
        loadedOnce = true;
        setLoadState();
        return;
    }

    try {
        const loaded: ImageWithFile[] = [];
        let offset = 0;
        let lastTotal = 0;
        let lastPageCount = 0;

        do {
            const page = await fetchSelectionPage(runId, offset, IMAGE_PAGE_SIZE, filtersForCurrentScope());
            // A different run, scope, or newer request owns the view now.
            if (seq !== requestSeq || key !== activeKey || currentRunId() !== runId) return;

            lastTotal = page.total;
            lastPageCount = page.items.length;
            const seen = new Set(loaded.map(img => img.image.id));
            loaded.push(...applyMissingFilter(page.items).filter(img => !seen.has(img.image.id)));
            offset += IMAGE_PAGE_SIZE;
        } while (offset < lastTotal && lastPageCount === IMAGE_PAGE_SIZE && loaded.length < minItems);

        selectionScopeTotal.set(lastTotal);
        images.set(loaded);
        nextOffset = offset;
        hasMore = offset < lastTotal && lastPageCount === IMAGE_PAGE_SIZE;
        loadedOnce = true;
        if (resetFocus) {
            focusedIndex.set(0);
            gridScrollTop.set(0);
        } else if (cached?.stale) {
            // The content was stale (decisions or membership changed while
            // this scope was closed): refetch it, but keep the remembered
            // view position instead of jumping back to the top.
            const maxIndex = Math.max(0, loaded.length - 1);
            focusedIndex.set(Math.min(cached.focusedIndex, maxIndex));
            gridScrollTop.set(cached.scrollTop);
        }
        rememberScopeState(key);
    } catch (e) {
        const isCurrent = seq === requestSeq && key === activeKey;
        if (isCurrent) {
            loadError = e instanceof Error ? e.message : String(e);
            console.error('Failed to load selection scope:', e);
        }
    } finally {
        if (seq === requestSeq && key === activeKey) {
            loading = false;
            setLoadState();
        }
    }
}

/** Grid infinite-scroll entry point: routes to the Selection loader while a
 *  run owns the view, otherwise to the ordinary library paging. */
export async function loadMoreImagesForCurrentView(pageSize = IMAGE_PAGE_SIZE) {
    if (currentRunId()) {
        await loadMoreSelectionImages(pageSize);
        return;
    }
    const { loadMoreImagesForCurrentScope } = await import('./image-loading');
    await loadMoreImagesForCurrentScope(pageSize);
}

export async function loadMoreSelectionImages(pageSize = IMAGE_PAGE_SIZE) {
    const runId = currentRunId();
    const key = selectionScopeKey();
    if (!runId) return;
    if (key !== activeKey) {
        await loadSelectionScope({ resetFocus: false });
        return;
    }
    if (!hasMore || loading || loadingMore) return;

    const offset = nextOffset;
    const seq = requestSeq;
    loadingMore = true;
    setLoadState();

    try {
        const normalizedPageSize = Math.max(1, Math.trunc(pageSize) || IMAGE_PAGE_SIZE);
        const page = await fetchSelectionPage(runId, offset, normalizedPageSize, filtersForCurrentScope());
        if (seq !== requestSeq || key !== activeKey || currentRunId() !== runId) return;

        nextOffset += normalizedPageSize;
        selectionScopeTotal.set(page.total);
        hasMore = nextOffset < page.total && page.items.length === normalizedPageSize;
        if (page.items.length > 0) {
            images.update(existing => {
                const seen = new Set(existing.map(img => img.image.id));
                const appended = applyMissingFilter(page.items).filter(img => !seen.has(img.image.id));
                return appended.length > 0 ? [...existing, ...appended] : existing;
            });
        }
        rememberScopeState(key);
    } catch (e) {
        console.error('Failed to load more selection images:', e);
    } finally {
        if (seq === requestSeq && key === activeKey) {
            loadingMore = false;
            setLoadState();
        }
    }
}

function lastTotalUpdate(total: number) {
    selectionScopeTotal.set(total);
}

/** Switch between the Source and Shortlist views, restoring each scope's own
 *  focus and scroll position. Never touches membership. */
export async function switchSelectionScope(scope: 'source' | 'shortlist') {
    if (get(selectionScope) === scope) return;
    selectionScope.set(scope);
    // A scope visited for the first time starts fresh at the top; a visited
    // scope restores its remembered focus and scroll.
    await loadSelectionScope({ resetFocus: !hasCachedSelectionScope() });
}