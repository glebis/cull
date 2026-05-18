import { describe, expect, it } from 'vitest';
import {
    canAssignCommandHotkey,
    eventMatchesShortcut,
    getShortcutConflict,
    scoreCommandPaletteItem,
    shortcutFromKeyboardEvent,
    sortCommandPaletteItems,
    type CommandPaletteItem,
} from './command-palette';

function keyEvent(key: string, modifiers: Partial<KeyboardEvent> = {}): KeyboardEvent {
    return {
        key,
        metaKey: false,
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        ...modifiers,
    } as KeyboardEvent;
}

function item(id: string, title: string, category = 'Command', extra: Partial<CommandPaletteItem> = {}): CommandPaletteItem {
    return {
        id,
        title,
        category,
        kind: 'command',
        run: () => undefined,
        ...extra,
    };
}

describe('command palette helpers', () => {
    it('scores title, category, keyword, and acronym matches', () => {
        const grid = item('view.grid', 'Grid View', 'View', { keywords: ['gallery'] });
        const smart = item('scope.smart.best', 'Best Midjourney Picks', 'Smart Collection');

        expect(scoreCommandPaletteItem('grid', grid)).toBeGreaterThan(0);
        expect(scoreCommandPaletteItem('view grid', grid)).toBeGreaterThan(0);
        expect(scoreCommandPaletteItem('gallery', grid)).toBeGreaterThan(0);
        expect(scoreCommandPaletteItem('bmp', smart)).toBeGreaterThan(0);
        expect(scoreCommandPaletteItem('missing', grid)).toBe(0);
    });

    it('sorts pinned items before otherwise stronger matches', () => {
        const items = [
            item('app.search', 'Search Images'),
            item('view.grid', 'Grid View'),
        ];

        expect(sortCommandPaletteItems(items, '', { pinnedIds: ['view.grid'] }).map(i => i.id)).toEqual([
            'view.grid',
            'app.search',
        ]);
    });

    it('filters destinations in command-only mode', () => {
        const items: CommandPaletteItem[] = [
            item('app.search', 'Search Images'),
            item('scope.collection.a', 'A Collection', 'Collection', { kind: 'destination' }),
        ];

        expect(sortCommandPaletteItems(items, '', { mode: 'commands' }).map(i => i.id)).toEqual(['app.search']);
    });

    it('formats and matches keyboard shortcuts', () => {
        const event = keyEvent('P', { metaKey: true, shiftKey: true });

        expect(shortcutFromKeyboardEvent(event)).toBe('Cmd+Shift+P');
        expect(eventMatchesShortcut(event, 'Cmd+Shift+P')).toBe(true);
        expect(eventMatchesShortcut(event, 'Cmd+P')).toBe(false);
    });

    it('detects hotkey conflicts against default and custom assignments', () => {
        const items = [
            item('app.search', 'Search Images', 'App', { defaultShortcut: 'Cmd+F' }),
            item('view.grid', 'Grid View'),
            item('view.loupe', 'Loupe View'),
        ];

        expect(getShortcutConflict('Cmd+F', 'view.grid', items, {})).toBe('Search Images');
        expect(getShortcutConflict('Cmd+F', 'app.search', items, {})).toBeNull();
        expect(getShortcutConflict('Cmd+P', 'view.grid', items, {})).toBe('Print');
        expect(getShortcutConflict('Tab', 'view.grid', items, {})).toBe('Cycle to next view');
        expect(getShortcutConflict('Shift+Tab', 'view.grid', items, {})).toBe('Cycle to previous view');
        expect(getShortcutConflict('Cmd+L', 'view.grid', items, { 'view.loupe': 'Cmd+L' })).toBe('Loupe View');
        expect(getShortcutConflict('Cmd+L', 'view.grid', items, {})).toBeNull();
    });

    it('blocks assigning shortcuts already owned by another command or reserved action', () => {
        const items = [
            item('view.grid', 'Grid View', 'View', { defaultShortcut: 'Cmd+1' }),
            item('view.loupe', 'Loupe View'),
        ];

        expect(canAssignCommandHotkey('Cmd+1', 'view.grid', items, {})).toBe(true);
        expect(canAssignCommandHotkey('Cmd+1', 'view.loupe', items, {})).toBe(false);
        expect(canAssignCommandHotkey('Cmd+P', 'view.loupe', items, {})).toBe(false);
        expect(canAssignCommandHotkey('Cmd+L', 'view.loupe', items, {})).toBe(true);
    });
});
