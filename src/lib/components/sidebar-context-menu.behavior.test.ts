// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';

const requestMocks = vi.hoisted(() => ({
    requestConfirm: vi.fn(),
    requestTextInput: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: vi.fn((path: string) => path) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('$lib/stores', async (importOriginal) => ({
    ...await importOriginal<typeof import('$lib/stores')>(),
    requestConfirm: requestMocks.requestConfirm,
    requestTextInput: requestMocks.requestTextInput,
}));
vi.mock('$lib/image-loading', () => ({ loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined) }));
vi.mock('$lib/clipboard-monitor', () => ({ applyClipboardMonitorCollection: vi.fn() }));
vi.mock('$lib/view-utils', () => ({ safeAssetPreviewPath: vi.fn(() => null) }));
vi.mock('$lib/api', () => ({
    addToCollection: vi.fn(),
    countByDetectedClass: vi.fn().mockResolvedValue(0),
    createCanvas: vi.fn(),
    createCollection: vi.fn(),
    createCollectionWithImages: vi.fn(),
    deleteCollectionApi: vi.fn(),
    deleteCanvas: vi.fn(),
    deleteFolder: vi.fn(),
    deleteSmartCollectionApi: vi.fn(),
    getClipboardMonitorStatus: vi.fn().mockResolvedValue(null),
    getImageCount: vi.fn().mockResolvedValue(2),
    importFolder: vi.fn().mockResolvedValue({ imported: 0, skipped: 2, errors: [], image_ids: [], cancelled: false }),
    listCanvases: vi.fn().mockResolvedValue([]),
    listCollections: vi.fn().mockResolvedValue([['c1', 'Portfolio', 2]]),
    listCollectionImages: vi.fn().mockResolvedValue([]),
    listDetectedClasses: vi.fn().mockResolvedValue([]),
    listFolders: vi.fn().mockResolvedValue([['/mock/library', 2]]),
    listImagesByFolder: vi.fn().mockResolvedValue([
        { image: { id: 'folder-image-1' }, path: '/mock/library/one.png' },
        { image: { id: 'folder-image-2' }, path: '/mock/library/two.png' },
    ]),
    listSessions: vi.fn().mockResolvedValue([]),
    listSmartCollections: vi.fn().mockResolvedValue([{
        id: 'smart-1', name: 'Five Stars', description: null, collection_type: 'smart',
        filter_json: '{"type":"rule","field":"rating","op":"eq","value":5}', nl_query: 'rating at least 5',
        is_preset: false, sort_order: 1, created_at: '2026-01-01', image_count: 0,
    }]),
    moveClipboardCaptureFolder: vi.fn(),
    publishClipboardCollection: vi.fn(),
    regenerateThumbnails: vi.fn(),
    renameCollectionApi: vi.fn(),
    renameFolder: vi.fn(),
    rescanSources: vi.fn(),
    setClipboardMonitorCaptureExistingOnStart: vi.fn(),
    startClipboardMonitor: vi.fn(),
    stopClipboardMonitor: vi.fn(),
    updateSmartCollectionApi: vi.fn(),
    validateSessionFolder: vi.fn().mockResolvedValue(true),
}));

import Sidebar from './Sidebar.svelte';
import {
    addToCollection,
    createCollection,
    createCollectionWithImages,
    importFolder,
    listImagesByFolder,
    listSmartCollections,
    updateSmartCollectionApi,
} from '$lib/api';
import { activeCanvas, activeCollection, activeDetectedClass, activeFolder, activeSession, activeSmartCollection, sessionCanvases, toasts } from '$lib/stores';

afterEach(() => cleanup());
beforeEach(() => {
    vi.clearAllMocks();
    activeCanvas.set(null);
    activeCollection.set(null);
    activeDetectedClass.set(null);
    activeFolder.set(null);
    activeSession.set(null);
    activeSmartCollection.set(null);
    sessionCanvases.set([]);
    toasts.set([]);
    requestMocks.requestTextInput.mockReset();
    requestMocks.requestConfirm.mockReset();
});

describe('Sidebar context menu behavior', () => {
    it('opens the folder action menu from right-click and runs a targeted rescan', async () => {
        const user = userEvent.setup();
        render(Sidebar);

        const folder = await screen.findByRole('treeitem', { name: /library, 2 images/i });
        await fireEvent.contextMenu(folder, { clientX: 40, clientY: 60 });

        const menu = await screen.findByRole('menu', { name: 'library actions' });
        expect(menu).toBeVisible();
        expect(screen.getByRole('menuitem', { name: 'Rename…' })).toBeVisible();
        const rescan = screen.getByRole('menuitem', { name: 'Rescan Folder' });
        await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Open Folder' })).toHaveFocus());
        await user.click(rescan);

        await waitFor(() => expect(importFolder).toHaveBeenCalledWith('/mock/library'));
        expect(screen.queryByRole('menu', { name: 'library actions' })).not.toBeInTheDocument();
    });

    it('creates a collection from folder contents through the atomic API', async () => {
        const user = userEvent.setup();
        requestMocks.requestTextInput.mockResolvedValue('Folder Picks');
        render(Sidebar);

        const folder = await screen.findByRole('treeitem', { name: /library, 2 images/i });
        await fireEvent.contextMenu(folder, { clientX: 40, clientY: 60 });
        await user.click(await screen.findByRole('menuitem', { name: 'Add Contents to Collection' }));
        await user.click(await screen.findByRole('menuitem', { name: 'New Collection…' }));

        await waitFor(() => expect(createCollectionWithImages).toHaveBeenCalledWith(
            'Folder Picks',
            ['folder-image-1', 'folder-image-2'],
        ));
        expect(listImagesByFolder).toHaveBeenCalledWith('/mock/library', 500, 0, false);
        expect(createCollection).not.toHaveBeenCalled();
        expect(addToCollection).not.toHaveBeenCalled();
    });

    it('keeps an empty user smart collection editable from the keyboard context-menu shortcut', async () => {
        const user = userEvent.setup();
        render(Sidebar);

        const smart = await screen.findByRole('button', { name: /Five Stars/i });
        smart.focus();
        await fireEvent.keyDown(smart, { key: 'F10', shiftKey: true });

        expect(await screen.findByRole('menuitem', { name: 'Edit Rules…' })).toBeVisible();
        expect(screen.getByRole('menuitem', { name: 'Delete Smart Collection…' })).toBeVisible();

        await user.click(screen.getByRole('menuitem', { name: 'Edit Rules…' }));
        vi.mocked(listSmartCollections).mockRejectedValueOnce(new Error('refresh unavailable'));
        await user.click(await screen.findByRole('button', { name: 'Save Rules' }));
        await waitFor(() => expect(updateSmartCollectionApi).toHaveBeenCalledWith(
            'smart-1',
            'Five Stars',
            expect.any(String),
            'rating at least 5',
        ));
        await waitFor(() => expect(get(toasts).at(-1)).toMatchObject({
            message: 'Smart collection updated, but the view could not refresh',
            type: 'warning',
        }));
    });

    it('confirms canvas deletion and reconciles the active canvas stores', async () => {
        const user = userEvent.setup();
        requestMocks.requestConfirm.mockResolvedValue(true);
        render(Sidebar);
        const session = {
            id: 'session-1', name: 'Review', description: null, folder_path: '/mock/review',
            settings_json: null, created_at: '2026-01-01', image_count: 2,
        };
        const canvas = {
            id: 'canvas-1', session_id: session.id, name: 'Selects', canvas_type: 'manual' as const,
            layout_json: '{}', filter_json: null, grid_config_json: null, sort_order: 0,
            created_at: '2026-01-01', updated_at: '2026-01-01',
        };
        activeSession.set(session);
        activeCanvas.set(canvas);
        sessionCanvases.set([canvas]);

        const canvasButton = await screen.findByRole('button', { name: 'Selects manual' });
        await fireEvent.contextMenu(canvasButton, { clientX: 40, clientY: 60 });
        await user.click(await screen.findByRole('menuitem', { name: 'Delete Canvas…' }));

        const { deleteCanvas } = await import('$lib/api');
        await waitFor(() => expect(deleteCanvas).toHaveBeenCalledWith('canvas-1'));
        expect(requestMocks.requestConfirm).toHaveBeenCalledWith(expect.objectContaining({
            title: 'Delete Canvas',
            danger: true,
        }));
        expect(get(activeCanvas)).toBeNull();
        expect(get(sessionCanvases)).toEqual([]);
    });

    it('anchors a keyboard-opened folder menu to the focused tree item', async () => {
        render(Sidebar);

        const folder = await screen.findByRole('treeitem', { name: /library, 2 images/i });
        vi.spyOn(folder, 'getBoundingClientRect').mockReturnValue({
            x: 280, y: 360, left: 280, top: 360, right: 440, bottom: 392,
            width: 160, height: 32, toJSON: () => ({}),
        });
        folder.focus();
        await fireEvent.keyDown(folder, { key: 'F10', shiftKey: true });

        const menu = await screen.findByRole('menu', { name: 'library actions' });
        await waitFor(() => expect(menu).toHaveStyle({ left: '312px', top: '384px' }));
    });
});
