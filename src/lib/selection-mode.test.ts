import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

import type { ImageWithFile, SelectionState } from './api';

// Selection Mode actions run against the real state modules with the Tauri
// boundary mocked — no runtime mocks of the state/actions themselves.
const apiMocks = vi.hoisted(() => ({
    previewSelectionSource: vi.fn(),
    createSelectionRun: vi.fn(),
    listSelectionRuns: vi.fn(),
    getSelectionRun: vi.fn(),
    listSelectionSource: vi.fn(),
    listSelectionShortlist: vi.fn(),
    addToShortlist: vi.fn(),
    removeFromShortlist: vi.fn(),
    finishSelectionRun: vi.fn(),
    reopenSelectionRun: vi.fn(),
    archiveSelectionRun: vi.fn(),
    restoreSelectionRun: vi.fn(),
    listCollections: vi.fn().mockResolvedValue([]),
    setRating: vi.fn().mockResolvedValue(undefined),
    setDecision: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('./api', () => {
    return {
        ...apiMocks,
        undo: vi.fn().mockResolvedValue(null),
        redo: vi.fn().mockResolvedValue(null),
        findSimilarImages: vi.fn().mockResolvedValue([]),
        getImagesByIds: vi.fn().mockResolvedValue([]),
        listImages: vi.fn().mockResolvedValue([]),
        getImageCount: vi.fn().mockResolvedValue(0),
        listImagesByFolder: vi.fn().mockResolvedValue([]),
        listImagesFiltered: vi.fn().mockResolvedValue([]),
        listCollectionImages: vi.fn().mockResolvedValue([]),
        listImagesByDetectedClass: vi.fn().mockResolvedValue([]),
        listImagesInReferencedFolder: vi.fn().mockResolvedValue([]),
        evaluateSmartCollection: vi.fn().mockResolvedValue([]),
        getBatchImages: vi.fn().mockResolvedValue([]),
    };
});

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockResolvedValue(() => {}),
}));

import {
    activeCollection,
    activeSmartCollection,
    focusedIndex,
    gridScrollTop,
    images,
    minSizeFilter,
    selectionRun,
    selectionScope,
    selectionScopeTotal,
    selectionSourceSearch,
    selectedIds,
    shortlistIds,
    showRejected,
    similarityViewActive,
    toasts,
    viewMode,
} from './stores';
import {
    addHighlightedToShortlist,
    archiveSelection,
    finishSelection,
    handleSelectionRunUpdated,
    isSelectionModeActive,
    leaveSelectionMode,
    removeHighlightedFromShortlist,
    restoreMostRecentActiveRun,
    setSelectionSourceSearch,
    startSelectionRun,
    toggleShortlistMembership,
    selectionStartAvailability,
    scopeLabel,
} from './selection-mode';
import {
    loadSelectionScope,
    resetSelectionPaging,
    invalidateSelectionCache,
    markSelectionShortlistStale,
    switchSelectionScope,
} from './selection-view';
import { updateSelectionCacheItem } from './selection-view';
import type { LibraryScope } from './library-scope';

function makeImage(id: string, overrides: Partial<ImageWithFile> = {}): ImageWithFile {
    return {
        image: {
            id,
            sha256_hash: `hash-${id}`,
            width: 100,
            height: 100,
            format: 'png',
            original_path: `/tmp/${id}.png`,
            file_size: 1024,
            created_at: '2026-09-05T00:00:00Z',
            imported_at: '2026-09-05T00:00:00Z',
            ai_prompt: null,
            raw_metadata: null,
        } as ImageWithFile['image'],
        path: `/tmp/${id}.png`,
        thumbnail_path: null,
        selection: null,
        source_label: null,
        missing_at: null,
        ...overrides,
    };
}

