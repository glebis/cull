import { beforeEach, describe, expect, it } from 'vitest';
import {
    canAssignCommandHotkey,
    eventMatchesShortcut,
    findDuplicateCommandHotkeys,
    getCommandPaletteItems,
    getShortcutConflict,
    listCommandShortcuts,
    pruneStalePins,
    readCommandFrequencies,
    readCommandHotkeys,
    recordCommandUse,
    resetCommandHotkeys,
    scoreCommandPaletteItem,
    setCommandHotkey,
    shortcutFromKeyboardEvent,
    setCommandPinned,
    readPinnedCommandIds,
    sortCommandPaletteItems,
    type CommandPaletteItem,
} from './command-palette';
import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSmartCollection,
    collectMode,
    collectModeTarget,
    collections,
    detectedClasses,
    focusedIndex,
    folders,
    images,
    selectedIds,
    sessions,
    sessionCanvases,
    smartCollections,
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

describe('command palette destination providers', () => {
    function resetDestinations() {
        images.set([]);
        focusedIndex.set(0);
        collections.set([]);
        smartCollections.set([]);
        folders.set([]);
        sessions.set([]);
        sessionCanvases.set([]);
        detectedClasses.set([]);
        activeCollection.set(null);
        activeFolder.set(null);
        activeSmartCollection.set(null);
        activeDetectedClass.set(null);
    }

    it('always offers the All Images destination', () => {
        resetDestinations();
        const ids = getCommandPaletteItems('all').map(i => i.id);
        expect(ids).toContain('scope.all');
    });

    it('exposes sessions as destinations with image counts', () => {
        resetDestinations();
        sessions.set([
            { id: 's1', name: 'Wedding Shoot', description: null, folder_path: '/p', settings_json: null, created_at: '', image_count: 42 },
        ] as never);

        const session = getCommandPaletteItems('all').find(i => i.id === 'scope.session.s1');
        expect(session).toBeTruthy();
        expect(session?.kind).toBe('destination');
        expect(session?.category).toBe('Session');
        expect(session?.title).toBe('Wedding Shoot');
        expect(session?.subtitle).toContain('42');
    });

    it('exposes canvases of the active session as destinations', () => {
        resetDestinations();
        sessionCanvases.set([
            { id: 'c1', session_id: 's1', name: 'Hero Wall', canvas_type: 'manual', layout_json: '{}', filter_json: null, grid_config_json: null, sort_order: 0, created_at: '', updated_at: '' },
        ] as never);

        const canvas = getCommandPaletteItems('all').find(i => i.id === 'scope.canvas.c1');
        expect(canvas).toBeTruthy();
        expect(canvas?.category).toBe('Canvas');
        expect(canvas?.title).toBe('Hero Wall');
    });

    it('exposes detected classes as destinations with counts', () => {
        resetDestinations();
        detectedClasses.set([['person', 12], ['dog', 3]]);

        const person = getCommandPaletteItems('all').find(i => i.id === 'scope.detected.person');
        expect(person).toBeTruthy();
        expect(person?.category).toBe('Detection');
        expect(person?.subtitle).toContain('12');
    });

    it('omits destinations in command-only mode', () => {
        resetDestinations();
        sessions.set([
            { id: 's1', name: 'Wedding Shoot', description: null, folder_path: '/p', settings_json: null, created_at: '', image_count: 1 },
        ] as never);
        detectedClasses.set([['person', 1]]);

        const ids = getCommandPaletteItems('commands').map(i => i.id);
        expect(ids).not.toContain('scope.session.s1');
        expect(ids).not.toContain('scope.detected.person');
        expect(ids).not.toContain('scope.all');
    });
});

