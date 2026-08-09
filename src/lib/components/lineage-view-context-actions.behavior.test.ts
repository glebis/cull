// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: vi.fn((path: string) => path) }));
vi.mock('$lib/view-utils', () => ({ safeAssetPreviewPath: vi.fn(() => null) }));
vi.mock('$lib/api', () => ({
    listLineageGroups: vi.fn().mockResolvedValue([{
        id: 'group-1', name: 'Variants', image_count: 2, detection_method: 'manual',
        created_at: '2026-01-01', updated_at: '2026-01-01',
    }]),
    getLineageGroupImages: vi.fn().mockResolvedValue([
        { image: { id: 'image-1' }, path: '/mock/one.png', selection: null },
        { image: { id: 'image-2' }, path: '/mock/two.png', selection: null },
    ]),
    renameLineageGroup: vi.fn(),
    dissolveLineageGroup: vi.fn(),
}));

import LineageView from './LineageView.svelte';
import { activeCollection, activeFolder, images, lineageLayout } from '$lib/stores';

afterEach(() => cleanup());
beforeEach(() => {
    activeCollection.set(null);
    activeFolder.set(null);
    lineageLayout.set('timeline');
    images.set([
        { image: { id: 'image-1' }, path: '/mock/one.png', selection: null },
        { image: { id: 'image-2' }, path: '/mock/two.png', selection: null },
    ] as never[]);
});

describe('LineageView context action behavior', () => {
    it('restores focus to the group button after a pointer-opened menu closes', async () => {
        const user = userEvent.setup();
        render(LineageView);

        const groupButton = await screen.findByRole('button', { name: 'Variants' });
        await fireEvent.contextMenu(groupButton, { clientX: 80, clientY: 100 });
        await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Rename…' })).toHaveFocus());
        await user.keyboard('{Escape}');

        await waitFor(() => expect(groupButton).toHaveFocus());
    });
});
