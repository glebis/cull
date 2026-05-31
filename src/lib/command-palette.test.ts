import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
    canAssignCommandHotkey,
    eventMatchesShortcut,
    findDuplicateCommandHotkeys,
    getCommandPaletteItems,
    getShortcutConflict,
    readRecentCommandIds,
    recordCommandUse,
    scoreCommandPaletteItem,
    shortcutFromKeyboardEvent,
    sortCommandPaletteItems,
    type CommandPaletteItem,
} from './command-palette';
import {
    collectMode,
    collectModeTarget,
    collections,
    focusedIndex,
    images,
    selectedIds,
    staticPublishingEnabled,
} from './stores';

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
    beforeEach(() => {
        const store = new Map<string, string>();
        vi.stubGlobal('localStorage', {
            getItem: (key: string) => store.get(key) ?? null,
            setItem: (key: string, value: string) => store.set(key, value),
            removeItem: (key: string) => store.delete(key),
            clear: () => store.clear(),
        });
    });

    function resetCommandContext() {
        images.set([]);
        focusedIndex.set(0);
        selectedIds.set(new Set());
        collections.set([]);
        collectMode.set(false);
        collectModeTarget.set(null);
        staticPublishingEnabled.set(false);
    }

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

    it('shows the five most recent palette commands at the top before other empty-query results', () => {
        const items = [
            item('app.search', 'Search Images'),
            item('view.grid', 'Grid View'),
            item('view.loupe', 'Loupe View'),
            item('view.compare', 'Compare View'),
            item('view.canvas', 'Canvas View'),
            item('view.lineage', 'Lineage View'),
            item('view.export', 'Export View'),
        ];

        expect(sortCommandPaletteItems(items, '', {
            recentIds: ['view.canvas', 'view.loupe', 'view.export', 'view.compare', 'view.grid'],
        }).map(i => i.id).slice(0, 5)).toEqual([
            'view.canvas',
            'view.loupe',
            'view.export',
            'view.compare',
            'view.grid',
        ]);
    });

    it('hides recent commands that do not match a non-empty query', () => {
        const items = [
            item('app.search', 'Search Images'),
            item('view.grid', 'Grid View'),
            item('view.loupe', 'Loupe View'),
        ];

        expect(sortCommandPaletteItems(items, 'grid', {
            recentIds: ['view.loupe', 'app.search'],
        }).map(i => i.id)).toEqual(['view.grid']);
    });

    it('keeps only the five last palette-used commands in recents', () => {
        for (const id of ['one', 'two', 'three', 'four', 'five', 'six']) {
            recordCommandUse(id);
        }

        expect(readRecentCommandIds()).toEqual(['six', 'five', 'four', 'three', 'two']);
    });

    it('filters destinations in command-only mode', () => {
        const items: CommandPaletteItem[] = [
            item('app.search', 'Search Images'),
            item('scope.collection.a', 'A Collection', 'Collection', { kind: 'destination' }),
        ];

        expect(sortCommandPaletteItems(items, '', { mode: 'commands' }).map(i => i.id)).toEqual(['app.search']);
    });

    it('filters items hidden by context predicates', () => {
        const items = [
            item('app.visible', 'Visible Command', 'App', { when: () => true }),
            item('app.hidden', 'Hidden Command', 'App', { when: () => false }),
        ];

        expect(sortCommandPaletteItems(items, '').map(i => i.id)).toEqual(['app.visible']);
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
        expect(getShortcutConflict('Cmd+P', 'view.grid', items, {})).toBe('Open command palette');
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

    it('reports duplicate command hotkeys', () => {
        const items = [
            item('view.grid', 'Grid View', 'View', { defaultShortcut: 'Cmd+1' }),
            item('view.loupe', 'Loupe View', 'View', { defaultShortcut: 'Cmd+2' }),
            item('view.compare', 'Compare View', 'View'),
        ];

        expect(findDuplicateCommandHotkeys(items, {
            'view.compare': 'Cmd+1',
        })).toEqual([
            {
                shortcut: 'Cmd+1',
                commandIds: ['view.grid', 'view.compare'],
                titles: ['Grid View', 'Compare View'],
            },
        ]);
    });

    it('assigns Cmd+0 to Actual Size and moves Export View to Cmd+7', () => {
        resetCommandContext();
        const items = getCommandPaletteItems('commands');
        const exportView = items.find(i => i.id === 'view.export');
        const actualSize = items.find(i => i.id === 'view.actual-size');

        expect(actualSize?.defaultShortcut).toBe('Cmd+0');
        expect(exportView?.defaultShortcut).toBe('Cmd+7');
        expect(getShortcutConflict('Cmd+0', 'view.grid', items, {})).toBe('Actual Size');
        expect(getShortcutConflict('Cmd+7', 'view.grid', items, {})).toBe('Export View');
        expect(findDuplicateCommandHotkeys(items, {})).toEqual([]);
    });

    it('shows Publish View before Export View only when Static Publishing is enabled', () => {
        resetCommandContext();
        expect(getCommandPaletteItems('commands').map(i => i.id)).not.toContain('view.publish');

        staticPublishingEnabled.set(true);
        const ids = getCommandPaletteItems('commands').map(i => i.id);

        expect(ids).toContain('view.publish');
        expect(ids.indexOf('view.publish')).toBe(ids.indexOf('view.export') - 1);
    });

    it('includes collection workflow commands', () => {
        resetCommandContext();
        const ids = getCommandPaletteItems('commands').map(i => i.id);

        expect(ids).toContain('collection.create-from-selection');
        expect(ids).toContain('collection.create-from-unselected');
        expect(ids).toContain('collection.toggle-collect-mode');
        expect(ids).toContain('collection.add-focused-to-collect-target');
    });

    it('disables collect-focused outside collect mode', () => {
        resetCommandContext();
        images.set([{
            image: { id: 'img-1' },
            path: '/tmp/img-1.png',
            thumbnail_path: null,
            selection: null,
        } as never]);

        const command = getCommandPaletteItems('commands')
            .find(i => i.id === 'collection.add-focused-to-collect-target');

        expect(command?.disabled).toBe(true);
    });

    it('enables collect-focused with a target and focused image', () => {
        resetCommandContext();
        collectMode.set(true);
        collectModeTarget.set('col-1');
        images.set([{
            image: { id: 'img-1' },
            path: '/tmp/img-1.png',
            thumbnail_path: null,
            selection: null,
        } as never]);

        const command = getCommandPaletteItems('commands')
            .find(i => i.id === 'collection.add-focused-to-collect-target');

        expect(command?.disabled).toBe(false);
    });
});
