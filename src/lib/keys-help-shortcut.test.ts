import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { shortcutsOpen } from './stores';

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

class TestHTMLElement {
    tagName = 'DIV';
    isContentEditable = false;
}
class TestHTMLInputElement extends TestHTMLElement {
    tagName = 'INPUT';
}
class TestHTMLTextAreaElement extends TestHTMLElement {}
class TestHTMLSelectElement extends TestHTMLElement {}

function helpEvent(target: unknown): KeyboardEvent {
    return {
        key: '?',
        metaKey: false,
        shiftKey: true,
        ctrlKey: false,
        altKey: false,
        target,
        preventDefault: vi.fn(),
    } as unknown as KeyboardEvent;
}

describe("'?' opens the keyboard-shortcuts help (UX-07)", () => {
    beforeEach(() => {
        vi.stubGlobal('HTMLElement', TestHTMLElement);
        vi.stubGlobal('HTMLInputElement', TestHTMLInputElement);
        vi.stubGlobal('HTMLTextAreaElement', TestHTMLTextAreaElement);
        vi.stubGlobal('HTMLSelectElement', TestHTMLSelectElement);
        vi.stubGlobal('document', { querySelector: vi.fn(() => null), fullscreenElement: null });
        vi.stubGlobal('window', new EventTarget());
        shortcutsOpen.set(false);
    });

    afterEach(() => {
        vi.unstubAllGlobals();
        shortcutsOpen.set(false);
    });

    it("'?' in a non-editable context opens the shortcuts overlay", async () => {
        const { handleKeydown } = await import('./keys');
        const event = helpEvent(new TestHTMLElement());

        handleKeydown(event);

        expect(event.preventDefault).toHaveBeenCalledOnce();
        expect(get(shortcutsOpen)).toBe(true);
    });

    it("'?' typed inside an input does NOT open the overlay", async () => {
        const { handleKeydown } = await import('./keys');
        const event = helpEvent(new TestHTMLInputElement());

        handleKeydown(event);

        expect(event.preventDefault).not.toHaveBeenCalled();
        expect(get(shortcutsOpen)).toBe(false);
    });
});

describe('the palette and help are advertised (UX-07)', () => {
    const statusBar = readFileSync(join(process.cwd(), 'src/lib/components/StatusBar.svelte'), 'utf8');

    it('status bar hint strip advertises ?:help', () => {
        expect(statusBar).toContain('?:help');
    });

    it('status bar hint strip advertises Cmd+P:commands', () => {
        expect(statusBar).toContain('Cmd+P:commands');
    });
});
