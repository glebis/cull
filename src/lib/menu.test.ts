import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

const mocks = vi.hoisted(() => ({
    listen: vi.fn(),
    openUrl: vi.fn(),
    updateMenuState: vi.fn(),
    getPreviewDisplayWebStreamStatus: vi.fn(),
    checkForUpdates: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: mocks.listen,
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
    openUrl: mocks.openUrl,
    openPath: vi.fn(),
    revealItemInDir: vi.fn(),
}));

vi.mock('./api', () => ({
    importFolder: vi.fn(),
    importFiles: vi.fn(),
    redo: vi.fn(),
    undo: vi.fn(),
    moveImage: vi.fn(),
    listOpenWithApplications: vi.fn(),
    openImagesWithApplication: vi.fn(),
    renameImage: vi.fn(),
    shareImages: vi.fn(),
    trashImages: vi.fn(),
    listFolders: vi.fn(),
    updateMenuState: mocks.updateMenuState,
    openPreviewDisplay: vi.fn(),
    listPreviewDisplayMonitors: vi.fn(),
    placePreviewDisplay: vi.fn(),
    startPreviewDisplayWebStream: vi.fn(),
    stopPreviewDisplayWebStream: vi.fn(),
    getPreviewDisplayWebStreamStatus: mocks.getPreviewDisplayWebStreamStatus,
    setAppSetting: vi.fn(),
}));

vi.mock('./image-loading', () => ({
    loadAllImages: vi.fn(),
    loadImagesForCurrentScope: vi.fn(),
    loadImagesUntil: vi.fn(),
}));

vi.mock('./update-manager', () => ({
    checkForUpdates: mocks.checkForUpdates,
}));

function makeImage(id: string) {
    return {
        image: {
            id,
            sha256_hash: '',
            width: 100,
            height: 100,
            format: 'jpeg',
            file_size: 1000,
            created_at: '',
            imported_at: '',
            ai_prompt: null,
            raw_metadata: null,
        },
        source_label: null,
        path: `/photos/${id}.jpg`,
        thumbnail_path: null,
        selection: null,
        missing_at: null,
    };
}

async function flushMicrotasks() {
    await Promise.resolve();
    await Promise.resolve();
}

describe('native menu bridge', () => {
    beforeEach(() => {
        vi.resetModules();
        vi.useFakeTimers();
        vi.clearAllMocks();
        mocks.updateMenuState.mockResolvedValue(undefined);
        mocks.getPreviewDisplayWebStreamStatus.mockResolvedValue({
            active: false,
            url: null,
            host: null,
            bound_host: null,
            port: null,
            remote_access: false,
        });
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it('pushes menu state even when menu-action listener setup stalls', async () => {
        mocks.listen.mockReturnValue(new Promise(() => {}) as never);
        const [{ initMenu }, { focusedIndex, images }] = await Promise.all([
            import('./menu'),
            import('./stores'),
        ]);

        images.set([makeImage('img-1')]);
        focusedIndex.set(0);
        void initMenu({ listenTimeoutMs: 50, retryDelayMs: 10 });
        await flushMicrotasks();

        expect(mocks.updateMenuState).toHaveBeenCalledWith(
            expect.objectContaining({
                hasFocusedImage: true,
            })
        );
    });

    it('restarts the menu-action listener after setup times out', async () => {
        let restartedHandler: ((event: { payload: string }) => void) | undefined;
        mocks.listen
            .mockReturnValueOnce(new Promise(() => {}) as never)
            .mockImplementationOnce(async (_eventName, handler) => {
                restartedHandler = handler as (event: { payload: string }) => void;
                return vi.fn();
            });

        const [{ initMenu }, { viewMode }] = await Promise.all([
            import('./menu'),
            import('./stores'),
        ]);

        void initMenu({ listenTimeoutMs: 50, retryDelayMs: 10 });
        await flushMicrotasks();
        expect(mocks.listen).toHaveBeenCalledTimes(1);

        await vi.advanceTimersByTimeAsync(60);
        await vi.advanceTimersByTimeAsync(10);
        await flushMicrotasks();

        expect(mocks.listen).toHaveBeenCalledTimes(2);
        restartedHandler?.({ payload: 'view_loupe' });
        expect(get(viewMode)).toBe('loupe');
    });

    it('opens the GitHub wiki when the native Help menu action fires', async () => {
        let handler: ((event: { payload: string }) => void) | undefined;
        mocks.listen.mockImplementation(async (_eventName, next) => {
            handler = next as (event: { payload: string }) => void;
            return vi.fn();
        });

        const { initMenu } = await import('./menu');

        void initMenu({ listenTimeoutMs: 50, retryDelayMs: 10 });
        await flushMicrotasks();

        handler?.({ payload: 'github_wiki' });
        await flushMicrotasks();

        expect(mocks.openUrl).toHaveBeenCalledWith('https://github.com/glebis/cull/wiki');
    });

    it('runs a manual update check when the native Cull menu action fires', async () => {
        let handler: ((event: { payload: string }) => void) | undefined;
        mocks.listen.mockImplementation(async (_eventName, next) => {
            handler = next as (event: { payload: string }) => void;
            return vi.fn();
        });

        const { initMenu } = await import('./menu');

        void initMenu({ listenTimeoutMs: 50, retryDelayMs: 10 });
        await flushMicrotasks();

        handler?.({ payload: 'check_update' });
        await flushMicrotasks();

        expect(mocks.checkForUpdates).toHaveBeenCalledWith('manual');
    });
});
