// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';

const mocks = vi.hoisted(() => ({
    open: vi.fn(),
    importFolder: vi.fn(),
    listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: vi.fn((path: string) => path) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: mocks.open }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('$lib/image-loading', () => ({ loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined) }));
vi.mock('$lib/clipboard-monitor', () => ({ applyClipboardMonitorCollection: vi.fn() }));
vi.mock('$lib/view-utils', () => ({ safeAssetPreviewPath: vi.fn(() => null) }));
vi.mock('$lib/api', () => ({
    countByDetectedClass: vi.fn().mockResolvedValue(0),
    createCanvas: vi.fn(),
    createCollection: vi.fn(),
    createSession: vi.fn(),
    deleteCollectionApi: vi.fn(),
    deleteFolder: vi.fn(),
    getClipboardMonitorStatus: vi.fn().mockResolvedValue(null),
    getImageCount: vi.fn().mockResolvedValue(0),
    importFolder: mocks.importFolder,
    listCanvases: vi.fn().mockResolvedValue([]),
    listCollections: vi.fn().mockResolvedValue([]),
    listCollectionImages: vi.fn().mockResolvedValue([]),
    listDetectedClasses: vi.fn().mockResolvedValue([]),
    listFolders: vi.fn().mockResolvedValue([]),
    listSessions: vi.fn().mockResolvedValue([]),
    listSmartCollections: vi.fn().mockResolvedValue([]),
    moveClipboardCaptureFolder: vi.fn(),
    publishClipboardCollection: vi.fn(),
    regenerateThumbnails: vi.fn(),
    renameCollectionApi: vi.fn(),
    renameFolder: vi.fn(),
    rescanSources: vi.fn(),
    setClipboardMonitorCaptureExistingOnStart: vi.fn(),
    startClipboardMonitor: vi.fn(),
    stopClipboardMonitor: vi.fn(),
    validateSessionFolder: vi.fn().mockResolvedValue(true),
}));

import Sidebar from './Sidebar.svelte';
import { toasts } from '$lib/stores';

afterEach(() => cleanup());
beforeEach(() => {
    vi.clearAllMocks();
    toasts.set([]);
    vi.stubGlobal('crypto', { randomUUID: () => 'sidebar-import-own' });
});

describe('Sidebar import progress ownership', () => {
    it('ignores another import and reports cancellation truthfully', async () => {
        let progressHandler: ((event: { payload: Record<string, unknown> }) => void) | undefined;
        mocks.listen.mockImplementation(async (name: string, handler: typeof progressHandler) => {
            if (name === 'import-progress') progressHandler = handler;
            return vi.fn();
        });
        mocks.open.mockResolvedValue('/photos/current');
        let resolveImport!: (value: unknown) => void;
        mocks.importFolder.mockImplementation(
            () => new Promise(resolve => { resolveImport = resolve; }),
        );

        const user = userEvent.setup();
        render(Sidebar);
        await user.click(await screen.findByRole('button', { name: 'Import folder' }));
        await waitFor(() => expect(mocks.importFolder).toHaveBeenCalledWith(
            '/photos/current',
            null,
            'sidebar-import-own',
        ));

        progressHandler?.({ payload: { progress_id: 'another-import', current: 9, total: 10 } });
        expect(screen.getByText('Scanning folder')).toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'Importing folder' })).toBeDisabled();

        progressHandler?.({ payload: { progress_id: 'sidebar-import-own', current: 2, total: 10 } });
        expect(await screen.findByText('Importing 2 of 10')).toBeInTheDocument();

        resolveImport({
            imported: 2,
            skipped: 0,
            errors: [],
            batch_id: 'batch-partial',
            image_ids: ['one', 'two'],
            cancelled: true,
        });

        expect(await screen.findByText('Cancelled: +2 imported, 0 skipped')).toBeInTheDocument();
        expect(get(toasts).at(-1)).toMatchObject({
            message: 'Import cancelled',
            type: 'warning',
        });
    });
});
