import { afterEach, describe, expect, it, vi } from 'vitest';

// Durability tests for the Selection Mode browser mock: runs, ordered
// membership, and the membership undo journal must survive a page reload so
// browser E2E can verify the resume flow. Each scenario re-imports
// tauri-mock.ts against the same stubbed sessionStorage to simulate a reload.

interface SelectionStateView {
    run: {
        id: string;
        name: string;
        status: 'active' | 'finished' | 'archived';
        shortlist_count: number;
        source_count: number;
    };
    shortlist_ids: string[];
}

function memoryStorage() {
    const store = new Map<string, string>();
    return {
        store,
        getItem: (key: string) => (store.has(key) ? (store.get(key) as string) : null),
        setItem: (key: string, value: string) => void store.set(key, value),
        removeItem: (key: string) => void store.delete(key),
    };
}

const ALL_SCOPE = { type: 'all', include_rejected: false };

afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
});

describe('Selection Mode mock durability across page reloads', () => {
    it('resumes runs, ordered membership and undo history from session storage after a reload', async () => {
        vi.stubGlobal('sessionStorage', memoryStorage());

        const first = await import('./tauri-mock');
        const created = await first.invoke<SelectionStateView>('create_selection_run', {
            name: 'Resume me',
            sourceScope: ALL_SCOPE,
            targetCount: 3,
        });
        await first.invoke('add_to_shortlist', { selectionId: created.run.id, imageIds: ['img-9', 'img-4'] });

        // Simulate a page reload: fresh module instance, same session storage.
        vi.resetModules();
        const reloaded = await import('./tauri-mock');
        const resumed = await reloaded.invoke<SelectionStateView>('get_selection_run', {
            selectionId: created.run.id,
        });
        expect(resumed.run.name).toBe('Resume me');
        expect(resumed.run.status).toBe('active');
        expect(resumed.run.source_count).toBe(20);
        expect(resumed.shortlist_ids).toEqual(['img-9', 'img-4']);

        // Membership undo history also survives the reload.
        await expect(reloaded.invoke('undo')).resolves.toBe('selection_membership');
        const undone = await reloaded.invoke<SelectionStateView>('get_selection_run', {
            selectionId: created.run.id,
        });
        expect(undone.shortlist_ids).toEqual([]);
        await expect(reloaded.invoke('redo')).resolves.toBe('selection_membership');
        const redone = await reloaded.invoke<SelectionStateView>('get_selection_run', {
            selectionId: created.run.id,
        });
        expect(redone.shortlist_ids).toEqual(['img-9', 'img-4']);

        // A run created after the resume never collides with the persisted id.
        const second = await reloaded.invoke<SelectionStateView>('create_selection_run', {
            name: 'Second run',
            sourceScope: ALL_SCOPE,
            targetCount: null,
        });
        expect(second.run.id).not.toBe(created.run.id);
    }, 20_000);

    it('honours ?selectionFixture=reset to wipe stored state on a fresh page', async () => {
        const storage = memoryStorage();
        vi.stubGlobal('sessionStorage', storage);
        vi.stubGlobal('window', { location: new URL('http://localhost/app') });

        const first = await import('./tauri-mock');
        const created = await first.invoke<SelectionStateView>('create_selection_run', {
            name: 'Wipeable',
            sourceScope: ALL_SCOPE,
            targetCount: null,
        });
        expect(storage.getItem('cull-e2e-selection-fixture-v1')).toBeTruthy();

        vi.resetModules();
        vi.stubGlobal('window', { location: new URL('http://localhost/app?selectionFixture=reset') });
        const reloaded = await import('./tauri-mock');
        await expect(reloaded.invoke('get_selection_run', { selectionId: created.run.id }))
            .rejects.toThrow(/not found/i);
        expect(storage.store.size).toBe(0);
    }, 20_000);
});