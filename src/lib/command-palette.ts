import { get } from 'svelte/store';
import {
    activeCanvas,
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSession,
    activeSmartCollection,
    collections,
    commandPaletteMode,
    commandPaletteOpen,
    focusedIndex,
    images,
    searchOpen,
    selectedIds,
    sessionCanvases,
    settingsOpen,
    showDetectionBoxes,
    showDetectionInspector,
    showToast,
    sidebarVisible,
    smartCollections,
    folders,
    viewMode,
    zenMode,
    navigateBack,
    navigateTo,
    type CommandPaletteMode,
    type ViewMode,
} from './stores';
import { invalidateImageCache, loadAllImages, loadImagesForCurrentScope } from './image-loading';
import { redo, setDecision, setRating, undo } from './api';

export type CommandPaletteItemKind = 'command' | 'destination';

export interface CommandPaletteItem {
    id: string;
    title: string;
    subtitle?: string;
    category: string;
    kind: CommandPaletteItemKind;
    keywords?: string[];
    defaultShortcut?: string;
    disabled?: boolean;
    run: () => void | Promise<void>;
}

export interface CommandPaletteSortOptions {
    mode?: CommandPaletteMode;
    pinnedIds?: string[];
    recentIds?: string[];
    hotkeys?: Record<string, string>;
}

export const COMMAND_PALETTE_SHORTCUT_POLICY = {
    universalPalette: 'Cmd+K',
    commandOnlyPalette: 'Cmd+Shift+P',
    imageSearch: ['/', 'Cmd+F'],
    obsidianPresetCandidate: 'Cmd+P',
};

export const COMMAND_PINS_STORAGE_KEY = 'cull.commandPalette.pins';
export const COMMAND_RECENTS_STORAGE_KEY = 'cull.commandPalette.recents';
export const COMMAND_HOTKEYS_STORAGE_KEY = 'cull.commandPalette.hotkeys';

const BUILT_IN_SHORTCUT_LABELS: Record<string, string> = {
    'Cmd+K': 'Open command palette',
    'Cmd+Shift+P': 'Open command-only palette',
    'Cmd+P': 'Print',
    'Cmd+F': 'Image search',
    '/': 'Image search',
    'Cmd+B': 'Toggle sidebar',
    'Cmd+Z': 'Undo',
    'Cmd+Shift+Z': 'Redo',
    'Cmd+1': 'Grid view',
    'Cmd+2': 'Loupe view',
    'Cmd+3': 'Compare view',
    'Cmd+4': 'Canvas view',
    'Cmd+5': 'Lineage view',
    'Cmd+6': 'Embeddings view',
    'Cmd+7': 'Export view',
    'Cmd+8': 'Tinder view',
    'Tab': 'Cycle to next view',
    'Shift+Tab': 'Cycle to previous view',
    'Backspace': 'Move focused image to Trash',
    'Cmd+Backspace': 'Delete focused image permanently',
};

const VIEW_COMMANDS: Array<{ mode: ViewMode; title: string; subtitle: string; shortcut: string }> = [
    { mode: 'grid', title: 'Grid View', subtitle: 'Browse thumbnails', shortcut: 'Cmd+1' },
    { mode: 'loupe', title: 'Loupe View', subtitle: 'Inspect the focused image', shortcut: 'Cmd+2' },
    { mode: 'compare', title: 'Compare View', subtitle: 'Compare selected or adjacent images', shortcut: 'Cmd+3' },
    { mode: 'canvas', title: 'Canvas View', subtitle: 'Arrange selected images spatially', shortcut: 'Cmd+4' },
    { mode: 'lineage', title: 'Lineage View', subtitle: 'Review related generations', shortcut: 'Cmd+5' },
    { mode: 'embeddings', title: 'Embeddings View', subtitle: 'Explore visual clusters', shortcut: 'Cmd+6' },
    { mode: 'export', title: 'Export View', subtitle: 'Prepare images for publishing', shortcut: 'Cmd+7' },
    { mode: 'tinder', title: 'Tinder View', subtitle: 'Fast accept or reject triage', shortcut: 'Cmd+8' },
];

function storageAvailable(): boolean {
    return typeof localStorage !== 'undefined';
}

function readJson<T>(key: string, fallback: T): T {
    if (!storageAvailable()) return fallback;
    try {
        const raw = localStorage.getItem(key);
        return raw ? JSON.parse(raw) as T : fallback;
    } catch {
        return fallback;
    }
}

function writeJson<T>(key: string, value: T) {
    if (!storageAvailable()) return;
    localStorage.setItem(key, JSON.stringify(value));
}

