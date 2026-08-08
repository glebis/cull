// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import type { ImageWithFile } from '$lib/api';

const mocks = vi.hoisted(() => ({
    setDecision: vi.fn().mockResolvedValue(undefined),
    commandShortcutHints: vi.fn((ids: string[]) => {
        const shortcuts: Record<string, string> = {
            'image.rating.0': '0',
            'image.rating.1': '1',
            'image.rating.2': '2',
            'image.rating.3': '3',
            'image.rating.4': '4',
            'image.rating.5': '5',
            'image.decision.accept': 'A',
            'image.decision.reject': 'X',
            'image.decision.undecided': 'U',
            'image.copy': 'Cmd+C',
            'image.trash': 'Backspace',
        };
        return Object.fromEntries(ids.flatMap(id => shortcuts[id] ? [[id, shortcuts[id]]] : []));
    }),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('$lib/api', () => ({
    addToCollection: vi.fn(),
    createCollection: vi.fn(),
    listCollections: vi.fn().mockResolvedValue([]),
    listFolders: vi.fn().mockResolvedValue([]),
    listOpenWithApplications: vi.fn().mockResolvedValue([]),
    moveImage: vi.fn(),
    openImagesWithApplication: vi.fn(),
    removeFromCollection: vi.fn(),
    renameImage: vi.fn(),
    setDecision: mocks.setDecision,
    setRating: vi.fn(),
    shareImages: vi.fn(),
}));
vi.mock('$lib/image-loading', () => ({
    invalidateImageCache: vi.fn(),
    loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined),
}));
vi.mock('$lib/similarity', () => ({ loadSimilarImages: vi.fn() }));
vi.mock('$lib/command-palette', () => ({
    commandShortcutHints: mocks.commandShortcutHints,
    eventMatchesShortcut: vi.fn((event: KeyboardEvent, shortcut: string) => (
        !event.metaKey && !event.ctrlKey && !event.altKey
        && event.key.toUpperCase() === shortcut.toUpperCase()
    )),
}));
vi.mock('$lib/image-copy-action', () => ({ copyImageWithToast: vi.fn() }));

import ContextMenu from './ContextMenu.svelte';

const image: ImageWithFile = {
    image: {
        id: 'image-one',
        sha256_hash: 'hash-one',
        width: 100,
        height: 100,
        format: 'png',
        file_size: 100,
        created_at: '2026-08-08T00:00:00Z',
        imported_at: '2026-08-08T00:00:00Z',
        ai_prompt: null,
        raw_metadata: null,
    },
    path: '/images/one.png',
    thumbnail_path: null,
    selection: null,
    source_label: null,
    missing_at: null,
};

afterEach(() => cleanup());
beforeEach(() => vi.clearAllMocks());

describe('ContextMenu shortcut hints', () => {
    it('renders the command registry shortcuts beside matching actions', async () => {
        const { container } = render(ContextMenu, {
            props: { image, x: 20, y: 20, onclose: vi.fn() },
        });

        expect(container.querySelector('[data-shortcut-for="image.decision.accept"]')).toHaveTextContent('A');
        expect(container.querySelector('[data-shortcut-for="image.decision.reject"]')).toHaveTextContent('X');
        expect(container.querySelector('[data-shortcut-for="image.decision.undecided"]')).toHaveTextContent('U');
        expect(container.querySelector('[data-shortcut-for="image.trash"]')).toHaveTextContent('Backspace');

        expect(mocks.commandShortcutHints).toHaveBeenCalledWith([
            'image.rating.0',
            'image.rating.1',
            'image.rating.2',
            'image.rating.3',
            'image.rating.4',
            'image.rating.5',
            'image.decision.accept',
            'image.decision.reject',
            'image.decision.undecided',
            'image.copy',
            'image.trash',
        ]);

        await fireEvent.mouseEnter(container.querySelector('[data-submenu-key="rate"]')!.parentElement!);
        for (let rating = 0; rating <= 5; rating += 1) {
            expect(container.querySelector(`[data-shortcut-for="image.rating.${rating}"]`))
                .toHaveTextContent(String(rating));
        }

        await fireEvent.mouseEnter(container.querySelector('[data-submenu-key="copy"]')!.parentElement!);
        expect(container.querySelector('[data-shortcut-for="image.copy"]')).toHaveTextContent('Cmd+C');
        expect(container.querySelector('[data-shortcut-for="image.copy"]')?.closest('button'))
            .toHaveTextContent('Copy Image');

        await fireEvent.keyDown(container.querySelector('[role="menu"]')!, { key: 'a' });
        expect(mocks.setDecision).toHaveBeenCalledWith('image-one', 'accept', null);
    });
});