describe('command palette frequency and pin persistence', () => {
    beforeEach(() => {
        const store = new Map<string, string>();
        const stub = {
            getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
            setItem: (k: string, v: string) => void store.set(k, String(v)),
            removeItem: (k: string) => void store.delete(k),
            clear: () => store.clear(),
        };
        (globalThis as { localStorage?: unknown }).localStorage = stub;
    });

    it('counts command usage frequency across calls', () => {
        recordCommandUse('view.grid');
        recordCommandUse('view.grid');
        recordCommandUse('view.loupe');

        const freq = readCommandFrequencies();
        expect(freq['view.grid']).toBe(2);
        expect(freq['view.loupe']).toBe(1);
    });

    it('ranks more frequently used items above less frequent ones at equal score', () => {
        const items = [
            item('view.grid', 'Grid View'),
            item('view.loupe', 'Loupe View'),
        ];
        // Same fuzzy score (empty query), differ only by frequency.
        const sorted = sortCommandPaletteItems(items, '', {
            frequencies: { 'view.loupe': 5, 'view.grid': 1 },
        });
        expect(sorted.map(i => i.id)).toEqual(['view.loupe', 'view.grid']);
    });

    it('still ranks pinned items above more frequent unpinned items', () => {
        const items = [
            item('view.grid', 'Grid View'),
            item('view.loupe', 'Loupe View'),
        ];
        const sorted = sortCommandPaletteItems(items, '', {
            pinnedIds: ['view.grid'],
            frequencies: { 'view.loupe': 99 },
        });
        expect(sorted[0].id).toBe('view.grid');
    });

    it('prunes pinned ids that no longer correspond to live items', () => {
        setCommandPinned('view.grid', true);
        setCommandPinned('scope.collection.deleted', true);
        expect(readPinnedCommandIds()).toContain('scope.collection.deleted');

        const kept = pruneStalePins(['view.grid', 'view.loupe']);
        expect(kept).toContain('view.grid');
        expect(kept).not.toContain('scope.collection.deleted');
        expect(readPinnedCommandIds()).not.toContain('scope.collection.deleted');
    });
});

describe('command palette shortcut inspection', () => {
    beforeEach(() => {
        const store = new Map<string, string>();
        (globalThis as { localStorage?: unknown }).localStorage = {
            getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
            setItem: (k: string, v: string) => void store.set(k, String(v)),
            removeItem: (k: string) => void store.delete(k),
            clear: () => store.clear(),
        };
    });

    it('lists every command with its effective shortcut and custom/default origin', () => {
        const items = [
            item('view.grid', 'Grid View', 'View', { defaultShortcut: 'Cmd+1' }),
            item('view.loupe', 'Loupe View', 'View'),
        ];
        const rows = listCommandShortcuts(items, { 'view.loupe': 'Cmd+L' });

        const grid = rows.find(r => r.id === 'view.grid');
        const loupe = rows.find(r => r.id === 'view.loupe');
        expect(grid?.shortcut).toBe('Cmd+1');
        expect(grid?.isCustom).toBe(false);
        expect(loupe?.shortcut).toBe('Cmd+L');
        expect(loupe?.isCustom).toBe(true);
    });

    it('flags rows that duplicate another binding', () => {
        const items = [
            item('view.grid', 'Grid View', 'View', { defaultShortcut: 'Cmd+1' }),
            item('view.loupe', 'Loupe View', 'View'),
        ];
        const rows = listCommandShortcuts(items, { 'view.loupe': 'Cmd+1' });
        expect(rows.find(r => r.id === 'view.grid')?.conflict).toBe(true);
        expect(rows.find(r => r.id === 'view.loupe')?.conflict).toBe(true);
    });

    it('persists command aliases and makes them searchable', async () => {
        const { setCommandAlias, readCommandAliases, applyCommandAliases } = await import('./command-palette');
        setCommandAlias('view.grid', 'gallery wall');
        expect(readCommandAliases()['view.grid']).toBe('gallery wall');

        const items = applyCommandAliases([item('view.grid', 'Grid View', 'View')], readCommandAliases());
        // The alias term should now produce a positive fuzzy score.
        expect(scoreCommandPaletteItem('gallery', items[0])).toBeGreaterThan(0);
    });

    it('clears an alias when set to null', async () => {
        const { setCommandAlias, readCommandAliases } = await import('./command-palette');
        setCommandAlias('view.grid', 'gallery');
        setCommandAlias('view.grid', null);
        expect(readCommandAliases()['view.grid']).toBeUndefined();
    });

    it('reset clears all custom hotkeys but leaves defaults intact', () => {
        setCommandHotkey('view.loupe', 'Cmd+L');
        expect(readCommandHotkeys()['view.loupe']).toBe('Cmd+L');

        resetCommandHotkeys();
        expect(readCommandHotkeys()).toEqual({});

        const items = [item('view.loupe', 'Loupe View', 'View', { defaultShortcut: 'Cmd+2' })];
        const rows = listCommandShortcuts(items, readCommandHotkeys());
        expect(rows[0].shortcut).toBe('Cmd+2');
        expect(rows[0].isCustom).toBe(false);
    });
});
