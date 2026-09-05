// Loaded only by the packaged, isolated native smoke build. These calls use the
// real Tauri backend and the smoke app identifier, never the user's library.
import { invoke } from '@tauri-apps/api/core';
import type { ImageWithFile } from './api';

type State = {
    run: { id: string; status: string; source_count: number; shortlist_count: number };
    shortlist_ids: string[];
};
type Page = { items: ImageWithFile[]; total: number };
type Marker = { id: string; imageId: string; originals: string };
const MARKER_KEY = 'native_smoke_selection_restart';
const SOURCE = { type: 'all', include_rejected: true };

function assert(condition: unknown, message: string): asserts condition {
    if (!condition) throw new Error(`Selection Mode native smoke: ${message}`);
}

function originalState(items: ImageWithFile[]): string {
    return JSON.stringify(items.map(item => ({
        id: item.image.id, path: item.path, selection: item.selection,
    })).sort((a, b) => a.id.localeCompare(b.id)));
}

async function sourcePage(id: string): Promise<Page> {
    return invoke('list_selection_source', {
        selectionId: id, offset: 0, limit: 200, includeRejected: true,
    });
}

export async function runNativeSelectionPersistenceSmoke(): Promise<{ resumed: boolean; completed: string[] }> {
    const saved = await invoke<string | null>('get_app_setting', { key: MARKER_KEY });
    if (!saved) {
        const preview = await invoke<{ count: number }>('preview_selection_source', { sourceScope: SOURCE });
        assert(preview.count === 2, 'isolated source should contain both seeded images');
        const state = await invoke<State>('create_selection_run', {
            name: 'Native restart shortlist', sourceScope: SOURCE, targetCount: 1,
        });
        assert(state.run.source_count === 2, 'creation lost source images');
        assert(state.shortlist_ids.length === 0, 'a new shortlist must start empty');
        const page = await sourcePage(state.run.id);
        assert(page.total === 2 && page.items.length === 2, 'source page must contain both fixtures');
        const imageId = page.items[0].image.id;
        const updated = await invoke<State>('add_to_shortlist', { selectionId: state.run.id, imageIds: [imageId] });
        assert(updated.shortlist_ids.length === 1 && updated.shortlist_ids[0] === imageId, 'membership was not saved');
        const originals = originalState(page.items);
        assert(originalState((await sourcePage(state.run.id)).items) === originals, 'shortlisting changed decisions or paths');
        await invoke('set_app_setting', {
            key: MARKER_KEY, value: JSON.stringify({ id: state.run.id, imageId, originals } satisfies Marker),
        });
        return { resumed: false, completed: ['selection-empty-start', 'selection-full-source', 'selection-shortlist-save'] };
    }

    const marker = JSON.parse(saved) as Marker;
    const state = await invoke<State>('get_selection_run', { selectionId: marker.id });
    assert(state.run.status === 'active', 'active run did not survive restart');
    assert(state.run.source_count === 2, 'source snapshot did not survive restart');
    assert(state.shortlist_ids.length === 1 && state.shortlist_ids[0] === marker.imageId, 'shortlist did not survive restart');
    assert(originalState((await sourcePage(marker.id)).items) === marker.originals, 'restart changed decisions or file paths');

    const removed = await invoke<State>('remove_from_shortlist', { selectionId: marker.id, imageIds: [marker.imageId] });
    assert(removed.shortlist_ids.length === 0, 'removal did not apply');
    assert(await invoke<string | null>('undo') !== null, 'membership removal did not record undo');
    const undone = await invoke<State>('get_selection_run', { selectionId: marker.id });
    assert(undone.shortlist_ids.includes(marker.imageId), 'undo did not restore membership');
    assert(await invoke<string | null>('redo') !== null, 'membership undo did not permit redo');
    assert((await invoke<State>('get_selection_run', { selectionId: marker.id })).shortlist_ids.length === 0, 'redo did not remove membership');
    await invoke('add_to_shortlist', { selectionId: marker.id, imageIds: [marker.imageId] });

    const finished = await invoke<State>('finish_selection_run', { selectionId: marker.id });
    assert(finished.run.status === 'finished' && finished.shortlist_ids.length === 1, 'finish lost membership');
    const collections = await invoke<[string, string, number][]>('list_collections');
    assert(collections.some(collection => collection[0] === marker.id), 'finished run is not a normal collection');
    const reopened = await invoke<State>('reopen_selection_run', { selectionId: marker.id });
    assert(reopened.run.status === 'active' && reopened.shortlist_ids.includes(marker.imageId), 'reopen lost shortlist identity');
    await invoke('archive_selection_run', { selectionId: marker.id });
    const restored = await invoke<State>('restore_selection_run', { selectionId: marker.id });
    assert(restored.run.status === 'active' && restored.shortlist_ids.includes(marker.imageId), 'archive/restore lost membership');
    await invoke('finish_selection_run', { selectionId: marker.id });
    assert(originalState((await sourcePage(marker.id)).items) === marker.originals, 'lifecycle or undo changed decisions or file paths');
    return { resumed: true, completed: ['selection-restart-persistence', 'selection-undo-redo', 'selection-finish-collection', 'selection-archive-restore', 'selection-originals-unchanged'] };
}