function uniqueList(values: string[]): string[] {
    return Array.from(new Set(values.filter(Boolean)));
}

export function readPinnedCommandIds(): string[] {
    return uniqueList(readJson<string[]>(COMMAND_PINS_STORAGE_KEY, []));
}

export function setCommandPinned(id: string, pinned: boolean): string[] {
    const current = readPinnedCommandIds();
    const next = pinned ? uniqueList([id, ...current]) : current.filter(item => item !== id);
    writeJson(COMMAND_PINS_STORAGE_KEY, next);
    return next;
}

export function readRecentCommandIds(): string[] {
    return uniqueList(readJson<string[]>(COMMAND_RECENTS_STORAGE_KEY, []));
}

export function recordCommandUse(id: string): string[] {
    const next = uniqueList([id, ...readRecentCommandIds()]).slice(0, 16);
    writeJson(COMMAND_RECENTS_STORAGE_KEY, next);
    return next;
}

export function removeRecentCommand(id: string): string[] {
    const next = readRecentCommandIds().filter(item => item !== id);
    writeJson(COMMAND_RECENTS_STORAGE_KEY, next);
    return next;
}

export function readCommandHotkeys(): Record<string, string> {
    return readJson<Record<string, string>>(COMMAND_HOTKEYS_STORAGE_KEY, {});
}

export function setCommandHotkey(id: string, shortcut: string | null): Record<string, string> {
    const hotkeys = { ...readCommandHotkeys() };
    for (const [commandId, existing] of Object.entries(hotkeys)) {
        if (existing === shortcut || commandId === id) {
            delete hotkeys[commandId];
        }
    }
    if (shortcut) hotkeys[id] = shortcut;
    writeJson(COMMAND_HOTKEYS_STORAGE_KEY, hotkeys);
    return hotkeys;
}

export function shortcutForItem(item: CommandPaletteItem, hotkeys: Record<string, string>): string | undefined {
    return hotkeys[item.id] ?? item.defaultShortcut;
}

function clearNavigationScope() {
    activeSession.set(null);
    sessionCanvases.set([]);
    activeCanvas.set(null);
}

async function openAllImages() {
    clearNavigationScope();
    await loadAllImages();
}

async function openCollection(id: string) {
    clearNavigationScope();
    activeSmartCollection.set(null);
    activeFolder.set(null);
    activeDetectedClass.set(null);
    activeCollection.set(id);
    await loadImagesForCurrentScope();
}

async function openFolder(path: string) {
    clearNavigationScope();
    activeSmartCollection.set(null);
    activeCollection.set(null);
    activeDetectedClass.set(null);
    activeFolder.set(path);
    await loadImagesForCurrentScope();
}

async function openSmartCollection(id: string) {
    const smart = get(smartCollections).find(item => item.id === id);
    if (!smart) return;
    clearNavigationScope();
    activeSmartCollection.set(smart);
    activeFolder.set(null);
    activeCollection.set(null);
    activeDetectedClass.set(null);
    await loadImagesForCurrentScope();
}

async function setFocusedRating(rating: number) {
    const idx = get(focusedIndex);
    const image = get(images)[idx];
    if (!image) return;
    await setRating(image.image.id, rating, get(activeSession)?.id ?? null);
    invalidateImageCache();
    images.update(all => {
        const next = [...all];
        const item = { ...next[idx] };
        item.selection = {
            image_id: image.image.id,
            project_id: item.selection?.project_id ?? null,
            star_rating: rating,
            color_label: item.selection?.color_label ?? null,
            decision: item.selection?.decision ?? 'undecided',
        };
        next[idx] = item;
        return next;
    });
}

async function setFocusedDecision(decision: 'accept' | 'reject' | 'undecided') {
    const idx = get(focusedIndex);
    const image = get(images)[idx];
    if (!image) return;
    await setDecision(image.image.id, decision, get(activeSession)?.id ?? null);
    invalidateImageCache();
    images.update(all => {
        const next = [...all];
        const item = { ...next[idx] };
        item.selection = {
            image_id: image.image.id,
            project_id: item.selection?.project_id ?? null,
            star_rating: item.selection?.star_rating ?? null,
            color_label: item.selection?.color_label ?? null,
            decision,
        };
        next[idx] = item;
        return next;
    });
}

function focusedImageTitle(): string {
    const image = get(images)[get(focusedIndex)];
    return image?.path.split('/').pop() ?? 'focused image';
}

