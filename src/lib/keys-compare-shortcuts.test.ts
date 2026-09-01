import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import type { ImageWithFile } from './api';
import { compareActiveSide, focusedIndex, images, selectedIds, viewMode } from './stores';

const mocks = vi.hoisted(() => ({
    commandForKeyboardEvent: vi.fn(),
    runCommandPaletteItem: vi.fn(),
    setDecision: vi.fn().mockResolvedValue(undefined),
    setRating: vi.fn().mockResolvedValue(undefined),
    undo: vi.fn().mockResolvedValue('library action'),
}));

vi.mock('./api', () => ({
    addToCollection: vi.fn(), createCollection: vi.fn(), listCollections: vi.fn(),
    pasteImageFromClipboard: vi.fn(), redo: vi.fn(), rotateImage: vi.fn(),
    setDecision: mocks.setDecision, setRating: mocks.setRating, undo: mocks.undo,
}));
vi.mock('./command-palette', () => ({
    commandForKeyboardEvent: mocks.commandForKeyboardEvent,
    openCommandPalette: vi.fn(),
    runCommandPaletteItem: mocks.runCommandPaletteItem,
}));
vi.mock('./image-loading', () => ({
    invalidateImageCache: vi.fn(),
    loadImagesForCurrentScope: vi.fn(),
}));
vi.mock('./shortcut-reminders', () => ({
    recordShortcutUse: vi.fn(),
    VIEW_CYCLE_SHORTCUT_REMINDER_ID: 'view-cycle',
}));

import { handleKeydown } from './keys';

function image(id: string): ImageWithFile {
    return {
        image: {
            id, sha256_hash: `hash-${id}`, width: 100, height: 100, format: 'png',
            file_size: 100, created_at: '2026-08-08T00:00:00Z',
            imported_at: '2026-08-08T00:00:00Z', ai_prompt: null, raw_metadata: null,
        },
        path: `/images/${id}.png`, thumbnail_path: null, selection: null,
        source_label: null, missing_at: null,
    };
}

function keyEvent(key: string, modifiers: Partial<KeyboardEvent> = {}): KeyboardEvent {
    return {
        key, metaKey: false, shiftKey: false, ctrlKey: false, altKey: false,
        target: null, preventDefault: vi.fn(),
        ...modifiers,
    } as unknown as KeyboardEvent;
}

describe('Compare image shortcuts', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.stubGlobal('HTMLElement', class {});
        vi.stubGlobal('HTMLInputElement', class {});
        vi.stubGlobal('HTMLTextAreaElement', class {});
        vi.stubGlobal('HTMLSelectElement', class {});
        vi.stubGlobal('document', { querySelector: vi.fn(() => null), fullscreenElement: null });
        vi.stubGlobal('window', new EventTarget());
        const allImages = [image('left'), image('right')];
        images.set(allImages);
        selectedIds.set(new Set(['left', 'right']));
        focusedIndex.set(0);
        compareActiveSide.set(0);
        viewMode.set('compare');
    });

    it('keeps bare 1/2 reserved for choosing the winning side', async () => {
        mocks.commandForKeyboardEvent.mockReturnValue({ id: 'image.rating.1' });

        handleKeydown(keyEvent('1'));

        expect(mocks.commandForKeyboardEvent).not.toHaveBeenCalled();
        expect(mocks.runCommandPaletteItem).not.toHaveBeenCalled();
        await vi.waitFor(() => expect(mocks.setDecision).toHaveBeenCalledTimes(2));
        expect(mocks.setDecision).toHaveBeenNthCalledWith(1, 'left', 'accept', null);
        expect(mocks.setDecision).toHaveBeenNthCalledWith(2, 'right', 'reject', null);
    });

    it('rates the active side when a Compare rating chord is pending', async () => {
        mocks.commandForKeyboardEvent.mockReturnValue(null);
        handleKeydown(keyEvent('s'));
        handleKeydown(keyEvent('1'));

        expect(mocks.runCommandPaletteItem).not.toHaveBeenCalled();
        expect(mocks.setDecision).not.toHaveBeenCalled();
        await vi.waitFor(() => expect(mocks.setRating).toHaveBeenCalledWith('left', 1, null));
    });

    it('keeps Grid selection undo ahead of database undo', () => {
        mocks.commandForKeyboardEvent.mockReturnValue(null);
        viewMode.set('grid');
        selectedIds.reset();
        selectedIds.set(new Set(['left']));

        handleKeydown(keyEvent('z', { metaKey: true }));

        expect(get(selectedIds)).toEqual(new Set());
        expect(mocks.undo).not.toHaveBeenCalled();
    });
});
