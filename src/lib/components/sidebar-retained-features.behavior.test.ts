// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/svelte';

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: vi.fn((path: string) => path) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('$lib/image-loading', () => ({ loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined) }));
vi.mock('$lib/clipboard-monitor', () => ({ applyClipboardMonitorCollection: vi.fn() }));
vi.mock('$lib/view-utils', () => ({ safeAssetPreviewPath: vi.fn(() => null) }));
vi.mock('$lib/api', () => ({
    countByDetectedClass: vi.fn().mockResolvedValue(0),
    createCanvas: vi.fn(),
    createCollection: vi.fn(),
    deleteCollectionApi: vi.fn(),
    deleteFolder: vi.fn(),
    getClipboardMonitorStatus: vi.fn().mockResolvedValue(null),
    getImageCount: vi.fn().mockResolvedValue(12),
    importFolder: vi.fn(),
    listCanvases: vi.fn().mockResolvedValue([]),
    listCollections: vi.fn().mockResolvedValue([]),
    listCollectionImages: vi.fn().mockResolvedValue([]),
    listDetectedClasses: vi.fn().mockResolvedValue([]),
    listFolders: vi.fn().mockResolvedValue([]),
    listReferencedSources: vi.fn().mockResolvedValue([]),
    listSessions: vi.fn().mockResolvedValue([]),
    listSmartCollections: vi.fn().mockResolvedValue([{
        id: 'recent-imports',
        name: 'Recent Imports',
        description: null,
        collection_type: 'smart',
        filter_json: '{"type":"rule","field":"imported_at","op":"last_n_days","value":7}',
        nl_query: null,
        is_preset: true,
        sort_order: 6,
        created_at: '2026-08-31T00:00:00Z',
        image_count: 5,
    }]),
    listSourceFolders: vi.fn().mockResolvedValue([]),
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
import {
    activeCanvas,
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeReferencedFolder,
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
    activeReferencedFolder.set(null);
    activeSession.set(null);
    activeSmartCollection.set(null);
    sessionCanvases.set([]);
});

describe('release-critical sidebar features', () => {
    it('renders Recent Imports without a decorative clock glyph', async () => {
        render(Sidebar);

        const recentImports = await screen.findByRole('button', { name: /Recent Imports/ });
        expect(recentImports).toHaveTextContent('Recent Imports');
        expect(recentImports).not.toHaveTextContent('⏰');
    });
});