function commandItems(): CommandPaletteItem[] {
    const hasImage = Boolean(get(images)[get(focusedIndex)]);
    const selectedCount = get(selectedIds).size;

    return [
        {
            id: 'app.search',
            title: 'Search Images',
            subtitle: 'Natural-language image filter',
            category: 'App',
            kind: 'command',
            keywords: ['find', 'filter', 'smart collection', 'query'],
            defaultShortcut: 'Cmd+F',
            run: () => searchOpen.set(true),
        },
        {
            id: 'app.settings',
            title: 'Open Settings',
            subtitle: 'MCP, publishing, and app settings',
            category: 'App',
            kind: 'command',
            keywords: ['preferences', 'configuration', 'mcp'],
            run: () => settingsOpen.set(true),
        },
        {
            id: 'app.reload-images',
            title: 'Reload Current View',
            subtitle: 'Refresh image data for the active scope',
            category: 'App',
            kind: 'command',
            keywords: ['refresh', 'rescan'],
            run: () => {
                window.dispatchEvent(new CustomEvent('reload-images'));
            },
        },
        {
            id: 'app.toggle-sidebar',
            title: 'Toggle Sidebar',
            subtitle: get(sidebarVisible) ? 'Hide sidebar' : 'Show sidebar',
            category: 'App',
            kind: 'command',
            keywords: ['left panel', 'navigation'],
            defaultShortcut: 'Cmd+B',
            run: () => sidebarVisible.update(value => !value),
        },
        {
            id: 'app.toggle-zen',
            title: 'Toggle Zen Mode',
            subtitle: get(zenMode) ? 'Leave focused layout' : 'Hide chrome and focus on images',
            category: 'App',
            kind: 'command',
            keywords: ['fullscreen', 'focus', 'immersive'],
            run: () => zenMode.update(value => !value),
        },
        {
            id: 'view.back',
            title: 'Back to Previous View',
            subtitle: 'Return to the last view mode',
            category: 'View',
            kind: 'command',
            keywords: ['history', 'previous'],
            run: () => {
                navigateBack();
            },
        },
        ...VIEW_COMMANDS.map(({ mode, title, subtitle, shortcut }): CommandPaletteItem => ({
            id: `view.${mode}`,
            title,
            subtitle: get(viewMode) === mode ? 'Current view' : subtitle,
            category: 'View',
            kind: 'command',
            keywords: [mode, 'tab', 'mode'],
            defaultShortcut: shortcut,
            run: () => navigateTo(mode),
        })),
        {
            id: 'edit.undo',
            title: 'Undo',
            subtitle: 'Undo the last library action',
            category: 'Edit',
            kind: 'command',
            keywords: ['history'],
            defaultShortcut: 'Cmd+Z',
            run: async () => {
                const label = await undo();
                if (label) {
                    showToast(`Undone: ${label}`, { type: 'info', duration: 4000 });
                    window.dispatchEvent(new CustomEvent('reload-images'));
                }
            },
        },
        {
            id: 'edit.redo',
            title: 'Redo',
            subtitle: 'Redo the last undone library action',
            category: 'Edit',
            kind: 'command',
            keywords: ['history'],
            defaultShortcut: 'Cmd+Shift+Z',
            run: async () => {
                const label = await redo();
                if (label) {
                    showToast(`Redone: ${label}`, { type: 'info', duration: 4000 });
                    window.dispatchEvent(new CustomEvent('reload-images'));
                }
            },
        },
        {
            id: 'selection.clear',
            title: 'Clear Selection',
            subtitle: `${selectedCount} selected`,
            category: 'Selection',
            kind: 'command',
            keywords: ['deselect'],
            disabled: selectedCount === 0,
            run: () => selectedIds.set(new Set()),
        },
        {
            id: 'image.trash',
            title: 'Move Focused Image to Trash',
            subtitle: focusedImageTitle(),
            category: 'Image',
            kind: 'command',
            keywords: ['delete', 'remove'],
            defaultShortcut: 'Backspace',
            disabled: !hasImage,
            run: () => {
                window.dispatchEvent(new CustomEvent('trash-focused-image'));
            },
        },
        {
            id: 'image.delete-permanently',
            title: 'Delete Focused Image Permanently',
            subtitle: focusedImageTitle(),
            category: 'Image',
            kind: 'command',
            keywords: ['remove', 'destroy'],
            defaultShortcut: 'Cmd+Backspace',
            disabled: !hasImage,
            run: () => {
                window.dispatchEvent(new CustomEvent('delete-focused-image'));
            },
        },
        ...[0, 1, 2, 3, 4, 5].map((rating): CommandPaletteItem => ({
            id: `image.rating.${rating}`,
            title: rating === 0 ? 'Clear Focused Image Rating' : `Rate Focused Image ${rating} Star${rating === 1 ? '' : 's'}`,
            subtitle: focusedImageTitle(),
            category: 'Image',
            kind: 'command',
            keywords: ['star', 'rank', 'score'],
            disabled: !hasImage,
            run: () => setFocusedRating(rating),
        })),
        ...(['accept', 'reject', 'undecided'] as const).map((decision): CommandPaletteItem => ({
            id: `image.decision.${decision}`,
            title: `Mark Focused Image ${decision.charAt(0).toUpperCase()}${decision.slice(1)}`,
            subtitle: focusedImageTitle(),
            category: 'Image',
            kind: 'command',
            keywords: ['pick', 'cull', 'triage'],
            disabled: !hasImage,
            run: () => setFocusedDecision(decision),
        })),
        {
            id: 'detection.toggle-boxes',
            title: 'Toggle Detection Boxes',
            subtitle: get(showDetectionBoxes) ? 'Hide detection overlays' : 'Show detection overlays',
            category: 'AI',
            kind: 'command',
            keywords: ['objects', 'overlay'],
            run: () => showDetectionBoxes.update(value => !value),
        },
        {
            id: 'detection.toggle-inspector',
            title: 'Toggle Detection Inspector',
            subtitle: get(showDetectionInspector) ? 'Hide inspector' : 'Show inspector',
            category: 'AI',
            kind: 'command',
            keywords: ['objects', 'metadata', 'panel'],
            run: () => showDetectionInspector.update(value => !value),
        },
    ];
}

