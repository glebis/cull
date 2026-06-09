import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { viewMode } from './stores';

vi.mock('./api', () => ({
    addToCollection: vi.fn(),
    copyImageToClipboard: vi.fn(),
    createCollection: vi.fn(),
    listCollections: vi.fn(),
    pasteImageFromClipboard: vi.fn(),
    redo: vi.fn(),
    rotateImage: vi.fn(),
    setDecision: vi.fn(),
    setRating: vi.fn(),
    undo: vi.fn(),
}));

vi.mock('./command-palette', () => ({
    commandForKeyboardEvent: vi.fn(() => null),
    openCommandPalette: vi.fn(),
    runCommandPaletteItem: vi.fn(),
}));

vi.mock('./image-loading', () => ({
    invalidateImageCache: vi.fn(),
    loadImagesForCurrentScope: vi.fn(),
}));

vi.mock('./shortcut-reminders', () => ({
    recordShortcutUse: vi.fn(),
    VIEW_CYCLE_SHORTCUT_REMINDER_ID: 'view-cycle',
}));

class TestHTMLElement {}
class TestHTMLInputElement extends TestHTMLElement {}
class TestHTMLTextAreaElement extends TestHTMLElement {}
class TestHTMLSelectElement extends TestHTMLElement {}

describe('Export keyboard shortcuts', () => {
    let events: string[];

    beforeEach(() => {
        vi.stubGlobal('HTMLElement', TestHTMLElement);
        vi.stubGlobal('HTMLInputElement', TestHTMLInputElement);
        vi.stubGlobal('HTMLTextAreaElement', TestHTMLTextAreaElement);
        vi.stubGlobal('HTMLSelectElement', TestHTMLSelectElement);
        vi.stubGlobal('document', { querySelector: vi.fn(() => null), fullscreenElement: null });

        events = [];
        const windowTarget = new EventTarget();
        windowTarget.addEventListener('cull-export-launch', () => events.push('cull-export-launch'));
        vi.stubGlobal('window', windowTarget);
        viewMode.set('grid');
    });

    afterEach(() => {
        vi.unstubAllGlobals();
        viewMode.set('grid');
    });

    it('dispatches export launch on Cmd+Enter in Export mode', async () => {
        const { handleKeydown } = await import('./keys');
        viewMode.set('export');
        const event = {
            key: 'Enter',
            metaKey: true,
            shiftKey: false,
            ctrlKey: false,
            altKey: false,
            target: null,
            preventDefault: vi.fn(),
        } as unknown as KeyboardEvent;

        handleKeydown(event);

        expect(event.preventDefault).toHaveBeenCalledOnce();
        expect(events).toEqual(['cull-export-launch']);
    });
});