function makeRun(overrides: Record<string, unknown> = {}): SelectionState['run'] {
    return {
        id: 'run-1',
        name: 'Client final',
        status: 'active',
        source_count: 3,
        shortlist_count: 0,
        target_count: null,
        source_scope: { type: 'all', include_rejected: false },
        created_at: '2026-09-05T00:00:00Z',
        updated_at: '2026-09-05T00:00:00Z',
        finished_at: null,
        rejected_shortlist_count: 0,
        ...overrides,
    } as SelectionState['run'];
}

function makeState(runOverrides: Record<string, unknown> = {}, ids: string[] = []): SelectionState {
    return { run: makeRun(runOverrides), shortlist_ids: ids };
}

/** Deferred invoke control: hold a mutation until the test resolves it. */
function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, resolve, reject };
}

/** The per-run mutation queue invokes on a microtask; flush it. */
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function resetStores() {
    images.set([]);
    focusedIndex.set(0);
    gridScrollTop.set(0);
    selectedIds.set(new Set());
    selectionRun.set(null);
    shortlistIds.set(new Set());
    selectionScope.set('source');
    selectionScopeTotal.set(null);
    selectionSourceSearch.set(null);
    showRejected.set(false);
    minSizeFilter.set(0);
    similarityViewActive.set(false);
    activeCollection.set(null);
    activeSmartCollection.set(null);
    viewMode.set('grid');
    resetSelectionPaging();
    invalidateSelectionCache();
    toasts.set([]);
    vi.clearAllMocks();
}

beforeEach(resetStores);

describe('starting a selection run', () => {
    it('starts with an empty shortlist even when images are highlighted', async () => {
        selectedIds.set(new Set(['img-1', 'img-2', 'img-3']));
        // The backend starts every run with zero collection_items.
        apiMocks.createSelectionRun.mockResolvedValue(makeState({}, []));
        apiMocks.listSelectionSource.mockResolvedValue({ items: [], total: 0 });

        const state = await startSelectionRun('Client final', 5);

        expect(state.run.status).toBe('active');
        expect(get(shortlistIds).size).toBe(0);
        expect(get(selectionRun)?.id).toBe('run-1');
        expect(get(viewMode)).toBe('grid');
        expect(apiMocks.createSelectionRun).toHaveBeenCalledWith(
            'Client final',
            { type: 'all', include_rejected: false },
            5,
        );
    });

    it('captures the resolved scope, not the visible page', async () => {
        // Only one image happens to be loaded in the grid; the scope is what
        // the backend resolves, so createSelectionRun receives the scope.
        images.set([makeImage('visible-1')]);
        apiMocks.createSelectionRun.mockResolvedValue(makeState());

        await startSelectionRun('Shoot', null);

        expect(apiMocks.createSelectionRun).toHaveBeenCalledWith(
            'Shoot',
            { type: 'all', include_rejected: false },
            null,
        );
    });

    it('rejects an unresolvable scope with an actionable reason', async () => {
        viewMode.set('canvas');
        const availability = selectionStartAvailability();

        expect(availability.available).toBe(false);
        expect(availability.reason).toContain('Canvas');
        await expect(startSelectionRun('X', null)).rejects.toThrow('Canvas');
        expect(apiMocks.createSelectionRun).not.toHaveBeenCalled();
    });

    it('disables start while similarity results are shown', () => {
        similarityViewActive.set(true);
        const availability = selectionStartAvailability();

        expect(availability.available).toBe(false);
        expect(availability.reason).toContain('temporary view');
    });

    it('captures parsed ad-hoc filters for the backend beyond the loaded page', async () => {
        const filter_json = JSON.stringify({ type: 'rule', field: 'rating', op: 'eq', value: 5 });
        activeSmartCollection.set({
            id: '__adhoc__', name: '5 stars', nl_query: '5 stars', filter_json,
            description: null, collection_type: 'smart', is_preset: false,
            sort_order: 0, created_at: '', image_count: 450,
        });
        images.set(Array.from({ length: 200 }, (_, i) => makeImage(`visible-${i}`)));
        apiMocks.createSelectionRun.mockResolvedValue(makeState({ source_count: 450 }));

        const result = await startSelectionRun('Five stars', null);

        expect(apiMocks.createSelectionRun).toHaveBeenCalledWith('Five stars', {
            type: 'smart', id: '__adhoc__', filter_json, include_rejected: false,
        }, null);
        expect(result.run.source_count).toBe(450);
    });

    it('is unavailable while another run is active', async () => {
        selectionRun.set(makeRun());
        const availability = selectionStartAvailability();
        expect(availability.available).toBe(false);
        expect(availability.reason).toContain('already active');
        expect(isSelectionModeActive()).toBe(true);
    });
});

