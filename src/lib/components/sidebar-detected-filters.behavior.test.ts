// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: vi.fn((path: string) => path) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('$lib/image-loading', () => ({ loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined) }));
vi.mock('$lib/clipboard-monitor', () => ({ applyClipboardMonitorCollection: vi.fn() }));
vi.mock('$lib/view-utils', () => ({ safeAssetPreviewPath: vi.fn(() => null) }));
vi.mock('$lib/api', () => ({
    countByDetectedClass: vi.fn().mockResolvedValue(4),
    createCanvas: vi.fn(),
    createCollection: vi.fn(),
    createSession: vi.fn(),
    deleteCollectionApi: vi.fn(),
    deleteFolder: vi.fn(),
    getClipboardMonitorStatus: vi.fn().mockResolvedValue(null),
    getImageCount: vi.fn().mockResolvedValue(0),
    importFolder: vi.fn(),
    listCanvases: vi.fn().mockResolvedValue([]),
    listCollections: vi.fn().mockResolvedValue([]),
    listCollectionImages: vi.fn().mockResolvedValue([]),
    listDetectedClasses: vi.fn().mockResolvedValue([['person', 4]]),
    listFolders: vi.fn().mockResolvedValue([]),
    listSessions: vi.fn().mockResolvedValue([]),
    listSmartCollections: vi.fn().mockResolvedValue([]),
    moveClipboardCaptureFolder: vi.fn(),
    publishClipboardCollection: vi.fn(),
    regenerateThumbnails: vi.fn(),
    renameCollectionApi: vi.fn(),
    rescanSources: vi.fn(),
    setClipboardMonitorCaptureExistingOnStart: vi.fn(),
    startClipboardMonitor: vi.fn(),
    stopClipboardMonitor: vi.fn(),
    validateSessionFolder: vi.fn().mockResolvedValue(true),
}));

import Sidebar from './Sidebar.svelte';
import { countByDetectedClass } from '$lib/api';
import { loadImagesForCurrentScope } from '$lib/image-loading';
import {
    activeCanvas,
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSession,
    activeSmartCollection,
    sessionCanvases,
} from '$lib/stores';

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
});

function libraryAllImagesButton(): HTMLButtonElement {
    const label = screen.getAllByText('All Images').find(node => node.closest('button')?.classList.contains('section-item'));
    return label?.closest('button') as HTMLButtonElement;
}

describe('Sidebar detected-class filter behavior', () => {
    it('moves the current navigation state from All Images to the selected class and loads it', async () => {
        const user = userEvent.setup();
        render(Sidebar);

        const classButton = await screen.findByRole('button', { name: /person/i });
        const allImages = libraryAllImagesButton();
        expect(allImages).toHaveAttribute('aria-current', 'true');

        await user.click(classButton);

        await waitFor(() => expect(get(activeDetectedClass)).toBe('person'));
        expect(allImages).not.toHaveAttribute('aria-current');
        expect(allImages).not.toHaveClass('active');
        expect(classButton).toHaveAttribute('aria-current', 'true');
        expect(classButton).toHaveClass('active');
        expect(countByDetectedClass).toHaveBeenCalledWith('person');
        expect(loadImagesForCurrentScope).toHaveBeenCalledOnce();
    });
});
