// Selection Mode: build a shortlist from an explicit, backend-resolved
// source while browsing the full library. The database is canonical;
// `shortlistIds` is the immediate marker cache. Membership is independent of
// transient highlighted IDs (`selectedIds`) and of accept/reject decisions.

import { get, writable, type Writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import {
    activeCollection,
    activeSmartCollection,
    collections,
    focusedImage,
    navigateTo,
    requestConfirm,
    selectionRun,
    selectionScope,
    selectionScopeTotal,
    selectionSourceSearch,
    selectedIds,
    shortlistIds,
    showRejected,
    similarityViewActive,
    smartCollections,
    statusHint,
    viewMode,
    type ViewMode,
} from './stores';
import {
    addToShortlist,
    archiveSelectionRun,
    createSelectionRun,
    finishSelectionRun,
    getSelectionRun,
    listCollections,
    listSelectionRuns,
    removeFromShortlist,
    reopenSelectionRun,
    restoreSelectionRun,
    type SelectionRun,
    type SelectionState,
} from './api';
import { currentLibraryScope, type LibraryScope, type SelectionSourceScope } from './library-scope';
import {
    clearImageScope,
    loadImagesForCurrentScope,
    resetImagePaging,
} from './image-loading';
import {
    invalidateSelectionCache,
    loadSelectionScope,
    markSelectionCachesStale,
    markSelectionShortlistStale,
    resetSelectionPaging,
    switchSelectionScope,
} from './selection-view';
import { showToast } from './stores';

export type { SelectionRun, SelectionState } from './api';
export type { SelectionSourceScope } from './library-scope';

// ---------------------------------------------------------------------------
// Announcement feedback (live region in the mode bar; no focus movement)
// ---------------------------------------------------------------------------

export const selectionAnnouncement: Writable<string | null> = writable(null);

let announcementTimer: ReturnType<typeof setTimeout> | undefined;
export function announceSelection(text: string) {
    selectionAnnouncement.set(text);
    statusHint.set(text);
    if (announcementTimer) clearTimeout(announcementTimer);
    announcementTimer = setTimeout(() => selectionAnnouncement.set(null), 6000);
}

function plural(count: number, word: string): string {
    return `${count} ${word}${count === 1 ? '' : 's'}`;
}

// ---------------------------------------------------------------------------
// Start availability: only resolvable scopes may capture a source. Transient
// views are disabled with an actionable explanation instead of silently
// capturing the wrong image set.
// ---------------------------------------------------------------------------

export interface SelectionStartAvailability {
    available: boolean;
    reason: string | null;
    scope: SelectionSourceScope | null;
    label: string;
}

const UNSTARTABLE_VIEW_REASONS: Partial<Record<ViewMode, string>> = {
    canvas: 'Canvas view is a working board, not a resolvable image scope. Switch to Grid, or pick a folder, collection, search, or All Images, then start the selection.',
    embeddings: 'The embedding explorer has its own scope. Pick a folder, collection, search, or All Images, then start the selection.',
    lineage: 'Lineage view shows generation lineage. Pick a folder, collection, search, or All Images, then start the selection.',
};

function basename(path: string): string {
    const parts = path.split('/').filter(Boolean);
    return parts[parts.length - 1] ?? path;
}

export function scopeLabel(scope: LibraryScope): string {
    switch (scope.type) {
        case 'import_batch':
            return 'Import batch';
        case 'smart': {
            const saved = get(smartCollections).find(item => item.id === scope.id);
            return saved?.name ?? 'Saved search';
        }
        case 'collection': {
            const found = get(collections).find(item => item[0] === scope.id);
            return found?.[1] ?? 'Collection';
        }
        case 'detected_class':
            return `Detected: ${scope.class_name}`;
        case 'folder':
            return basename(scope.path);
        case 'referenced_folder':
            return basename(scope.relative_path) || 'Referenced folder';
        case 'filtered':
            return `Images \u2265 ${scope.min_size}px`;
        case 'all':
            return 'All Images';
    }
}

export function selectionStartAvailability(): SelectionStartAvailability {
    const run = get(selectionRun);
    if (run && run.status === 'active') {
        return {
            available: false,
            reason: `\u201C${run.name}\u201D is already active \u2014 finish, leave, or archive it before starting another selection.`,
            scope: null,
            label: run.name,
        };
    }
    if (get(similarityViewActive)) {
        return {
            available: false,
            reason: 'Similarity results are a temporary view that cannot be captured as a source. Pick a folder, collection, search, or All Images instead.',
            scope: null,
            label: 'Similar results',
        };
    }
    const mode = get(viewMode);
    const viewReason = UNSTARTABLE_VIEW_REASONS[mode];
    if (viewReason) {
        return {
            available: false,
            reason: viewReason,
            scope: null,
            label: mode.charAt(0).toUpperCase() + mode.slice(1),
        };
    }

    const scope = currentLibraryScope();
    // CommandBar parses natural language into filter_json. Capture that exact
    // resolved scope, including its priority relative to other active filters.
    const adhoc = get(activeSmartCollection);
    if (scope.type === 'smart' && adhoc?.id === '__adhoc__') {
        const query = adhoc.nl_query?.trim() || adhoc.name.trim();
        return { available: true, reason: null, scope, label: `Search “${query}”` };
    }
    return { available: true, reason: null, scope, label: scopeLabel(scope) };
}

// ---------------------------------------------------------------------------
// Run lifecycle
// ---------------------------------------------------------------------------

function enterRun(state: SelectionState) {
    membershipContext = null;
    // Detach library paging first: it bumps the request sequence so any
    // in-flight library load cannot clobber the selection view afterwards.
    resetImagePaging();
    // Detaches selection paging but keeps cached Source/Shortlist entries so
    // leaving and resuming the same run restores each scope's own view.
    resetSelectionPaging();
    selectionRun.set(state.run);
    shortlistIds.set(new Set(state.shortlist_ids));
    selectionScope.set('source');
    selectionScopeTotal.set(null);
    setSelectionSourceSearchQuiet(null);
    void loadSelectionScope({ resetFocus: true });
    navigateTo('grid');
}

async function detachModeAndRestoreLibrary() {
    selectionRun.set(null);
    shortlistIds.set(new Set());
    selectionScope.set('source');
    selectionScopeTotal.set(null);
    setSelectionSourceSearchQuiet(null);
    membershipContext = null;
    resetSelectionPaging();
    await loadImagesForCurrentScope({ resetFocus: false });
}

export async function startSelectionRun(name: string, targetCount: number | null): Promise<SelectionState> {
    const availability = selectionStartAvailability();
    if (!availability.available || !availability.scope) {
        throw new Error(availability.reason ?? 'Selection cannot start from this view.');
    }
    const trimmed = name.trim();
    if (!trimmed) throw new Error('Give the selection a name before starting.');
    if (targetCount !== null && (!Number.isFinite(targetCount) || Math.trunc(targetCount) < 1)) {
        throw new Error('Target count must be a positive number.');
    }
    const state = await createSelectionRun(
        trimmed,
        availability.scope,
        targetCount === null ? null : Math.trunc(targetCount),
    );
    enterRun(state);
    announceSelection(`Selection \u201C${state.run.name}\u201D started with an empty shortlist. Source ${state.run.source_count}.`);
    return state;
}

/** Startup restore: re-open the most recent active run so its chrome and
 *  views return coherently after an app restart. */
export async function restoreMostRecentActiveRun(): Promise<SelectionRun | null> {
    const runs = await listSelectionRuns('active');
    if (!runs || runs.length === 0) return null;
    const mostRecent = runs.reduce((a, b) => (a.updated_at >= b.updated_at ? a : b));
    const state = await getSelectionRun(mostRecent.id);
    enterRun(state);
    announceSelection(`Selection \u201C${state.run.name}\u201D resumed. Shortlist ${state.run.shortlist_count} of source ${state.run.source_count}.`);
    return state.run;
}

/** Resume a run chosen from the resume dialog: active runs reopen as-is,
 *  finished runs continue under the same identity, archived runs are
 *  restored first. */
export async function resumeSelectionRun(selectionId: string): Promise<SelectionState> {
    const state = await getSelectionRun(selectionId);
    if (state.run.status === 'finished') {
        return continueFinishedRun(selectionId);
    }
    if (state.run.status === 'archived') {
        const restored = await restoreSelectionRun(selectionId);
        enterRun(restored);
        announceSelection(`Selection \u201C${restored.run.name}\u201D restored from the archive.`);
        return restored;
    }
    enterRun(state);
    announceSelection(`Selection \u201C${state.run.name}\u201D resumed.`);
    return state;
}

async function continueFinishedRun(selectionId: string): Promise<SelectionState> {
    const reopened = await reopenSelectionRun(selectionId);
    enterRun(reopened);
    announceSelection(`Selection \u201C${reopened.run.name}\u201D reopened. Membership is unchanged.`);
    return reopened;
}

/** \u201CContinue as Selection\u201D on a collection: reopens the run that produced it
 *  instead of creating a new empty run or duplicating membership. */
export async function continueSelectionFromCollection(collectionId: string): Promise<boolean> {
    let state: SelectionState;
    try {
        state = await getSelectionRun(collectionId);
    } catch {
        showToast('This collection is not a selection result', {
            detail: 'Only collections produced by Finish Selection can continue as a selection.',
            type: 'warning',
            duration: 8000,
        });
        return false;
    }
    if (state.run.status === 'finished') {
        await continueFinishedRun(state.run.id);
        return true;
    }
    if (state.run.status === 'archived') {
        const restored = await restoreSelectionRun(state.run.id);
        enterRun(restored);
        announceSelection(`Archived selection \u201C${restored.run.name}\u201D restored and reopened.`);
        return true;
    }
    enterRun(state);
    announceSelection(`Selection \u201C${state.run.name}\u201D resumed.`);
    return true;
}

/** Leave the mode without changing the run. It stays active and resumable. */
export async function leaveSelectionMode(): Promise<void> {
    const run = get(selectionRun);
    if (!run) return;
    announceSelection(`Left Selection Mode. \u201C${run.name}\u201D stays active and can be resumed.`);
    await detachModeAndRestoreLibrary();
}

/** Finish: one backend transaction that exposes the shortlist as a normal
 *  manual collection. Decisions and files are never changed. */
export async function finishSelection(): Promise<SelectionState | null> {
    const run = get(selectionRun);
    if (!run) return null;
    if (run.shortlist_count === 0 && get(shortlistIds).size === 0) {
        showToast('The shortlist is empty \u2014 add images before finishing, or archive the selection instead.', { type: 'warning', duration: 8000 });
        return null;
    }
    const state = await finishSelectionRun(run.id);
    const finished = state.run;
    invalidateSelectionCache(finished.id);

    // Back to the library, then open the resulting collection (the run id is
    // the collection id after the finish transaction).
    selectionRun.set(null);
    shortlistIds.set(new Set());
    selectionScope.set('source');
    selectionScopeTotal.set(null);
    setSelectionSourceSearchQuiet(null);
    membershipContext = null;
    resetSelectionPaging();
    clearImageScope();
    activeCollection.set(finished.id);
    try {
        collections.set(await listCollections(get(showRejected)));
    } catch (e) {
        console.error('Failed to refresh collections after finishing selection:', e);
    }
    await loadImagesForCurrentScope({ resetFocus: true, force: true });
    navigateTo('grid');
    showToast(`Selection finished \u2014 collection \u201C${finished.name}\u201D created. No files or review decisions changed.`, { type: 'success', duration: 9000 });
    return state;
}

/** Archive: lifecycle-only change. Membership and history are retained and
 *  the run can be restored from the resume dialog. */
export async function archiveSelection(): Promise<SelectionState | null> {
    const run = get(selectionRun);
    if (!run) return null;
    const confirmed = await requestConfirm({
        title: 'Archive Selection',
        description: `\u201C${run.name}\u201D keeps its shortlist (${run.shortlist_count}) and history. You can restore it later from Resume Selection. Nothing is deleted.`,
        confirmLabel: 'Archive',
        cancelLabel: 'Keep Active',
    });
    if (!confirmed) return null;
    const state = await archiveSelectionRun(run.id);
    invalidateSelectionCache(run.id);
    announceSelection(`Selection \u201C${state.run.name}\u201D archived. Restore it any time from Resume Selection.`);
    await detachModeAndRestoreLibrary();
    return state;
}

// ---------------------------------------------------------------------------
// Scope search (CommandBar constrained to the stable source)
// ---------------------------------------------------------------------------

function setSelectionSourceSearchQuiet(query: string | null) {
    selectionSourceSearch.set(query && query.trim() ? query.trim() : null);
}

export async function setSelectionSourceSearch(query: string | null): Promise<void> {
    if (!get(selectionRun)) return;
    setSelectionSourceSearchQuiet(query);
    await loadSelectionScope({ resetFocus: true, force: true });
}

export async function switchSelectionModeScope(scope: 'source' | 'shortlist'): Promise<void> {
    await switchSelectionScope(scope);
}

// ---------------------------------------------------------------------------
// Membership mutations: optimistic markers, per-run intent serialization,
// captured-ID discipline, rollback with retry.
// ---------------------------------------------------------------------------

type MembershipOp = 'add' | 'remove';

interface MembershipContext {
    runId: string;
    canonicalIds: Set<string>;
    pending: Map<symbol, { op: MembershipOp; imageIds: string[] }>;
}
let membershipContext: MembershipContext | null = null;
// Leaving or switching runs detaches outstanding responses from the next view.
selectionRun.subscribe(run => {
    if (!run || run.id !== membershipContext?.runId) membershipContext = null;
});

function renderMembership(context: MembershipContext) {
    if (membershipContext !== context || get(selectionRun)?.id !== context.runId) return;
    const ids = new Set(context.canonicalIds);
    for (const { op, imageIds } of context.pending.values()) {
        for (const id of imageIds) {
            if (op === 'add') ids.add(id);
            else ids.delete(id);
        }
    }
    shortlistIds.set(ids);
}
const mutationQueues = new Map<string, Promise<unknown>>();

function enqueueMutation<T>(runId: string, op: () => Promise<T>): Promise<T> {
    const previous = mutationQueues.get(runId) ?? Promise.resolve();
    const next = previous.then(op, op);
    mutationQueues.set(runId, next.catch(() => { }));
    return next;
}

/** Apply a canonical SelectionState to the marker cache without discarding
 *  the optimistic effect of mutations still queued behind it. Late responses
 *  for a different (finished/resumed) run are dropped. */
function adoptSelectionState(state: SelectionState, expectedRunId: string) {
    const current = get(selectionRun);
    if (!current || current.id !== expectedRunId) return;
    if (membershipContext?.runId === expectedRunId) {
        membershipContext.canonicalIds = new Set(state.shortlist_ids);
        renderMembership(membershipContext);
    } else {
        shortlistIds.set(new Set(state.shortlist_ids));
    }
    selectionRun.set(state.run);
}

function membershipErrorDetail(e: unknown): string {
    const raw = e instanceof Error ? e.message : String(e ?? 'unknown error');
    return `The change could not be saved: ${raw}`;
}

async function mutateMembership(
    runId: string,
    op: MembershipOp,
    imageIds: string[],
): Promise<void> {
    if (imageIds.length === 0) return;
    const context = membershipContext ??= {
        runId,
        canonicalIds: new Set(get(shortlistIds)),
        pending: new Map(),
    };
    const token = Symbol();
    context.pending.set(token, { op, imageIds });
    renderMembership(context);
    announceSelection(op === 'add'
        ? `Adding ${plural(imageIds.length, 'image')} to shortlist\u2026`
        : `Removing ${plural(imageIds.length, 'image')} from shortlist\u2026`);

    try {
        const state = await enqueueMutation(runId, () =>
            op === 'add'
                ? addToShortlist(runId, imageIds)
                : removeFromShortlist(runId, imageIds),
        );
        context.pending.delete(token);
        markSelectionShortlistStale(runId);
        if (membershipContext !== context) return;
        adoptSelectionState(state, runId);
        // Shortlist list content changed: flag the cached shortlist pages for
        // refetch while keeping each scope's remembered view position, and
        // refresh immediately if the Shortlist view is open.
        if (get(selectionScope) === 'shortlist' && get(selectionRun)?.id === runId) {
            void loadSelectionScope({ resetFocus: false, force: true });
        }
        const run = state.run;
        const targetSuffix = run.target_count !== null ? ` of target ${run.target_count}` : '';
        if (op === 'add') {
            announceSelection(`Added ${plural(imageIds.length, 'image')} to shortlist. Shortlist ${run.shortlist_count}${targetSuffix}.`);
        } else {
            announceSelection(`Removed ${plural(imageIds.length, 'image')} from shortlist. Shortlist ${run.shortlist_count}${targetSuffix}.`);
        }
    } catch (e) {
        context.pending.delete(token);
        if (membershipContext === context && get(selectionRun)?.id === runId) {
            // Restore the last confirmed membership, then replay every remaining
            // intent. Inverting a failed remove can invent a member when an
            // earlier queued add also failed.
            renderMembership(context);
            announceSelection('Shortlist change failed. Nothing was changed.');
            showToast('Could not update the shortlist', {
                detail: `${membershipErrorDetail(e)} Nothing was changed. Retry keeps the same images.`,
                type: 'error',
                duration: 12000,
                actions: [{
                    label: 'Retry',
                    onclick: () => void retryMembership(runId, op, imageIds),
                }],
            });
        }
    }
}

function retryMembership(runId: string, op: MembershipOp, imageIds: string[]) {
    const run = get(selectionRun);
    if (!run || run.id !== runId || run.status !== 'active') {
        showToast('The selection is no longer active, so the change was not retried.', { type: 'warning', duration: 8000 });
        return;
    }
    void mutateMembership(runId, op, imageIds);
}

/** True while an active Selection Mode run owns the view. Keyboard and
 *  palette call sites use this to route Space and group commands. */
export function isSelectionModeActive(): boolean {
    const run = get(selectionRun);
    return run !== null && run.status === 'active';
}

/** Space: toggle the focused image's shortlist membership. */
export function toggleShortlistFocused(): void {
    const image = get(focusedImage);
    if (!image) return;
    toggleShortlistMembership(image.image.id);
}

export function toggleShortlistMembership(imageId: string): void {
    const run = get(selectionRun);
    if (!run || run.status !== 'active') return;
    const adding = !get(shortlistIds).has(imageId);
    void mutateMembership(run.id, adding ? 'add' : 'remove', [imageId]);
}

/** Explicit group command: adds the highlighted set captured at invocation
 *  time. The captured IDs never change after an await. */
export function addHighlightedToShortlist(): void {
    const run = get(selectionRun);
    if (!run || run.status !== 'active') return;
    const capturedIds = [...get(selectedIds)]; // captured synchronously
    if (capturedIds.length === 0) return;
    const current = get(shortlistIds);
    const toAdd = capturedIds.filter(id => !current.has(id));
    if (toAdd.length === 0) {
        announceSelection(`All ${plural(capturedIds.length, 'highlighted image')} are already shortlisted.`);
        return;
    }
    void mutateMembership(run.id, 'add', toAdd);
}

export function removeHighlightedFromShortlist(): void {
    const run = get(selectionRun);
    if (!run || run.status !== 'active') return;
    const capturedIds = [...get(selectedIds)]; // captured synchronously
    if (capturedIds.length === 0) return;
    const current = get(shortlistIds);
    const toRemove = capturedIds.filter(id => current.has(id));
    if (toRemove.length === 0) {
        announceSelection(`None of the ${plural(capturedIds.length, 'highlighted image')} are shortlisted.`);
        return;
    }
    void mutateMembership(run.id, 'remove', toRemove);
}

// ---------------------------------------------------------------------------
// Backend event: selection-run:updated fires after mutations and undo/redo,
// including changes made outside the current component (undo history, other
// surfaces). Events for other runs never touch the open one.
// ---------------------------------------------------------------------------

interface SelectionRunUpdatedPayload {
    run?: SelectionRun;
    shortlist_ids?: string[];
    run_id?: string;
    selection_id?: string;
}

/** Test/inspection seam: the selection-run:updated handler. Exported so the
 *  undo/redo refresh behavior can be exercised without a live event bus. */
export function handleSelectionRunUpdated(payload: SelectionRunUpdatedPayload | null) {
    const current = get(selectionRun);
    if (!current) return;
    const payloadId = payload?.run?.id ?? payload?.run_id ?? payload?.selection_id;
    if (!payloadId || payloadId !== current.id) return;

    if (payload?.run && Array.isArray(payload.shortlist_ids)) {
        adoptSelectionState(payload as SelectionState, current.id);
    } else {
        void getSelectionRun(current.id)
            .then(state => adoptSelectionState(state, current.id))
            .catch(e => console.error('Failed to refresh selection run after update event:', e));
    }
    // Undo/redo of membership changes the Shortlist list content: flag its
    // cached pages stale (memory kept) and refresh while it is open.
    markSelectionShortlistStale(current.id);
    if (get(selectionScope) === 'shortlist') {
        void loadSelectionScope({ resetFocus: false, force: true });
    }
}

export function initSelectionModeEvents(): Promise<() => void> {
    return listen<SelectionRunUpdatedPayload>('selection-run:updated', (event) => {
        handleSelectionRunUpdated(event.payload ?? null);
    });
}

/** Backend undo/redo can change ratings and decisions while Selection Mode
 *  owns the view (the ordinary reload-images path is intentionally paused).
 *  Refetch the current scope's pages and flag every cache stale so no stale
 *  review state survives a Source/Shortlist switch. Remembered focus and
 *  scroll are kept. */
export async function refreshSelectionViewAfterLibraryUndo(): Promise<void> {
    if (!isSelectionModeActive()) return;
    const run = get(selectionRun);
    markSelectionCachesStale(run?.id);
    await loadSelectionScope({ resetFocus: false, force: true });
}