describe('membership mutations', () => {
    it('Space toggles the focused image id with optimistic markers and canonical adoption', async () => {
        images.set([makeImage('img-1'), makeImage('img-2')]);
        selectionRun.set(makeRun());
        shortlistIds.set(new Set());
        focusedIndex.set(0);

        let resolveAdd!: (value: SelectionState) => void;
        apiMocks.addToShortlist.mockImplementation(
            () => new Promise<SelectionState>((res) => { resolveAdd = res; }),
        );

        toggleShortlistMembership('img-1');
        await flush();

        // Optimistic: marker visible before the backend responds.
        expect(get(shortlistIds).has('img-1')).toBe(true);
        expect(apiMocks.addToShortlist).toHaveBeenCalledWith('run-1', ['img-1']);

        resolveAdd(makeState({ shortlist_count: 1 }, ['img-1']));
        await Promise.resolve();
        await Promise.resolve();

        expect(get(shortlistIds)).toEqual(new Set(['img-1']));
        expect(get(selectionRun)?.shortlist_count).toBe(1);
    });

    it('captured highlighted IDs never change after awaits', async () => {
        images.set([makeImage('img-1'), makeImage('img-2')]);
        selectionRun.set(makeRun());
        shortlistIds.set(new Set());
        selectedIds.set(new Set(['img-1', 'img-2']));

        let resolveAdd!: (value: SelectionState) => void;
        apiMocks.addToShortlist.mockImplementation(
            () => new Promise<SelectionState>((res) => { resolveAdd = res; }),
        );

        addHighlightedToShortlist();
        await flush();
        const capturedCall = apiMocks.addToShortlist.mock.calls[0];
        expect(capturedCall[1]).toEqual(['img-1', 'img-2']);

        // The user changes the highlight while the mutation is in flight.
        selectedIds.set(new Set(['img-2', 'img-3']));
        resolveAdd(makeState({ shortlist_count: 2 }, ['img-1', 'img-2']));
        await vi.waitFor(() => {
            expect(get(shortlistIds)).toEqual(new Set(['img-1', 'img-2']));
        });
        // The second mutation was never sent with the changed set.
        expect(apiMocks.addToShortlist).toHaveBeenCalledTimes(1);
    });

    it('rolls back optimistic markers and offers Retry on persistence failure', async () => {
        images.set([makeImage('img-1')]);
        selectionRun.set(makeRun());
        shortlistIds.set(new Set());

        apiMocks.addToShortlist.mockRejectedValueOnce(new Error('database is locked'));

        toggleShortlistMembership('img-1');
        await flush();
        await flush();
        await vi.waitFor(() => {
            // Truthful pending state resolved: nothing was changed.
            expect(get(shortlistIds).size).toBe(0);
            expect(get(selectionRun)?.shortlist_count).toBe(0);
        });

        // The error toast explains Retry and keeps the captured IDs.
        const toast = get(toasts).find(t => t.type === 'error');
        expect(toast?.message).toBe('Could not update the shortlist');
        expect(toast?.actions?.[0]?.label).toBe('Retry');

        apiMocks.addToShortlist.mockResolvedValueOnce(makeState({ shortlist_count: 1 }, ['img-1']));
        await toast!.actions![0].onclick();

        await vi.waitFor(() => {
            expect(get(shortlistIds)).toEqual(new Set(['img-1']));
        });
        expect(apiMocks.addToShortlist).toHaveBeenLastCalledWith('run-1', ['img-1']);
    });

    it('serializes mutation intent per run: the second op waits for the first', async () => {
        selectionRun.set(makeRun());
        shortlistIds.set(new Set());

        const calls: string[] = [];
        let resolveFirst!: (value: SelectionState) => void;
        apiMocks.addToShortlist.mockImplementationOnce(() => {
            calls.push('first-start');
            return new Promise<SelectionState>((res) => { resolveFirst = res; });
        });
        apiMocks.removeFromShortlist.mockImplementationOnce(async () => {
            calls.push('second-start');
            return makeState();
        });

        toggleShortlistMembership('img-1');
        toggleShortlistMembership('img-1'); // queued behind the first op

        await flush();
        expect(calls).toEqual(['first-start']);

        resolveFirst(makeState({ shortlist_count: 1 }, ['img-1']));
        await vi.waitFor(() => {
            expect(calls).toEqual(['first-start', 'second-start']);
        });
    });

    it('preserves the newest add while add/remove/add responses arrive', async () => {
        selectionRun.set(makeRun());
        const first = deferred<SelectionState>();
        const second = deferred<SelectionState>();
        const third = deferred<SelectionState>();
        apiMocks.addToShortlist.mockImplementationOnce(() => first.promise)
            .mockImplementationOnce(() => third.promise);
        apiMocks.removeFromShortlist.mockImplementationOnce(() => second.promise);
        toggleShortlistMembership('img-1');
        toggleShortlistMembership('img-1');
        toggleShortlistMembership('img-1');
        await flush();
        first.resolve(makeState({ shortlist_count: 1 }, ['img-1']));
        await flush();
        second.resolve(makeState());
        await flush();
        expect(get(shortlistIds)).toEqual(new Set(['img-1']));
        third.resolve(makeState({ shortlist_count: 1 }, ['img-1']));
        await flush();
    });

    it('does not invent membership when a queued add and remove both fail', async () => {
        selectionRun.set(makeRun());
        const add = deferred<SelectionState>();
        const remove = deferred<SelectionState>();
        apiMocks.addToShortlist.mockImplementationOnce(() => add.promise);
        apiMocks.removeFromShortlist.mockImplementationOnce(() => remove.promise);
        toggleShortlistMembership('img-1');
        toggleShortlistMembership('img-1');
        await flush();
        add.reject(new Error('add failed'));
        await flush();
        remove.reject(new Error('remove failed'));
        await flush();
        expect(get(shortlistIds)).toEqual(new Set());
        expect(get(selectionRun)?.shortlist_count).toBe(0);
    });

    it('keeps a new run pending marker when an old run responds for the same image', async () => {
        selectionRun.set(makeRun());
        const oldAdd = deferred<SelectionState>();
        const newAdd = deferred<SelectionState>();
        apiMocks.addToShortlist.mockImplementationOnce(() => oldAdd.promise)
            .mockImplementationOnce(() => newAdd.promise);
        toggleShortlistMembership('img-1');
        await flush();
        await leaveSelectionMode();
        selectionRun.set(makeRun({ id: 'run-2' }));
        toggleShortlistMembership('img-1');
        await flush();
        oldAdd.resolve(makeState({ shortlist_count: 1 }, ['img-1']));
        await flush();
        handleSelectionRunUpdated(makeState({ id: 'run-2' }));
        expect(get(shortlistIds)).toEqual(new Set(['img-1']));
        newAdd.resolve(makeState({ id: 'run-2', shortlist_count: 1 }, ['img-1']));
        await flush();
    });

    it('restores the last successful state when the next queued removal fails', async () => {
        selectionRun.set(makeRun());
        const add = deferred<SelectionState>();
        const remove = deferred<SelectionState>();
        apiMocks.addToShortlist.mockImplementationOnce(() => add.promise);
        apiMocks.removeFromShortlist.mockImplementationOnce(() => remove.promise);
        toggleShortlistMembership('img-1');
        toggleShortlistMembership('img-1');
        await flush();
        add.resolve(makeState({ shortlist_count: 1 }, ['img-1']));
        await flush();
        expect(get(shortlistIds)).toEqual(new Set());
        remove.reject(new Error('remove failed'));
        await flush();
        expect(get(shortlistIds)).toEqual(new Set(['img-1']));
        expect(get(selectionRun)?.shortlist_count).toBe(1);
    });

    it('removing a non-member and re-adding a member are no-ops on the wire', () => {
        selectionRun.set(makeRun());
        shortlistIds.set(new Set(['img-1']));
        selectedIds.set(new Set(['img-1']));
        apiMocks.removeFromShortlist.mockResolvedValue(makeState({}, ['img-1']));

        removeHighlightedFromShortlist();

        expect(apiMocks.removeFromShortlist).not.toHaveBeenCalled();
        apiMocks.addToShortlist.mockResolvedValue(makeState({}, ['img-1']));
        addHighlightedToShortlist();
        expect(apiMocks.addToShortlist).not.toHaveBeenCalled();
    });

    it('drops late canonical responses for a different or resumed run', async () => {
        images.set([makeImage('img-1')]);
        selectionRun.set(makeRun());
        shortlistIds.set(new Set());

        let resolveAdd!: (value: SelectionState) => void;
        apiMocks.addToShortlist.mockImplementation(
            () => new Promise<SelectionState>((res) => { resolveAdd = res; }),
        );

        toggleShortlistMembership('img-1');
        await flush();
        // The user leaves the mode (run cleared) before the response arrives.
        await leaveSelectionMode();

        resolveAdd(makeState({ shortlist_count: 1 }, ['img-1']));
        await Promise.resolve();
        await Promise.resolve();

        expect(get(selectionRun)).toBeNull();
    });
});

