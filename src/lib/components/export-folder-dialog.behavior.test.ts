// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';

const mocks = vi.hoisted(() => ({
    chooseFolder: vi.fn(),
    exportImages: vi.fn(),
    listImageIds: vi.fn(),
    listImageIdsForScope: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: mocks.chooseFolder }));
vi.mock('$lib/api', () => ({
    exportImagesToFolder: mocks.exportImages,
    listImageIds: mocks.listImageIds,
}));
vi.mock('$lib/embedding-scope', () => ({
    listImageIdsForScope: mocks.listImageIdsForScope,
}));

import ExportFolderDialog from './ExportFolderDialog.svelte';
import {
    activeCollection,
    activeFolder,
    collections,
    exportFolderOpen,
    exportFolderSmartCollection,
    selectedIds,
    showRejected,
} from '$lib/stores';

afterEach(() => cleanup());

beforeEach(() => {
    vi.clearAllMocks();
    activeCollection.set(null);
    activeFolder.set(null);
    collections.set([]);
    selectedIds.set(new Set(['unrelated-selection']));
    showRejected.set(false);
    exportFolderSmartCollection.set({
        id: 'smart-1',
        name: 'Five Stars',
        description: null,
        collection_type: 'smart',
        filter_json: '{"type":"rule","field":"rating","op":"eq","value":5}',
        nl_query: 'rating 5',
        is_preset: false,
        sort_order: 1,
        created_at: '2026-01-01',
        image_count: 2,
    });
    exportFolderOpen.set(true);
    mocks.chooseFolder.mockResolvedValue('/exports');
    mocks.listImageIdsForScope.mockResolvedValue(['smart-a', 'smart-b']);
    mocks.listImageIds.mockResolvedValue(['library-a']);
    mocks.exportImages.mockResolvedValue({
        exported: 2,
        skipped: 0,
        errors: [],
        output_dir: '/exports',
        files: [],
    });
});

describe('ExportFolderDialog smart-collection export', () => {
    it('exports the complete smart result set instead of selection or library IDs', async () => {
        const user = userEvent.setup();
        render(ExportFolderDialog);

        expect(screen.getByText(/smart collection “Five Stars”/i)).toBeVisible();
        await user.click(screen.getByRole('button', { name: 'Choose Folder & Export' }));

        await waitFor(() => expect(mocks.listImageIdsForScope).toHaveBeenCalledWith({
            type: 'smart',
            id: 'smart-1',
            filter_json: '{"type":"rule","field":"rating","op":"eq","value":5}',
            include_rejected: false,
        }));
        expect(mocks.listImageIds).not.toHaveBeenCalled();
        expect(mocks.exportImages).toHaveBeenCalledWith(expect.objectContaining({
            image_ids: ['smart-a', 'smart-b'],
        }));
    });
});
