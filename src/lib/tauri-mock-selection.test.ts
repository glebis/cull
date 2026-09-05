import { beforeEach, describe, expect, it } from 'vitest';
import { invoke, listen } from './tauri-mock';

// Focused tests for the browser-only Selection Mode mock handlers in
// tauri-mock.ts (contract: docs/superpowers/plans/2026-09-05-selection-mode-implementation.md).
// They exercise the handlers through the same invoke seam the browser E2E uses.
// Durability across simulated page reloads lives in tauri-mock-selection-reload.test.ts.

interface SelectionRunView {
    id: string;
    name: string;
    status: 'active' | 'finished' | 'archived';
    source_count: number;
    shortlist_count: number;
    target_count: number | null;
    source_scope: unknown;
    created_at: string;
    updated_at: string;
    finished_at: string | null;
    rejected_shortlist_count: number;
}

interface SelectionStateView {
    run: SelectionRunView;
    shortlist_ids: string[];
}

interface SelectionPageView {
    items: Array<{
        image: { id: string; file_size: number; format: string };
        path: string;
        selection: { decision: string } | null;
        source_label: string | null;
        missing_at: string | null;
    }>;
    total: number;
}

const ALL_SCOPE = { type: 'all', include_rejected: false };
const SEARCH_SCOPE = { type: 'search', base: ALL_SCOPE, query: 'image-1' };

function resetFixture() {
    (globalThis as any).__CULL_E2E_SELECTION_RESET__();
}

function createRun(
    name = 'Client final',
    scope: unknown = ALL_SCOPE,
    targetCount: number | null = null,
): Promise<SelectionStateView> {
    return invoke<SelectionStateView>('create_selection_run', { name, sourceScope: scope, targetCount });
}

beforeEach(resetFixture);