describe('backend undo events refresh membership', () => {
    it('adopts the canonical state for the open run and ignores other runs', () => {
        selectionRun.set(makeRun({ shortlist_count: 2 }, ));
        shortlistIds.set(new Set(['img-1']));

        handleSelectionRunUpdated({
            run: makeRun({ shortlist_count: 1 }),
            shortlist_ids: ['img-9'],
        });

        expect(get(shortlistIds)).toEqual(new Set(['img-9']));
        expect(get(selectionRun)?.shortlist_count).toBe(1);

        // An update belonging to a different run must not touch this view.
        handleSelectionRunUpdated({
            run: makeRun({ id: 'run-other', shortlist_count: 7 }),
            shortlist_ids: ['img-other'],
        });
        expect(get(shortlistIds)).toEqual(new Set(['img-9']));
    });

    it('refreshes by id when the event carries no full state', async () => {
        selectionRun.set(makeRun({ shortlist_count: 1 }));
        shortlistIds.set(new Set());
        apiMocks.getSelectionRun.mockResolvedValue(makeState({ shortlist_count: 1 }, ['img-2']));

        handleSelectionRunUpdated({ run_id: 'run-1' });

        await vi.waitFor(() => {
            expect(get(shortlistIds)).toEqual(new Set(['img-2']));
        });
        expect(apiMocks.getSelectionRun).toHaveBeenCalledWith('run-1');
    });
});

