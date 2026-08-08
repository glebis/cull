// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import CommandPalette from './CommandPalette.svelte';
import {
    collections,
    commandPaletteMode,
    commandPaletteOpen,
    folders,
    sessions,
    sessionCanvases,
    smartCollections,
} from '$lib/stores';

describe('CommandPalette destinations', () => {
    beforeEach(() => {
        localStorage.clear();
        collections.set([['collection-1', 'Portfolio', 7]]);
        folders.set([]);
        smartCollections.set([]);
        sessions.set([]);
        sessionCanvases.set([]);
        commandPaletteMode.set('commands');
        commandPaletteOpen.set(true);
    });

    afterEach(() => {
        commandPaletteOpen.set(false);
        cleanup();
    });

    it('populates destinations when switching an open command-only palette to All', async () => {
        render(CommandPalette);

        expect(screen.queryByText('Portfolio')).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'All' }));

        await waitFor(() => expect(screen.getByText('Portfolio')).toBeTruthy());
    });
});
