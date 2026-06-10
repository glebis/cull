import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { viewMode } from './stores';
import { clearPluginTabs, registerCoreTabs } from './plugins/tab-registry';

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

function tabEvent(overrides: Partial<KeyboardEvent> & { target?: unknown } = {}): KeyboardEvent {
    return {
        key: 'Tab',
        metaKey: false,
        shiftKey: false,
        ctrlKey: false,
        altKey: false,
        target: null,
        preventDefault: vi.fn(),
        ...overrides,
    } as unknown as KeyboardEvent;
}

describe('Tab does not hijack native keyboard focus order (UX-01)', () => {
    beforeEach(() => {
        vi.stubGlobal('HTMLElement', TestHTMLElement);
        vi.stubGlobal('HTMLInputElement', TestHTMLInputElement);
        vi.stubGlobal('HTMLTextAreaElement', TestHTMLTextAreaElement);
        vi.stubGlobal('HTMLSelectElement', TestHTMLSelectElement);
        vi.stubGlobal('document', { querySelector: vi.fn(() => null), fullscreenElement: null });
        vi.stubGlobal('window', new EventTarget());
        clearPluginTabs();
        registerCoreTabs();
        viewMode.set('grid');
    });

    afterEach(() => {
        vi.unstubAllGlobals();
        viewMode.set('grid');
    });

    it('bare Tab on a focused button is not consumed and does not cycle views', async () => {
        const { handleKeydown } = await import('./keys');
        const button = new TestHTMLElement();
        button.tagName = 'BUTTON';
        const event = tabEvent({ target: button as unknown as EventTarget });

        handleKeydown(event);

        expect(event.preventDefault).not.toHaveBeenCalled();
        expect(get(viewMode)).toBe('grid');
    });

    it('bare Shift+Tab is not consumed and does not cycle views', async () => {
        const { handleKeydown } = await import('./keys');
        const event = tabEvent({ shiftKey: true, target: new TestHTMLElement() as unknown as EventTarget });

        handleKeydown(event);

        expect(event.preventDefault).not.toHaveBeenCalled();
        expect(get(viewMode)).toBe('grid');
    });

    it('Ctrl+Tab still cycles to the next view', async () => {
        const { handleKeydown } = await import('./keys');
        const event = tabEvent({ ctrlKey: true, target: new TestHTMLElement() as unknown as EventTarget });

        handleKeydown(event);

        expect(event.preventDefault).toHaveBeenCalledOnce();
        expect(get(viewMode)).toBe('loupe');
    });

    it('Ctrl+Shift+Tab cycles to the previous view', async () => {
        const { handleKeydown } = await import('./keys');
        viewMode.set('loupe');
        const event = tabEvent({ ctrlKey: true, shiftKey: true, target: new TestHTMLElement() as unknown as EventTarget });

        handleKeydown(event);

        expect(event.preventDefault).toHaveBeenCalledOnce();
        expect(get(viewMode)).toBe('grid');
    });
});