describe('resume, finish, archive and leave', () => {
    it('restores the most recent active run on startup', async () => {
        apiMocks.listSelectionRuns.mockResolvedValue([
            makeRun({ id: 'run-old', updated_at: '2026-09-01T00:00:00Z' }),
            makeRun({ id: 'run-new', updated_at: '2026-09-05T00:00:00Z' }),
        ]);
        apiMocks.getSelectionRun.mockResolvedValue(makeState({ id: 'run-new' }, ['img-1']));
        apiMocks.listSelectionSource.mockResolvedValue({ items: [], total: 0 });

        const run = await restoreMostRecentActiveRun();

        expect(run?.id).toBe('run-new');
        expect(apiMocks.getSelectionRun).toHaveBeenCalledWith('run-new');
        expect(get(selectionRun)?.id).toBe('run-new');
        expect(get(shortlistIds)).toEqual(new Set(['img-1']));
    });

    it('finishing exits the mode and opens the resulting manual collection', async () => {
        selectionRun.set(makeRun({ shortlist_count: 2 }));
        shortlistIds.set(new Set(['img-1', 'img-2']));
        apiMocks.finishSelectionRun.mockResolvedValue(
            makeState({ status: 'finished', shortlist_count: 2, finished_at: '2026-09-05T01:00:00Z' }, ['img-1', 'img-2']),
        );
        apiMocks.listCollections.mockResolvedValue([['run-1', 'Client final', 2]]);

        await finishSelection();

        expect(apiMocks.finishSelectionRun).toHaveBeenCalledWith('run-1');
        expect(get(selectionRun)).toBeNull();
        expect(get(activeCollection)).toBe('run-1');
        expect(get(viewMode)).toBe('grid');
    });

    it('archiving is confirmable and keeps the mode recoverable', async () => {
        selectionRun.set(makeRun());
        shortlistIds.set(new Set());
        apiMocks.archiveSelectionRun.mockResolvedValue(makeState({ status: 'archived' }));
        const { requestConfirm, confirmDialog } = await import('./stores');

        const archiving = archiveSelection();
        await flush();
        const request = get(confirmDialog);
        expect(request?.title).toBe('Archive Selection');
        expect(request?.description).toContain('restore it later');
        request!.resolve(true);

        await archiving;
        expect(apiMocks.archiveSelectionRun).toHaveBeenCalledWith('run-1');
        expect(get(selectionRun)).toBeNull();
        void requestConfirm;
    });

    it('leaving keeps the run resumable', async () => {
        selectionRun.set(makeRun());
        shortlistIds.set(new Set(['img-1']));
        apiMocks.listSelectionSource.mockResolvedValue({ items: [], total: 0 });

        await leaveSelectionMode();

        expect(get(selectionRun)).toBeNull();
        expect(apiMocks.archiveSelectionRun).not.toHaveBeenCalled();
        expect(apiMocks.finishSelectionRun).not.toHaveBeenCalled();
    });
});

