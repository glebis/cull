import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { getAppSetting } from './api';
import { showToast } from './stores';

export const AUTO_UPDATE_ENABLED_KEY = 'auto_update_enabled';
export const AUTO_UPDATE_INTERVAL_MS = 24 * 60 * 60 * 1000;

export type UpdateCheckMode = 'manual' | 'auto';

interface UpdateRuntime {
    check: typeof check;
    relaunch: typeof relaunch;
    getAppSetting: typeof getAppSetting;
    showToast: typeof showToast;
    setInterval: typeof globalThis.setInterval;
    clearInterval: typeof globalThis.clearInterval;
}

type UpdateRuntimeOverrides = Partial<UpdateRuntime>;

type AutoUpdateTimer = ReturnType<typeof globalThis.setInterval>;

function runtime(overrides: UpdateRuntimeOverrides = {}): UpdateRuntime {
    return {
        check,
        relaunch,
        getAppSetting,
        showToast,
        setInterval: globalThis.setInterval.bind(globalThis),
        clearInterval: globalThis.clearInterval.bind(globalThis),
        ...overrides,
    };
}

export async function isAutoUpdateEnabled(overrides: UpdateRuntimeOverrides = {}): Promise<boolean> {
    const deps = runtime(overrides);
    return (await deps.getAppSetting(AUTO_UPDATE_ENABLED_KEY)) !== 'false';
}

function stringifyError(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
}

function isMissingReleaseFeed(error: unknown): boolean {
    const message = stringifyError(error).toLowerCase();
    return message.includes('404') || message.includes('not found');
}

function showNoUpdatesToast(deps: UpdateRuntime) {
    deps.showToast('Cull is up to date', {
        detail: 'No updates are available.',
        type: 'info',
        duration: 5000,
    });
}

async function downloadAndInstallUpdate(update: Update, deps: UpdateRuntime) {
    await update.downloadAndInstall();
    deps.showToast('Update ready to restart', {
        detail: `Cull ${update.version} has been installed. Restart Cull to finish updating.`,
        type: 'success',
        duration: 30000,
        actions: [
            {
                label: 'Restart',
                onclick: () => {
                    void deps.relaunch();
                },
            },
        ],
    });
}

export async function checkForUpdates(
    mode: UpdateCheckMode = 'manual',
    overrides: UpdateRuntimeOverrides = {}
) {
    const deps = runtime(overrides);

    try {
        const update = await deps.check();
        if (!update) {
            if (mode === 'manual') {
                showNoUpdatesToast(deps);
            }
            return;
        }

        await downloadAndInstallUpdate(update, deps);
    } catch (error) {
        if (mode === 'manual' && isMissingReleaseFeed(error)) {
            showNoUpdatesToast(deps);
            return;
        }

        if (mode === 'manual') {
            deps.showToast('Update check failed', {
                detail: stringifyError(error),
                type: 'error',
                duration: 8000,
            });
        } else {
            console.debug('Automatic update check failed:', error);
        }
    }
}

export async function startAutoUpdates(overrides: UpdateRuntimeOverrides = {}): Promise<() => void> {
    const deps = runtime(overrides);
    if (!(await isAutoUpdateEnabled(deps))) {
        return () => {};
    }

    void checkForUpdates('auto', deps);

    const timer: AutoUpdateTimer = deps.setInterval(async () => {
        if (await isAutoUpdateEnabled(deps)) {
            await checkForUpdates('auto', deps);
        }
    }, AUTO_UPDATE_INTERVAL_MS);

    return () => deps.clearInterval(timer);
}
