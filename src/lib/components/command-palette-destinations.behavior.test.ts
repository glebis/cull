// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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
        vi.restoreAllMocks();
    });

    it('populates destinations when switching an open command-only palette to All', async () => {
        render(CommandPalette);

        expect(screen.queryByText('Portfolio')).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'All' }));

        await waitFor(() => expect(screen.getByText('Portfolio')).toBeTruthy());
    });

    it('keeps the keyboard context menu inside the viewport', async () => {
        Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 });
        Object.defineProperty(window, 'innerHeight', { configurable: true, value: 768 });
        const originalRect = HTMLElement.prototype.getBoundingClientRect;
        vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
            if (this.classList.contains('palette-context-menu')) {
                return {
                    x: 796,
                    y: 450,
                    left: 796,
                    right: 1016,
                    top: 450,
                    bottom: 760,
                    width: 220,
                    height: 310,
                    toJSON: () => ({}),
                };
            }
            return originalRect.call(this);
        });
        render(CommandPalette);

        const input = screen.getByPlaceholderText('Run a command');
        input.getBoundingClientRect = () => ({
            x: 976,
            y: 692,
            left: 976,
            right: 1008,
            top: 692,
            bottom: 716,
            width: 32,
            height: 24,
            toJSON: () => ({}),
        });

        await fireEvent.keyDown(input, { key: 'F10', shiftKey: true });

        const menu = screen.getByRole('menu');
        expect(menu.getAttribute('style')).toContain('left: 796px');
        expect(menu.getAttribute('style')).toContain('top: 450px');
    });

    it('uses Pin and Unpin terminology for command preferences', async () => {
        render(CommandPalette);

        const settings = screen.getByRole('option', { name: /Open Settings/ });
        await fireEvent.contextMenu(settings, { clientX: 40, clientY: 40 });
        await fireEvent.click(screen.getByRole('menuitem', { name: 'Pin' }));

        await fireEvent.contextMenu(settings, { clientX: 40, clientY: 40 });
        expect(screen.getByRole('menuitem', { name: 'Unpin' })).toBeTruthy();
    });
});
