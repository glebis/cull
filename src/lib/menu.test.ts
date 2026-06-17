import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

const mocks = vi.hoisted(() => ({
    dialogOpen: vi.fn(),
    importFolder: vi.fn(),
    importFiles: vi.fn(),
    listen: vi.fn(),
    loadAllImages: vi.fn(),
    loadImagesForCurrentScope: vi.fn(),
    loadImagesUntil: vi.fn(),
    openUrl: vi.fn(),
    updateMenuState: vi.fn(),
    getPreviewDisplayWebStreamStatus: vi.fn(),
    checkForUpdates: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: mocks.listen,
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: mocks.dialogOpen,
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
    openUrl: mocks.openUrl,
    openPath: vi.fn(),
    revealItemInDir: vi.fn(),
}));

vi.mock('./api', () => ({
    importFolder: mocks.importFolder,
    importFiles: mocks.importFiles,
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
    loadAllImages: mocks.loadAllImages,
    loadImagesForCurrentScope: mocks.loadImagesForCurrentScope,
    loadImagesUntil: mocks.loadImagesUntil,
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

    it('opens the command palette in commands mode from the native menu', async () => {
        let menuHandler: ((event: { payload: string }) => void) | undefined;
        mocks.listen.mockImplementation(async (_eventName, handler) => {
            menuHandler = handler as (event: { payload: string }) => void;
            return vi.fn();
        });

        const [{ initMenu }, { commandPaletteMode, commandPaletteOpen }] = await Promise.all([
            import('./menu'),
            import('./stores'),
        ]);

        await initMenu({ listenTimeoutMs: 50, retryDelayMs: 10 });
        await flushMicrotasks();

        menuHandler?.({ payload: 'command_palette' });

        expect(get(commandPaletteOpen)).toBe(true);
        expect(get(commandPaletteMode)).toBe('commands');
    });

    it('imports the selected folder from the native Import Folder menu action', async () => {
        let menuHandler: ((event: { payload: string }) => void) | undefined;
        mocks.listen.mockImplementation(async (_eventName, handler) => {
            menuHandler = handler as (event: { payload: string }) => void;
            return vi.fn();
        });

        const [{ initMenu }, stores] = await Promise.all([
            import('./menu'),
            import('./stores'),
        ]);

        mocks.dialogOpen.mockResolvedValue('/photos/new-import' as never);
        mocks.importFolder.mockResolvedValue({
            imported: 1,
            skipped: 0,
            errors: [],
            batch_id: null,
            image_ids: ['img-1'],
        } as never);
        mocks.loadImagesForCurrentScope.mockResolvedValue(undefined as never);

        await initMenu({ listenTimeoutMs: 50, retryDelayMs: 10 });
        await flushMicrotasks();

        menuHandler?.({ payload: 'import_folder' });
        await flushMicrotasks();

        expect(mocks.dialogOpen).toHaveBeenCalledWith({ directory: true });
        expect(mocks.importFolder).toHaveBeenCalledWith('/photos/new-import');
        expect(get(stores.activeFolder)).toBe('/photos/new-import');
        expect(get(stores.viewMode)).toBe('grid');
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

    it('opens the agent skills dialog when the native Help menu action fires', async () => {
        let handler: ((event: { payload: string }) => void) | undefined;
        mocks.listen.mockImplementation(async (_eventName, next) => {
            handler = next as (event: { payload: string }) => void;
            return vi.fn();
        });

        const [{ initMenu }, { agentSkillsOpen }] = await Promise.all([
            import('./menu'),
            import('./stores'),
        ]);

        void initMenu({ listenTimeoutMs: 50, retryDelayMs: 10 });
        await flushMicrotasks();

        handler?.({ payload: 'agent_skills' });

        expect(get(agentSkillsOpen)).toBe(true);
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

    it('toggles the Loupe histogram from the native View menu and syncs checked state', async () => {
        let handler: ((event: { payload: string }) => void) | undefined;
        mocks.listen.mockImplementation(async (_eventName, next) => {
            handler = next as (event: { payload: string }) => void;
            return vi.fn();
        });

        const [{ initMenu }, { showLoupeHistogram }] = await Promise.all([
            import('./menu'),
            import('./stores'),
        ]);

        void initMenu({ listenTimeoutMs: 50, retryDelayMs: 10 });
        await flushMicrotasks();

        expect(mocks.updateMenuState).toHaveBeenCalledWith(
            expect.objectContaining({
                showLoupeHistogram: false,
            })
        );

        handler?.({ payload: 'view_loupe_histogram' });
        await flushMicrotasks();

        expect(get(showLoupeHistogram)).toBe(true);
        expect(mocks.updateMenuState).toHaveBeenCalledWith(
            expect.objectContaining({
                showLoupeHistogram: true,
            })
        );
    });
});
