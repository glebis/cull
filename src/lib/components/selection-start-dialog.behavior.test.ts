// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';

import SelectionStartDialog from './SelectionStartDialog.svelte';
import { createSelectionRun, listSelectionSource, previewSelectionSource } from '$lib/api';
import { selectionRun, selectionStartOpen, similarityViewActive, viewMode } from '$lib/stores';

const apiMocks = vi.hoisted(() => ({
    previewSelectionSource: vi.fn(),
    createSelectionRun: vi.fn(),
    listSelectionSource: vi.fn(),
    listImages: vi.fn().mockResolvedValue([]),
    getImageCount: vi.fn().mockResolvedValue(0),
    listImagesByFolder: vi.fn().mockResolvedValue([]),
    listImagesFiltered: vi.fn().mockResolvedValue([]),
    listCollectionImages: vi.fn().mockResolvedValue([]),
    listImagesByDetectedClass: vi.fn().mockResolvedValue([]),
    listImagesInReferencedFolder: vi.fn().mockResolvedValue([]),
    evaluateSmartCollection: vi.fn().mockResolvedValue([]),
    getBatchImages: vi.fn().mockResolvedValue([]),
}));

// The dialog runs the real startSelectionRun action — only the Tauri
// boundary is mocked, so the Start click exercises the actual handler.
vi.mock('$lib/api', () => ({
    ...apiMocks,
    undo: vi.fn().mockResolvedValue(null),
    redo: vi.fn().mockResolvedValue(null),
}));

function makeRunState() {
    return {
        run: {
            id: 'run-1',
            name: 'All Images',
            status: 'active',
            source_count: 12,
            shortlist_count: 0,
            target_count: null,
            source_scope: { type: 'all', include_rejected: false },
            created_at: '2026-09-05T00:00:00Z',
            updated_at: '2026-09-05T00:00:00Z',
            finished_at: null,
            rejected_shortlist_count: 0,
        },
        shortlist_ids: [],
    };
}

beforeEach(() => {
    viewMode.set('grid');
    similarityViewActive.set(false);
    selectionRun.set(null);
    selectionStartOpen.set(true);
    apiMocks.previewSelectionSource.mockResolvedValue({ count: 12 });
    apiMocks.createSelectionRun.mockResolvedValue(makeRunState());
    apiMocks.listSelectionSource.mockResolvedValue({ items: [], total: 0 });
});

afterEach(() => {
    cleanup();
    selectionStartOpen.set(false);
    selectionRun.set(null);
    vi.clearAllMocks();
});

async function openDialog() {
    render(SelectionStartDialog);
    const startButton = await screen.findByRole('button', { name: 'Start Selection' });
    // The source count must resolve before Start is enabled.
    await waitFor(() => expect(startButton).toBeEnabled());
    return { startButton, targetInput: screen.getByRole('spinbutton') };
}

describe('SelectionStartDialog rendered behavior', () => {
    it('accepts a positive whole target through native numeric input events without crashing', async () => {
        const user = userEvent.setup();
        const { targetInput } = await openDialog();

        await targetInput.click();
        await user.type(targetInput, '5');

        // The crash regression: the number binding used to reach a string
        // .trim() call. The dialog must keep working with a numeric state.
        expect(screen.queryByText('Target must be a positive whole number')).not.toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'Start Selection' })).toBeEnabled();
    });

    it('treats a cleared target as optional again', async () => {
        const user = userEvent.setup();
        const { targetInput } = await openDialog();

        await targetInput.click();
        await user.type(targetInput, '5');
        await user.clear(targetInput);

        expect(screen.queryByText('Target must be a positive whole number')).not.toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'Start Selection' })).toBeEnabled();
    });

    it('rejects zero, negative, and fractional targets and disables Start', async () => {
        const user = userEvent.setup();
        const { targetInput } = await openDialog();

        await targetInput.click();
        await user.type(targetInput, '2.5');

        expect(await screen.findByText('Target must be a positive whole number')).toBeVisible();
        expect(screen.getByRole('button', { name: 'Start Selection' })).toBeDisabled();
        expect(createSelectionRun).not.toHaveBeenCalled();

        await user.clear(targetInput);
        await user.type(targetInput, '0');
        expect(await screen.findByText('Target must be a positive whole number')).toBeVisible();

        await user.clear(targetInput);
        await user.type(targetInput, '-3');
        expect(await screen.findByText('Target must be a positive whole number')).toBeVisible();
        expect(screen.getByRole('button', { name: 'Start Selection' })).toBeDisabled();
    });

    it('completes the start action with no target: run created, dialog closed, mode entered', async () => {
        const user = userEvent.setup();
        const { startButton } = await openDialog();

        await user.click(startButton);

        await waitFor(() => {
            expect(createSelectionRun).toHaveBeenCalledTimes(1);
        });
        expect(createSelectionRun).toHaveBeenCalledWith(
            'All Images',
            { type: 'all', include_rejected: false },
            null,
        );
        // The dialog closes and the run is entered through the real action.
        await waitFor(() => expect(get(selectionStartOpen)).toBe(false));
        expect(get(selectionRun)?.id).toBe('run-1');
        expect(get(selectionRun)?.status).toBe('active');
        expect(get(selectionRun)?.target_count).toBeNull();
        // The scoped loader warmed the Source view for the new run.
        await waitFor(() => expect(listSelectionSource).toHaveBeenCalledWith('run-1', 0, 200, expect.anything()));
        expect(previewSelectionSource).toHaveBeenCalled();
    });
});