describe('Selection Mode browser mock', () => {
    it('previews backend-resolved source counts for supported scopes', async () => {
        await expect(invoke<{ count: number }>('preview_selection_source', { sourceScope: ALL_SCOPE }))
            .resolves.toEqual({ count: 20 });
        await expect(invoke<{ count: number }>('preview_selection_source', { sourceScope: SEARCH_SCOPE }))
            .resolves.toEqual({ count: 11 });
        await expect(invoke<{ count: number }>('preview_selection_source', {
            sourceScope: {
                type: 'smart',
                id: 'preset-3',
                filter_json: '{"type":"rule","field":"decision","op":"eq","value":"accept"}',
                include_rejected: false,
            },
        })).resolves.toEqual({ count: 1 });
        await expect(invoke<{ count: number }>('preview_selection_source', {
            sourceScope: { type: 'import_batch', batch_id: 'missing', include_rejected: false },
        })).resolves.toEqual({ count: 0 });
    }, 20_000);

    it('creates an empty-start run and echoes the full SelectionRun contract', async () => {
        const state = await createRun('Client final', ALL_SCOPE, 5);
        expect(state.run).toMatchObject({
            name: 'Client final',
            status: 'active',
            source_count: 20,
            shortlist_count: 0,
            target_count: 5,
            finished_at: null,
            rejected_shortlist_count: 0,
        });
        expect(state.run.source_scope).toEqual(ALL_SCOPE);
        // Selection Mode always starts empty: highlighted ids are never seeded.
        expect(state.shortlist_ids).toEqual([]);
        const runs = await invoke<SelectionRunView[]>('list_selection_runs', { status: 'active' });
        expect(runs.map(run => run.id)).toContain(state.run.id);
        const fetched = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(fetched.run.id).toBe(state.run.id);
    }, 20_000);

    it('rejects starting from an empty resolved source with an actionable error', async () => {
        await expect(createRun('Empty', { type: 'import_batch', batch_id: 'missing', include_rejected: false }))
            .rejects.toThrow(/empty/i);
    }, 20_000);

    it('captures the complete ordered snapshot of a search scope, not the visible page', async () => {
        const state = await createRun('Search run', SEARCH_SCOPE, null);
        expect(state.run.source_count).toBe(11);
        const page = await invoke<SelectionPageView>('list_selection_source', {
            selectionId: state.run.id, offset: 0, limit: 100,
        });
        expect(page.total).toBe(11);
        expect(page.items.map(item => item.image.id)).toEqual([
            'img-1', 'img-10', 'img-11', 'img-12', 'img-13', 'img-14', 'img-15', 'img-16', 'img-17', 'img-18', 'img-19',
        ]);
    }, 20_000);

    it('adds ordered groups atomically after validating every id', async () => {
        const state = await createRun();
        const afterAdd = await invoke<SelectionStateView>('add_to_shortlist', {
            selectionId: state.run.id,
            imageIds: ['img-7', 'img-3', 'img-11'],
        });
        // Membership preserves addition order instead of sorting.
        expect(afterAdd.shortlist_ids).toEqual(['img-7', 'img-3', 'img-11']);
        expect(afterAdd.run.shortlist_count).toBe(3);

        // One unknown id rolls the whole group back.
        await expect(invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-2', 'img-999'] }))
            .rejects.toThrow(/img-999/);
        const unchanged = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(unchanged.shortlist_ids).toEqual(['img-7', 'img-3', 'img-11']);
    }, 20_000);

    it('rejects ids outside the captured source', async () => {
        const state = await createRun('Search run', SEARCH_SCOPE, null);
        await expect(invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-3'] }))
            .rejects.toThrow(/outside the captured Selection source/);
        await expect(invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-10'] }))
            .resolves.toMatchObject({ shortlist_ids: ['img-10'] });
    }, 20_000);

    it('treats re-adding members and removing non-members as idempotent no-ops without undo records', async () => {
        const state = await createRun();
        await invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-7', 'img-3', 'img-11'] });
        const noOpAdd = await invoke<SelectionStateView>('add_to_shortlist', {
            selectionId: state.run.id, imageIds: ['img-7'],
        });
        expect(noOpAdd.shortlist_ids).toEqual(['img-7', 'img-3', 'img-11']);
        const noOpRemove = await invoke<SelectionStateView>('remove_from_shortlist', {
            selectionId: state.run.id, imageIds: ['img-4'],
        });
        expect(noOpRemove.shortlist_ids).toEqual(['img-7', 'img-3', 'img-11']);

        // The next undo reverts the real group add, proving the no-ops recorded nothing.
        await expect(invoke('undo')).resolves.toBe('selection_membership');
        const undone = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(undone.shortlist_ids).toEqual([]);
        await expect(invoke('redo')).resolves.toBe('selection_membership');
        const redone = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(redone.shortlist_ids).toEqual(['img-7', 'img-3', 'img-11']);
    }, 20_000);

    it('removes groups as one undoable operation', async () => {
        const state = await createRun();
        await invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-7', 'img-3', 'img-11'] });
        const after = await invoke<SelectionStateView>('remove_from_shortlist', {
            selectionId: state.run.id, imageIds: ['img-7', 'img-11'],
        });
        expect(after.shortlist_ids).toEqual(['img-3']);
        await expect(invoke('undo')).resolves.toBe('selection_membership');
        const undone = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(undone.shortlist_ids).toEqual(['img-7', 'img-3', 'img-11']);
    }, 20_000);

    it('injects membership save failures before mutating so the UI can roll back and retry', async () => {
        const state = await createRun();
        (globalThis as any).__CULL_E2E_SHORTLIST_FAILURES__ = 2;
        await expect(invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-5'] }))
            .rejects.toThrow(/injected E2E fault/);
        await expect(invoke('remove_from_shortlist', { selectionId: state.run.id, imageIds: ['img-5'] }))
            .rejects.toThrow(/injected E2E fault/);
        const unchanged = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(unchanged.shortlist_ids).toEqual([]);
        // Faults consumed: the retry path now succeeds.
        const retry = await invoke<SelectionStateView>('add_to_shortlist', {
            selectionId: state.run.id, imageIds: ['img-5'],
        });
        expect(retry.shortlist_ids).toEqual(['img-5']);
    }, 20_000);

    it('keeps decisions and original paths untouched by membership and lifecycle changes', async () => {
        const state = await createRun();
        const before = JSON.parse(JSON.stringify(await invoke<unknown[]>('list_images')));
        await invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-3', 'img-6'] });
        await invoke('remove_from_shortlist', { selectionId: state.run.id, imageIds: ['img-3'] });
        await invoke('finish_selection_run', { selectionId: state.run.id });
        await invoke('reopen_selection_run', { selectionId: state.run.id });
        await invoke('archive_selection_run', { selectionId: state.run.id });
        await invoke('restore_selection_run', { selectionId: state.run.id });
        const after = await invoke<unknown[]>('list_images');
        expect(JSON.parse(JSON.stringify(after))).toEqual(before);
    }, 20_000);

    it('reflects surviving library references in counts and keeps trash undo working', async () => {
        const state = await createRun();
        await invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-3', 'img-6'] });
        await invoke('trash_images_detailed', { imageIds: ['img-3'] });
        const afterTrash = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(afterTrash.run.source_count).toBe(19);
        expect(afterTrash.run.shortlist_count).toBe(1);
        expect(afterTrash.shortlist_ids).toEqual(['img-6']);
        // The shared undo command reverts the most recent operation first: the
        // trash, then the membership group. Both paths keep working together,
        // and shortlist membership survives the trash/undo round-trip.
        await expect(invoke('undo')).resolves.toBe('rating');
        const afterTrashUndo = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(afterTrashUndo.run.source_count).toBe(20);
        expect(afterTrashUndo.shortlist_ids).toEqual(['img-3', 'img-6']);
        await expect(invoke('undo')).resolves.toBe('selection_membership');
        const afterMembershipUndo = await invoke<SelectionStateView>('get_selection_run', { selectionId: state.run.id });
        expect(afterMembershipUndo.shortlist_ids).toEqual([]);
    }, 20_000);

    it('finishes, reopens, archives and restores with actionable lifecycle errors', async () => {
        const state = await createRun('Client final');
        const collectionId = state.run.id; // Finishing keeps the native project/run ID.

        // Finishing an empty shortlist is blocked; archiving is the alternative.
        await expect(invoke('finish_selection_run', { selectionId: state.run.id }))
            .rejects.toThrow(/empty shortlist/i);

        await invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-4'] });
        const finished = await invoke<SelectionStateView>('finish_selection_run', { selectionId: state.run.id });
        expect(finished.run.status).toBe('finished');
        expect(finished.run.finished_at).toBeTruthy();
        // Finishing exposes the result as a normal named collection.
        const collections = await invoke<Array<[string, string, number]>>('list_collections');
        expect(collections.find(([id]) => id === collectionId)).toEqual([collectionId, 'Client final', 1]);
        const collectionImages = await invoke<SelectionPageView['items']>('list_collection_images', { collectionId });
        expect(collectionImages.map(item => item.image.id)).toEqual(['img-4']);

        await expect(invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-8'] }))
            .rejects.toThrow(/finished/);

        const reopened = await invoke<SelectionStateView>('reopen_selection_run', { selectionId: state.run.id });
        expect(reopened.run.status).toBe('active');
        expect(reopened.run.finished_at).toBeNull();
        expect(reopened.shortlist_ids).toEqual(['img-4']);
        expect((await invoke<Array<[string, string, number]>>('list_collections')).find(([id]) => id === collectionId))
            .toBeUndefined();

        const archived = await invoke<SelectionStateView>('archive_selection_run', { selectionId: state.run.id });
        expect(archived.run.status).toBe('archived');
        const restoredToActive = await invoke<SelectionStateView>('restore_selection_run', { selectionId: state.run.id });
        expect(restoredToActive.run.status).toBe('active');

        await invoke('finish_selection_run', { selectionId: state.run.id });
        await invoke('archive_selection_run', { selectionId: state.run.id });
        const restoredToFinished = await invoke<SelectionStateView>('restore_selection_run', { selectionId: state.run.id });
        expect(restoredToFinished.run.status).toBe('finished');

        await invoke('archive_selection_run', { selectionId: state.run.id });
        await expect(invoke('reopen_selection_run', { selectionId: state.run.id }))
            .rejects.toThrow(/cannot be reopened/i);
        const finalRestored = await invoke<SelectionStateView>('restore_selection_run', { selectionId: state.run.id });
        expect(finalRestored.run.status).toBe('finished');
    }, 20_000);

    it('pages source and shortlist scopes with offset, limit and layered filters', async () => {
        const state = await createRun();
        await invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-19', 'img-5', 'img-12'] });
        const shortlistPage = await invoke<SelectionPageView>('list_selection_shortlist', {
            selectionId: state.run.id, offset: 1, limit: 1,
        });
        expect(shortlistPage.items.map(item => item.image.id)).toEqual(['img-5']);
        expect(shortlistPage.total).toBe(3);

        const sourcePage = await invoke<SelectionPageView>('list_selection_source', {
            selectionId: state.run.id, offset: 0, limit: 5, query: 'image-1',
        });
        expect(sourcePage.total).toBe(11);
        expect(sourcePage.items).toHaveLength(5);

        const tooBig = await invoke<SelectionPageView>('list_selection_source', {
            selectionId: state.run.id, offset: 0, limit: 5, minSize: 3_000_000,
        });
        expect(tooBig).toEqual({ items: [], total: 0 });

        const fullPage = await invoke<SelectionPageView>('list_selection_source', {
            selectionId: state.run.id, offset: 0, limit: 100,
        });
        expect(fullPage.items[0]).toMatchObject({
            image: { id: 'img-0' },
            path: '/mock/image-0.png',
            source_label: null,
            missing_at: null,
        });
    }, 20_000);

    it('emits selection-run:updated after mutations and membership undo/redo', async () => {
        const events: Array<{ run: { id: string; shortlist_count: number }; shortlist_ids: string[] }> = [];
        const unlisten = await listen('selection-run:updated', (event) => events.push(event.payload as never));
        try {
            const state = await createRun('Events');
            await invoke('add_to_shortlist', { selectionId: state.run.id, imageIds: ['img-2'] });
            await invoke('undo');
            const payloads = events.filter(item => item.run.id === state.run.id);
            expect(payloads.map(item => item.run.shortlist_count)).toEqual([0, 1, 0]);
        } finally {
            unlisten();
        }
    }, 20_000);

    it('returns actionable errors for unknown runs', async () => {
        await expect(invoke('get_selection_run', { selectionId: 'sel-nope' })).rejects.toThrow(/not found/i);
        await expect(invoke('add_to_shortlist', { selectionId: 'sel-nope', imageIds: ['img-1'] }))
            .rejects.toThrow(/not found/i);
    }, 20_000);
});
describe('shortlist proposal atomic approval', () => {
    async function proposal(selectionId: string, kind = 'shortlist_add') {
        return invoke<{ id: string }>('create_action_proposal', { request: {
            kind, persona: 'curator', criteria: 'Reviewed candidates', visual_level: 'text',
            source_context_json: JSON.stringify({ selection_id: selectionId }),
            items_json: JSON.stringify([{ image_id: 'img-3' }, { image_id: 'img-7' }]),
            guard_results_json: '{}',
        } });
    }

    it('applies only approved candidates to the captured run even after another run starts', async () => {
        const captured = await createRun('Captured');
        const review = await proposal(captured.run.id);
        const current = await createRun('Current');
        await invoke('apply_action_proposal', { proposalId: review.id, approvedImageIds: ['img-7'], resultJson: '{}' });
        expect((await invoke<SelectionStateView>('get_selection_run', { selectionId: captured.run.id })).shortlist_ids).toEqual(['img-7']);
        expect((await invoke<SelectionStateView>('get_selection_run', { selectionId: current.run.id })).shortlist_ids).toEqual([]);
        await expect(invoke('apply_action_proposal', { proposalId: review.id, approvedImageIds: ['img-3'], resultJson: '{}' })).rejects.toThrow('not pending');
        const removal = await proposal(captured.run.id, 'shortlist_remove');
        await invoke('apply_action_proposal', { proposalId: removal.id, approvedImageIds: ['img-7'], resultJson: '{}' });
        expect((await invoke<SelectionStateView>('get_selection_run', { selectionId: captured.run.id })).shortlist_ids).toEqual([]);
    });

    it('keeps approval pending and membership unchanged on validation or save failure, then retries', async () => {
        const captured = await createRun();
        const review = await proposal(captured.run.id);
        const apply = (ids: string[]) => invoke('apply_action_proposal', { proposalId: review.id, approvedImageIds: ids, resultJson: '{}' });
        await expect(apply(['img-3', 'img-9'])).rejects.toThrow('reviewed proposal');
        (globalThis as any).__CULL_E2E_SHORTLIST_FAILURES__ = 1;
        await expect(apply(['img-3'])).rejects.toThrow('injected E2E fault');
        expect((await invoke<SelectionStateView>('get_selection_run', { selectionId: captured.run.id })).shortlist_ids).toEqual([]);
        expect(await invoke('list_action_proposals', { status: 'pending' })).toEqual([expect.objectContaining({ id: review.id, status: 'pending' })]);
        await apply(['img-3']);
        expect((await invoke<SelectionStateView>('get_selection_run', { selectionId: captured.run.id })).shortlist_ids).toEqual(['img-3']);
        expect(await invoke('list_action_proposals', { status: 'pending' })).toEqual([]);
    });
});
