import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
    check: vi.fn(),
    relaunch: vi.fn(),
    getAppSetting: vi.fn(),
    setAppSetting: vi.fn(),
    showToast: vi.fn(),
    setInterval: vi.fn(),
    clearInterval: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-updater', () => ({
    check: mocks.check,
}));

vi.mock('@tauri-apps/plugin-process', () => ({
    relaunch: mocks.relaunch,
}));

vi.mock('./api', () => ({
    getAppSetting: mocks.getAppSetting,
    setAppSetting: mocks.setAppSetting,
}));

vi.mock('./stores', () => ({
    showToast: mocks.showToast,
}));

async function loadUpdateManager() {
    vi.resetModules();
    return import('./update-manager');
}

function makeUpdate(version = '0.2.0') {
    return {
        version,
        downloadAndInstall: vi.fn(async (onEvent?: (event: unknown) => void) => {
            onEvent?.({ event: 'Started', data: { contentLength: 2_097_152 } });
            onEvent?.({ event: 'Finished' });
        }),
    };
}

describe('update manager', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mocks.getAppSetting.mockResolvedValue(null);
        mocks.setAppSetting.mockResolvedValue(undefined);
        mocks.check.mockResolvedValue(null);
        mocks.relaunch.mockResolvedValue(undefined);
        mocks.setInterval.mockReturnValue(42);
    });

    it('manual checks notify when Cull is already current', async () => {
        const { checkForUpdates } = await loadUpdateManager();

        await checkForUpdates('manual');

        expect(mocks.check).toHaveBeenCalledOnce();
        expect(mocks.showToast).toHaveBeenCalledWith('Cull is up to date', {
            detail: 'No updates are available.',
            type: 'info',
            duration: 5000,
        });
    });

    it('manual checks treat a missing GitHub release feed as no update', async () => {
        mocks.check.mockRejectedValue(new Error('HTTP status client error (404 Not Found)'));
        const { checkForUpdates } = await loadUpdateManager();

        await checkForUpdates('manual');

        expect(mocks.showToast).toHaveBeenCalledWith('Cull is up to date', {
            detail: 'No updates are available.',
            type: 'info',
            duration: 5000,
        });
    });

    it('manual checks download an available update and offer immediate restart', async () => {
        const update = makeUpdate('0.2.0');
        mocks.check.mockResolvedValue(update);
        const { checkForUpdates } = await loadUpdateManager();

        await checkForUpdates('manual');

        expect(update.downloadAndInstall).toHaveBeenCalledOnce();
        expect(mocks.showToast).toHaveBeenLastCalledWith('Update ready to restart', {
            detail: 'Cull 0.2.0 has been installed. Restart Cull to finish updating.',
            type: 'success',
            duration: 30000,
            actions: [
                {
                    label: 'Restart',
                    onclick: expect.any(Function),
                },
            ],
        });

        const restartAction = mocks.showToast.mock.lastCall?.[1].actions[0].onclick;
        await restartAction();
        expect(mocks.relaunch).toHaveBeenCalledOnce();
    });

    it('auto updates are on by default, check at launch, and schedule a 24h cadence', async () => {
        let scheduled: (() => void | Promise<void>) | undefined;
        mocks.setInterval.mockImplementation((callback: () => void | Promise<void>) => {
            scheduled = callback;
            return 42;
        });
        const { AUTO_UPDATE_INTERVAL_MS, startAutoUpdates } = await loadUpdateManager();

        const cleanup = await startAutoUpdates({
            setInterval: mocks.setInterval,
            clearInterval: mocks.clearInterval,
        });

        expect(mocks.getAppSetting).toHaveBeenCalledWith('auto_update_enabled');
        expect(mocks.check).toHaveBeenCalledTimes(1);
        expect(mocks.setInterval).toHaveBeenCalledWith(expect.any(Function), AUTO_UPDATE_INTERVAL_MS);

        await scheduled?.();
        expect(mocks.check).toHaveBeenCalledTimes(2);

        cleanup();
        expect(mocks.clearInterval).toHaveBeenCalledWith(42);
    });

    it('auto updates can be turned off', async () => {
        mocks.getAppSetting.mockResolvedValue('false');
        const { startAutoUpdates } = await loadUpdateManager();

        const cleanup = await startAutoUpdates({
            setInterval: mocks.setInterval,
            clearInterval: mocks.clearInterval,
        });

        expect(mocks.check).not.toHaveBeenCalled();
        expect(mocks.setInterval).not.toHaveBeenCalled();
        cleanup();
        expect(mocks.clearInterval).not.toHaveBeenCalled();
    });
});
