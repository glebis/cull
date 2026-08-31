import { get } from 'svelte/store';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReferencedFolderPage, ReferencedSource } from './api';

const mocks = vi.hoisted(() => ({
    cancelReferencedSourceJob: vi.fn().mockResolvedValue(true),
    listen: vi.fn().mockResolvedValue(() => {}),
    listReferencedSources: vi.fn().mockResolvedValue([]),
    loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined),
    openReferencedFolder: vi.fn(),
}));

vi.mock('./api', () => ({
    cancelReferencedSourceJob: mocks.cancelReferencedSourceJob,
    listReferencedSources: mocks.listReferencedSources,
    openReferencedFolder: mocks.openReferencedFolder,
}));
vi.mock('./image-loading', () => ({
    loadImagesForCurrentScope: mocks.loadImagesForCurrentScope,
}));
vi.mock('@tauri-apps/api/event', () => ({
    listen: mocks.listen,
}));

let referenced: typeof import('./referenced-sources');
let stores: typeof import('./stores');

const source: ReferencedSource = {
    id: 'source-1',
    platform_volume_id: 'volume-1',
    display_name: 'CARD',
    last_mount_path: '/Volumes/CARD',
    source_kind: 'sd_card',
    capacity_bytes: 64_000_000_000,
    recursive_default: false,
    settings_json: '{}',
    last_seen_at: '2026-08-31T10:00:00Z',
    offline_at: null,
};

function page(jobId: string, relativePath: string): ReferencedFolderPage {
    return {
        job_id: jobId,
        source_id: source.id,
        relative_path: relativePath,
        requested_paths: [],
        image_ids: [],
        discovered_count: 0,
        next_cursor: null,
        indexing: true,
    };
}

function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
    return { promise, reject, resolve };
}

beforeEach(async () => {
    vi.clearAllMocks();
    mocks.listen.mockResolvedValue(() => {});
    vi.resetModules();
    referenced = await import('./referenced-sources');
    stores = await import('./stores');
    referenced.referencedFolderPage.set(null);
    referenced.referencedSourceIndexing.set(false);
    stores.toasts.set([]);
});

describe('referenced source read supersession', () => {
    it('keeps the newer folder when an older open resolves last', async () => {
        const older = deferred<ReferencedFolderPage>();
        const newer = deferred<ReferencedFolderPage>();
        mocks.openReferencedFolder
            .mockReturnValueOnce(older.promise)
            .mockReturnValueOnce(newer.promise);

        const olderOpen = referenced.openReferencedSourceFolder(source, 'DCIM/OLD');
        const newerOpen = referenced.openReferencedSourceFolder(source, 'DCIM/NEW');
        newer.resolve(page('job-newer', 'DCIM/NEW'));
        await newerOpen;
        older.resolve(page('job-older', 'DCIM/OLD'));
        await olderOpen;

        expect(get(referenced.referencedFolderPage)?.job_id).toBe('job-newer');
        expect(mocks.cancelReferencedSourceJob).toHaveBeenCalledWith('job-older');
        expect(mocks.loadImagesForCurrentScope).toHaveBeenCalledTimes(1);
    });

    it('cancels the active device read before opening another folder', async () => {
        mocks.openReferencedFolder
            .mockResolvedValueOnce(page('job-active', 'DCIM/OLD'))
            .mockResolvedValueOnce(page('job-next', 'DCIM/NEW'));

        await referenced.openReferencedSourceFolder(source, 'DCIM/OLD');
        await referenced.openReferencedSourceFolder(source, 'DCIM/NEW');

        expect(mocks.cancelReferencedSourceJob).toHaveBeenCalledWith('job-active');
        const activeCancellation = mocks.cancelReferencedSourceJob.mock.calls
            .findIndex(([jobId]) => jobId === 'job-active');
        expect(mocks.cancelReferencedSourceJob.mock.invocationCallOrder[activeCancellation])
            .toBeLessThan(mocks.openReferencedFolder.mock.invocationCallOrder[1]);
    });

    it('does not surface an older read failure over a newer request', async () => {
        const older = deferred<ReferencedFolderPage>();
        const newer = deferred<ReferencedFolderPage>();
        mocks.openReferencedFolder
            .mockReturnValueOnce(older.promise)
            .mockReturnValueOnce(newer.promise);

        const olderOpen = referenced.openReferencedSourceFolder(source, 'DCIM/OLD');
        const newerOpen = referenced.openReferencedSourceFolder(source, 'DCIM/NEW');
        older.reject(new Error('stale read failed'));

        await expect(olderOpen).resolves.toBeUndefined();
        expect(get(referenced.referencedSourceIndexing)).toBe(true);
        expect(get(stores.toasts)).toEqual([]);

        newer.resolve(page('job-newer', 'DCIM/NEW'));
        await newerOpen;
    });

    it('ignores the cancelled job event while its replacement is opening', async () => {
        await referenced.initializeReferencedSources();
        mocks.openReferencedFolder.mockResolvedValueOnce(page('job-old', 'DCIM/OLD'));
        await referenced.openReferencedSourceFolder(source, 'DCIM/OLD');
        mocks.loadImagesForCurrentScope.mockClear();

        const replacement = deferred<ReferencedFolderPage>();
        mocks.openReferencedFolder.mockReturnValueOnce(replacement.promise);
        const replacementOpen = referenced.openReferencedSourceFolder(source, 'DCIM/NEW');
        await vi.waitFor(() => expect(mocks.openReferencedFolder).toHaveBeenCalledTimes(2));

        const pageListener = mocks.listen.mock.calls
            .find(([eventName]) => eventName === 'referenced-source:page-updated')?.[1];
        await pageListener({
            payload: {
                job_id: 'job-old',
                source_id: source.id,
                relative_path: 'DCIM/OLD',
                image_ids: [],
                completed: true,
                cancelled: true,
                error: null,
            },
        });

        expect(get(referenced.referencedSourceIndexing)).toBe(true);
        expect(mocks.loadImagesForCurrentScope).not.toHaveBeenCalled();

        replacement.resolve(page('job-new', 'DCIM/NEW'));
        await replacementOpen;
    });
});