describe('scoped source search and view memory', () => {
    it('passes the query and filters to the backend source loader', async () => {
        selectionRun.set(makeRun());
        showRejected.set(true);
        minSizeFilter.set(640);
        apiMocks.listSelectionSource.mockResolvedValue({ items: [makeImage('img-1')], total: 1 });

        await setSelectionSourceSearch('sunset');

        expect(apiMocks.listSelectionSource).toHaveBeenCalledWith(
            'run-1',
            0,
            200,
            { query: 'sunset', minSize: 640, includeRejected: true },
        );
        expect(get(images).map(i => i.image.id)).toEqual(['img-1']);
        expect(get(selectionScopeTotal)).toBe(1);
    });

    it('keeps every shortlist member visible regardless of the rejected filter', async () => {
        selectionRun.set(makeRun());
        showRejected.set(false);
        apiMocks.listSelectionShortlist.mockResolvedValue({
            items: [makeImage('img-1', { missing_at: null })],
            total: 1,
        });

        selectionScope.set('shortlist');
        await loadSelectionScope({ resetFocus: true });

        expect(apiMocks.listSelectionShortlist).toHaveBeenCalledWith(
            'run-1',
            0,
            200,
            { includeRejected: true },
        );
    });

    it('keeps Source and Shortlist focus and scroll independent', async () => {
        selectionRun.set(makeRun());
        const sourceItems = Array.from({ length: 5 }, (_, i) => makeImage(`src-${i}`));
        apiMocks.listSelectionSource.mockResolvedValue({ items: sourceItems, total: 5 });
        apiMocks.listSelectionShortlist.mockResolvedValue({ items: [], total: 0 });

        await loadSelectionScope({ resetFocus: true });
        focusedIndex.set(3);
        gridScrollTop.set(420);

        await switchSelectionScope('shortlist');
        expect(get(focusedIndex)).toBe(0);
        expect(get(gridScrollTop)).toBe(0);

        // Back to Source: cached focus and scroll are restored.
        await switchSelectionScope('source');
        expect(get(focusedIndex)).toBe(3);
        expect(get(gridScrollTop)).toBe(420);
        expect(get(shortlistIds)).toEqual(new Set());
    });

    it('paginates against the backend total instead of assuming the visible page is everything', async () => {
        selectionRun.set(makeRun({ source_count: 450 }));
        const pageOne = Array.from({ length: 200 }, (_, i) => makeImage(`p1-${i}`));
        apiMocks.listSelectionSource.mockResolvedValue({ items: pageOne, total: 450 });

        await loadSelectionScope({ resetFocus: true });

        expect(get(images)).toHaveLength(200);
        expect(apiMocks.listSelectionSource).toHaveBeenCalledTimes(1);
        const { loadMoreSelectionImages } = await import('./selection-view');
        const pageTwo = Array.from({ length: 200 }, (_, i) => makeImage(`p2-${i}`));
        apiMocks.listSelectionSource.mockResolvedValue({ items: pageTwo, total: 450 });

        await loadMoreSelectionImages();

        expect(get(images)).toHaveLength(400);
        expect(apiMocks.listSelectionSource).toHaveBeenLastCalledWith('run-1', 200, 200, expect.anything());
    });

    it('repaints ratings inside cached selection pages without losing view memory', async () => {
        selectionRun.set(makeRun());
        apiMocks.listSelectionSource.mockResolvedValue({
            items: [makeImage('img-1'), makeImage('img-2')],
            total: 2,
        });

        await loadSelectionScope({ resetFocus: true });
        focusedIndex.set(1);
        gridScrollTop.set(240);

        const { withRating } = await import('./selection-updates');
        updateSelectionCacheItem('img-2', item => withRating(item, 4));

        // Switching away and back restores the remembered position AND the
        // fresh rating — never the stale cached one.
        await switchSelectionScope('shortlist');
        await switchSelectionScope('source');
        expect(get(focusedIndex)).toBe(1);
        expect(get(gridScrollTop)).toBe(240);
        const restored = get(images).find(item => item.image.id === 'img-2');
        expect(restored?.selection?.star_rating).toBe(4);
    });

    it('refetches shortlist content after a membership change while keeping its remembered position', async () => {
        selectionRun.set(makeRun({ shortlist_count: 1 }));
        apiMocks.listSelectionShortlist.mockResolvedValue({
            items: [makeImage('img-1')],
            total: 1,
        });

        selectionScope.set('shortlist');
        await loadSelectionScope({ resetFocus: true });
        focusedIndex.set(0);
        gridScrollTop.set(90);

        // Membership changes while the Source view is open: the shortlist
        // cache is flagged stale, not erased.
        markSelectionShortlistStale('run-1');
        apiMocks.listSelectionShortlist.mockResolvedValue({
            items: [makeImage('img-1'), makeImage('img-2')],
            total: 2,
        });

        await switchSelectionScope('source');
        await switchSelectionScope('shortlist');

        // Fresh membership content…
        expect(get(images).map(item => item.image.id)).toEqual(['img-1', 'img-2']);
        // …with the remembered view position intact.
        expect(get(gridScrollTop)).toBe(90);
        expect(apiMocks.listSelectionShortlist).toHaveBeenCalledTimes(2);
    });
});

describe('scope labels and availability', () => {
    it('labels every resolvable scope kind', () => {
        expect(scopeLabel({ type: 'all', include_rejected: false } as LibraryScope)).toBe('All Images');
        expect(scopeLabel({ type: 'folder', path: '/Pictures/Shoot', min_size: 0, include_rejected: false } as LibraryScope)).toBe('Shoot');
        expect(scopeLabel({ type: 'collection', id: 'c1', include_rejected: false } as LibraryScope)).toBe('Collection');
        expect(scopeLabel({ type: 'detected_class', class_name: 'person', include_rejected: false } as LibraryScope)).toBe('Detected: person');
    });
});