function destinationItems(): CommandPaletteItem[] {
    const activeCollectionId = get(activeCollection);
    const activeFolderPath = get(activeFolder);
    const activeSmartId = get(activeSmartCollection)?.id ?? null;

    return [
        {
            id: 'scope.all',
            title: 'All Images',
            subtitle: 'Library root',
            category: 'Destination',
            kind: 'destination',
            keywords: ['library', 'root'],
            run: openAllImages,
        },
        ...get(smartCollections)
            .filter(item => Boolean(item.filter_json))
            .map((item): CommandPaletteItem => ({
                id: `scope.smart.${item.id}`,
                title: item.name,
                subtitle: item.id === activeSmartId ? 'Current smart collection' : `${item.image_count ?? 0} images`,
                category: 'Smart Collection',
                kind: 'destination',
                keywords: ['smart', 'filter', item.nl_query ?? '', item.description ?? ''],
                run: () => openSmartCollection(item.id),
            })),
        ...get(collections).map(([id, name, count]): CommandPaletteItem => ({
            id: `scope.collection.${id}`,
            title: name,
            subtitle: id === activeCollectionId ? 'Current collection' : `${count} images`,
            category: 'Collection',
            kind: 'destination',
            keywords: ['collection', id],
            run: () => openCollection(id),
        })),
        ...get(folders).map(([path, count]): CommandPaletteItem => ({
            id: `scope.folder.${path}`,
            title: path.split('/').filter(Boolean).pop() ?? path,
            subtitle: path === activeFolderPath ? 'Current folder' : path,
            category: 'Folder',
            kind: 'destination',
            keywords: ['folder', path],
            run: () => openFolder(path),
        })),
    ];
}

export function getCommandPaletteItems(mode: CommandPaletteMode = get(commandPaletteMode)): CommandPaletteItem[] {
    const commands = commandItems();
    if (mode === 'commands') return commands;
    return [...commands, ...destinationItems()];
}

function normalize(value: string): string {
    return value.toLowerCase().replace(/[^a-z0-9/]+/g, ' ').trim();
}

function compact(value: string): string {
    return normalize(value).replace(/\s+/g, '');
}

function acronym(value: string): string {
    return normalize(value)
        .split(/\s+/)
        .filter(Boolean)
        .map(part => part[0])
        .join('');
}

function termScore(term: string, item: CommandPaletteItem): number {
    const title = normalize(item.title);
    const subtitle = normalize(item.subtitle ?? '');
    const category = normalize(item.category);
    const keywords = normalize((item.keywords ?? []).join(' '));
    const id = normalize(item.id.replace(/\./g, ' '));
    const haystack = `${title} ${subtitle} ${category} ${keywords} ${id}`;

    if (title === term) return 500;
    if (title.startsWith(term)) return 360;
    if (title.split(/\s+/).some(word => word.startsWith(term))) return 260;
    if (acronym(item.title).startsWith(term)) return 230;
    if (category.startsWith(term)) return 190;
    if (subtitle.includes(term)) return 150;
    if (keywords.includes(term)) return 130;
    if (id.includes(term)) return 100;
    if (compact(haystack).includes(compact(term))) return 80;
    return 0;
}

export function scoreCommandPaletteItem(query: string, item: CommandPaletteItem): number {
    const terms = normalize(query).split(/\s+/).filter(Boolean);
    if (terms.length === 0) return 1;
    let score = 0;
    for (const term of terms) {
        const current = termScore(term, item);
        if (current === 0) return 0;
        score += current;
    }
    return score + (item.kind === 'command' ? 8 : 0);
}

export function sortCommandPaletteItems(
    items: CommandPaletteItem[],
    query: string,
    options: CommandPaletteSortOptions = {},
): CommandPaletteItem[] {
    const pinned = new Set(options.pinnedIds ?? []);
    const recent = options.recentIds ?? [];
    const hotkeys = options.hotkeys ?? {};
    const mode = options.mode ?? 'all';

    return items
        .filter(item => mode === 'all' || item.kind === 'command')
        .map(item => ({
            item,
            score: scoreCommandPaletteItem(query, item),
            pinnedRank: pinned.has(item.id) ? 1 : 0,
            recentRank: recent.includes(item.id) ? recent.length - recent.indexOf(item.id) : 0,
            hasShortcut: shortcutForItem(item, hotkeys) ? 1 : 0,
        }))
        .filter(entry => entry.score > 0)
        .sort((a, b) =>
            b.pinnedRank - a.pinnedRank ||
            b.score - a.score ||
            b.recentRank - a.recentRank ||
            b.hasShortcut - a.hasShortcut ||
            a.item.title.localeCompare(b.item.title)
        )
        .map(entry => entry.item);
}

function shortcutParts(shortcut: string) {
    const parts = shortcut.split('+').filter(Boolean);
    const key = parts[parts.length - 1] ?? '';
    return {
        meta: parts.includes('Cmd'),
        shift: parts.includes('Shift'),
        alt: parts.includes('Option'),
        ctrl: parts.includes('Ctrl'),
        key,
    };
}

export function shortcutFromKeyboardEvent(event: KeyboardEvent): string | null {
    const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
    if (['Meta', 'Shift', 'Alt', 'Control'].includes(key)) return null;
    const parts = [
        event.metaKey ? 'Cmd' : '',
        event.ctrlKey ? 'Ctrl' : '',
        event.altKey ? 'Option' : '',
        event.shiftKey ? 'Shift' : '',
        key,
    ].filter(Boolean);
    return parts.length > 0 ? parts.join('+') : null;
}

export function eventMatchesShortcut(event: KeyboardEvent, shortcut: string): boolean {
    const parts = shortcutParts(shortcut);
    const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
    return event.metaKey === parts.meta &&
        event.shiftKey === parts.shift &&
        event.altKey === parts.alt &&
        event.ctrlKey === parts.ctrl &&
        key === parts.key;
}

export function getShortcutConflict(
    shortcut: string,
    currentItemId: string,
    items: CommandPaletteItem[],
    hotkeys: Record<string, string>,
): string | null {
    const current = items.find(item => item.id === currentItemId);
    if (current?.defaultShortcut === shortcut) return null;
    const existing = items.find(item =>
        item.id !== currentItemId &&
        shortcutForItem(item, hotkeys) === shortcut
    );
    if (existing) return existing.title;
    const builtin = BUILT_IN_SHORTCUT_LABELS[shortcut];
    return builtin ?? null;
}

export function canAssignCommandHotkey(
    shortcut: string,
    currentItemId: string,
    items: CommandPaletteItem[],
    hotkeys: Record<string, string>,
): boolean {
    return !shortcut || getShortcutConflict(shortcut, currentItemId, items, hotkeys) === null;
}

export async function runCommandPaletteItem(item: CommandPaletteItem) {
    if (item.disabled) return;
    await item.run();
}

export function commandForKeyboardEvent(event: KeyboardEvent): CommandPaletteItem | null {
    const hotkeys = readCommandHotkeys();
    const items = getCommandPaletteItems('all');
    const item = items.find(candidate => {
        const shortcut = hotkeys[candidate.id];
        return shortcut ? eventMatchesShortcut(event, shortcut) : false;
    });
    return item && !item.disabled ? item : null;
}

export async function runCommandForKeyboardEvent(event: KeyboardEvent): Promise<boolean> {
    const item = commandForKeyboardEvent(event);
    if (!item) return false;
    await runCommandPaletteItem(item);
    recordCommandUse(item.id);
    return true;
}

export function openCommandPalette(mode: CommandPaletteMode = 'all') {
    commandPaletteMode.set(mode);
    commandPaletteOpen.set(true);
}
