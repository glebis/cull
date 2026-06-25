import { get } from 'svelte/store';
import {
    activeCanvas,
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSession,
    activeSmartCollection,
    agentPanelPinned,
    agentPanelVisible,
    agentVisualLevel,
    collectMode,
    collectModeTarget,
    collections,
    commandPaletteMode,
    commandPaletteOpen,
    contactSheetOpen,
    detectedClasses,
    exportFolderOpen,
    groupRankingOpen,
    focusedIndex,
    images,
    requestCollectionTarget,
    requestTextInput,
    searchOpen,
    selectedIds,
    sessions,
    shortcutsOpen,
    sessionCanvases,
    settingsOpen,
    showDetectionBoxes,
    showDetectionInspector,
    showToast,
    sidebarVisible,
    smartCollections,
    folders,
    statusHint,
    resetLoupeTransform,
    clientToolsEnabled,
    cycleAgentVisualLevel,
    viewMode,
    zenMode,
    navigateBack,
    navigateTo,
    type CommandPaletteMode,
    type ViewMode,
} from './stores';
import { invalidateImageCache, loadAllImages, loadImagesForCurrentScope } from './image-loading';
import { addToCollection, createCollection, getClientFeedback, listCanvases, listClientFeedback, listCollections, redo, saveTextToPath, setClientFeedback, setDecision, setRating, undo, validateSessionFolder, type Canvas, type Session } from './api';
import { withDecision, withRating, type ImageDecision } from './selection-updates';
import { createWorkflow, readWorkflows, runWorkflow, type CommandWorkflow } from './workflows';
import { buildDeliveryCsv, type DeliveryRow } from './delivery-csv';
import { loadSimilarImages } from './similarity';
import { getPluginPaletteCommands } from './plugins/loader';
import { tabRegistry } from './plugins/tab-registry';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';

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
    when?: () => boolean;
    run: () => void | Promise<void>;
}

export interface CommandPaletteSortOptions {
    mode?: CommandPaletteMode;
    pinnedIds?: string[];
    recentIds?: string[];
    frequencies?: Record<string, number>;
    hotkeys?: Record<string, string>;
}

export interface CommandHotkeyDuplicate {
    shortcut: string;
    commandIds: string[];
    titles: string[];
}

export const COMMAND_PALETTE_SHORTCUT_POLICY = {
    universalPalette: 'Cmd+K',
    commandOnlyPalette: 'Cmd+P',
    alternateCommandOnlyPalette: 'Cmd+Shift+P',
    imageSearch: ['/', 'Cmd+F'],
};

export const COMMAND_PINS_STORAGE_KEY = 'cull.commandPalette.pins';
export const COMMAND_RECENTS_STORAGE_KEY = 'cull.commandPalette.recents';
export const COMMAND_FREQUENCY_STORAGE_KEY = 'cull.commandPalette.frequency';
export const COMMAND_HOTKEYS_STORAGE_KEY = 'cull.commandPalette.hotkeys';
export const COMMAND_ALIASES_STORAGE_KEY = 'cull.commandPalette.aliases';

export const BUILT_IN_SHORTCUT_LABELS: Record<string, string> = {
    'Cmd+K': 'Open command palette',
    'Cmd+P': 'Open command palette',
    'Cmd+Shift+P': 'Open command-only palette',
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
    'Cmd+0': 'Actual Size',
    'Cmd+8': 'Speed Review view',
    '?': 'Open keyboard shortcuts help',
    'Ctrl+Tab': 'Cycle to next view',
    'Ctrl+Shift+Tab': 'Cycle to previous view',
    'Backspace': 'Move focused image to Trash',
    'Cmd+Backspace': 'Delete focused image permanently',
};

// View commands are derived from the tab registry (the single source of truth
// for top-level tabs). Cmd+digit shortcuts are an app-level concern that stays
// here, keyed by core tab id; plugin tabs have no built-in digit shortcut.
const CORE_VIEW_SHORTCUTS: Partial<Record<string, string>> = {
    grid: 'Cmd+1', loupe: 'Cmd+2', compare: 'Cmd+3', canvas: 'Cmd+4',
    lineage: 'Cmd+5', embeddings: 'Cmd+6', export: 'Cmd+7', tinder: 'Cmd+8',
};

function buildViewCommands(): Array<{ mode: ViewMode; title: string; subtitle: string; shortcut?: string }> {
    return get(tabRegistry).map(t => ({
        mode: t.id,
        title: t.label,
        subtitle: t.subtitle ?? '',
        shortcut: CORE_VIEW_SHORTCUTS[t.id],
    }));
}

/** Test seam. */
export function viewCommandsForTest() { return buildViewCommands(); }

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

// Drop pinned IDs that no longer correspond to a live palette item (e.g. a
// collection or folder that was deleted), so stale pins do not accumulate.
// Built-in command IDs are always considered valid even if not currently
// visible due to context predicates.
export function pruneStalePins(liveIds: Iterable<string>): string[] {
    const live = new Set(liveIds);
    const pruned = readPinnedCommandIds().filter(id => live.has(id) || !id.startsWith('scope.'));
    writeJson(COMMAND_PINS_STORAGE_KEY, pruned);
    return pruned;
}

export function readRecentCommandIds(): string[] {
    return uniqueList(readJson<string[]>(COMMAND_RECENTS_STORAGE_KEY, []));
}

export function recordCommandUse(id: string): string[] {
    const next = uniqueList([id, ...readRecentCommandIds()]).slice(0, 5);
    writeJson(COMMAND_RECENTS_STORAGE_KEY, next);
    const freq = readCommandFrequencies();
    freq[id] = (freq[id] ?? 0) + 1;
    writeJson(COMMAND_FREQUENCY_STORAGE_KEY, freq);
    return next;
}

export function readCommandFrequencies(): Record<string, number> {
    return readJson<Record<string, number>>(COMMAND_FREQUENCY_STORAGE_KEY, {});
}

export function removeRecentCommand(id: string): string[] {
    const next = readRecentCommandIds().filter(item => item !== id);
    writeJson(COMMAND_RECENTS_STORAGE_KEY, next);
    return next;
}

export function readCommandHotkeys(): Record<string, string> {
    return readJson<Record<string, string>>(COMMAND_HOTKEYS_STORAGE_KEY, {});
}

export function readCommandAliases(): Record<string, string> {
    return readJson<Record<string, string>>(COMMAND_ALIASES_STORAGE_KEY, {});
}

// Set or clear a user alias — extra search terms that make a command easier to
// find by the words the user actually thinks in.
export function setCommandAlias(id: string, alias: string | null): Record<string, string> {
    const aliases = { ...readCommandAliases() };
    const trimmed = alias?.trim();
    if (trimmed) aliases[id] = trimmed;
    else delete aliases[id];
    writeJson(COMMAND_ALIASES_STORAGE_KEY, aliases);
    return aliases;
}

// Fold stored aliases into each item's keywords so the existing fuzzy scorer
// matches them with no scoring-path changes.
export function applyCommandAliases(
    items: CommandPaletteItem[],
    aliases: Record<string, string>,
): CommandPaletteItem[] {
    return items.map(item => {
        const alias = aliases[item.id];
        if (!alias) return item;
        return { ...item, keywords: [...(item.keywords ?? []), alias] };
    });
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

// Clear every custom hotkey assignment, reverting all commands to their defaults.
export function resetCommandHotkeys(): Record<string, string> {
    writeJson(COMMAND_HOTKEYS_STORAGE_KEY, {});
    return {};
}

export interface CommandShortcutRow {
    id: string;
    title: string;
    category: string;
    shortcut?: string;
    isCustom: boolean;
    conflict: boolean;
}

// Flatten the registry into an inspectable list of shortcut rows for the
// keyboard-shortcuts settings surface. Flags any binding that collides with
// another command's effective binding.
export function listCommandShortcuts(
    items: CommandPaletteItem[],
    hotkeys: Record<string, string>,
): CommandShortcutRow[] {
    const counts = new Map<string, number>();
    for (const item of items) {
        const shortcut = shortcutForItem(item, hotkeys);
        if (shortcut) counts.set(shortcut, (counts.get(shortcut) ?? 0) + 1);
    }
    return items.map(item => {
        const shortcut = shortcutForItem(item, hotkeys);
        return {
            id: item.id,
            title: item.title,
            category: item.category,
            shortcut,
            isCustom: Boolean(hotkeys[item.id]),
            conflict: Boolean(shortcut && (counts.get(shortcut) ?? 0) > 1),
        };
    });
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

async function openDetectedClass(className: string) {
    clearNavigationScope();
    activeSmartCollection.set(null);
    activeFolder.set(null);
    activeCollection.set(null);
    activeDetectedClass.set(className);
    await loadImagesForCurrentScope();
}

async function openSession(session: Session) {
    // Mirrors SessionSwitcher.selectSession: validate the folder, load its
    // canvases, then switch the active session and reload images for the scope.
    activeCanvas.set(null);
    try {
        const valid = await validateSessionFolder(session.id);
        if (!valid) {
            showToast('Session folder missing — files may be unavailable', { type: 'warning' });
        }
        sessionCanvases.set(await listCanvases(session.id));
    } catch {
        sessionCanvases.set([]);
    }
    activeSmartCollection.set(null);
    activeFolder.set(null);
    activeCollection.set(null);
    activeDetectedClass.set(null);
    activeSession.set(session);
    await loadImagesForCurrentScope();
}

function openCanvas(canvas: Canvas) {
    activeCanvas.set(canvas);
    navigateTo('canvas');
}

async function setFocusedRating(rating: number) {
    const idx = get(focusedIndex);
    const image = get(images)[idx];
    if (!image) return;
    await setRating(image.image.id, rating, get(activeSession)?.id ?? null);
    invalidateImageCache();
    images.update(all => {
        const next = [...all];
        next[idx] = withRating(next[idx], rating);
        return next;
    });
}

async function setFocusedDecision(decision: ImageDecision) {
    const idx = get(focusedIndex);
    const image = get(images)[idx];
    if (!image) return;
    await setDecision(image.image.id, decision, get(activeSession)?.id ?? null);
    invalidateImageCache();
    images.update(all => {
        const next = [...all];
        next[idx] = withDecision(next[idx], decision);
        return next;
    });
}

function focusedImageTitle(): string {
    const image = get(images)[get(focusedIndex)];
    return image?.path.split('/').pop() ?? 'focused image';
}

function selectedImageIds(inverse = false): string[] {
    const selected = get(selectedIds);
    return get(images)
        .filter(item => inverse ? !selected.has(item.image.id) : selected.has(item.image.id))
        .map(item => item.image.id);
}

async function createCollectionFromImageSet(inverse = false) {
    const imageIds = selectedImageIds(inverse);
    if (imageIds.length === 0) {
        statusHint.set(inverse ? 'No unselected images' : 'Select images first');
        setTimeout(() => statusHint.set(null), 2000);
        return;
    }

    const name = await requestTextInput({
        title: inverse ? 'Create Collection from Unselected' : 'Create Collection from Selection',
        label: 'Collection name',
        description: `${imageIds.length} images will be added.`,
        placeholder: 'Collection name',
        confirmLabel: 'Create',
    });
    if (!name?.trim()) return;

    const collectionId = await createCollection(name.trim());
    await addToCollection(collectionId, imageIds);
    collections.set(await listCollections());
    statusHint.set(`Created "${name.trim()}" with ${imageIds.length} images`);
    setTimeout(() => statusHint.set(null), 2000);
}

async function toggleCollectMode() {
    if (get(collectMode)) {
        collectMode.set(false);
        collectModeTarget.set(null);
        statusHint.set(null);
        return;
    }

    const availableCollections = get(collections);
    const target = await requestCollectionTarget({
        title: 'Collect Mode',
        description: availableCollections.length > 0
            ? 'Choose the collection that Space will add images to, or create a new one.'
            : 'Create a collection that Space will add images to.',
        collections: availableCollections,
        confirmLabel: 'Start',
    });
    if (!target) return;

    let targetId: string;
    if (target.type === 'existing') {
        targetId = target.collectionId;
    } else {
        targetId = await createCollection(target.name);
        collections.set(await listCollections());
    }

    collectMode.set(true);
    collectModeTarget.set(targetId);
    const collectionName = get(collections).find(item => item[0] === targetId)?.[1] ?? '';
    statusHint.set(`Collect mode: Space to add, B to exit [${collectionName}]`);
}

async function addFocusedImageToCollectTarget() {
    const target = get(collectModeTarget);
    const image = get(images)[get(focusedIndex)];
    if (!target || !image) return;

    await addToCollection(target, [image.image.id]);
    invalidateImageCache();
    collections.set(await listCollections());
    if (get(activeCollection) === target) {
        await loadImagesForCurrentScope({ resetFocus: false, force: true });
    }
    statusHint.set('Added to collection. Space for next, B to exit');
}

function toggleAgentPanel() {
    const open = get(agentPanelVisible) || get(agentPanelPinned);
    agentPanelVisible.set(!open);
    agentPanelPinned.set(!open);
}

export const WORKFLOW_CREATE_COMMAND_ID = 'workflow.create-from-recents';

async function executeWorkflow(workflow: CommandWorkflow) {
    const result = await runWorkflow(workflow, {
        resolveItem: id => getCommandPaletteItems('all').find(item => item.id === id),
        confirm: item =>
            typeof window !== 'undefined' && typeof window.confirm === 'function'
                ? window.confirm(`Run destructive step "${item.title}"?`)
                : true,
    });

    if (result.cancelled) {
        statusHint.set(`Workflow "${workflow.name}" cancelled`);
        setTimeout(() => statusHint.set(null), 2000);
        return;
    }
    if (!result.ok) {
        showToast(result.error ?? 'Workflow failed', { type: 'error', duration: 6000 });
        return;
    }
    showToast(`Workflow "${workflow.name}" complete`, { type: 'success', duration: 3000 });
    window.dispatchEvent(new CustomEvent('reload-images'));
}

async function createWorkflowFromRecents() {
    // Recent IDs are stored most-recent-first; a workflow should replay them in
    // the order they were used, so reverse to chronological and drop meta steps.
    const steps = readRecentCommandIds()
        .filter(id => id !== WORKFLOW_CREATE_COMMAND_ID && !id.startsWith('workflow.'))
        .reverse();
    if (steps.length === 0) {
        statusHint.set('Run some commands first to capture a workflow');
        setTimeout(() => statusHint.set(null), 2500);
        return;
    }

    const name = await requestTextInput({
        title: 'Save Workflow from Recent Commands',
        label: 'Workflow name',
        description: `${steps.length} recent command${steps.length === 1 ? '' : 's'} will be saved as a runnable sequence.`,
        placeholder: 'Workflow name',
        confirmLabel: 'Save',
    });
    if (!name?.trim()) return;

    createWorkflow(name.trim(), steps);
    statusHint.set(`Saved workflow "${name.trim()}"`);
    setTimeout(() => statusHint.set(null), 2500);
}

async function toggleFocusedClientFavorite() {
    const image = get(images)[get(focusedIndex)];
    if (!image) return;
    const existing = await getClientFeedback(image.image.id);
    const nextFavorite = !(existing?.favorite ?? false);
    await setClientFeedback(image.image.id, nextFavorite, existing?.comment ?? null);
    statusHint.set(nextFavorite ? 'Client favorite added' : 'Client favorite removed');
    setTimeout(() => statusHint.set(null), 2000);
}

async function addFocusedClientComment() {
    const image = get(images)[get(focusedIndex)];
    if (!image) return;
    const existing = await getClientFeedback(image.image.id);
    const comment = await requestTextInput({
        title: 'Client Comment',
        label: 'Comment',
        description: 'Stored separately from your curator rating and decision.',
        placeholder: 'Client feedback…',
        confirmLabel: 'Save',
    });
    if (comment === null) return;
    await setClientFeedback(image.image.id, existing?.favorite ?? false, comment.trim() || null);
    statusHint.set('Client comment saved');
    setTimeout(() => statusHint.set(null), 2000);
}

async function exportDeliveryCsv() {
    const items = get(images);
    if (items.length === 0) {
        statusHint.set('No images to export');
        setTimeout(() => statusHint.set(null), 2000);
        return;
    }
    const target = await saveDialog({
        title: 'Save delivery list',
        defaultPath: 'delivery-list.csv',
        filters: [{ name: 'CSV', extensions: ['csv'] }],
    });
    if (!target) return;

    const feedback = await listClientFeedback();
    const byId = new Map(feedback.map(f => [f.image_id, f]));
    const rows: DeliveryRow[] = items.map(item => {
        const fb = byId.get(item.image.id);
        return {
            filename: item.path.split('/').filter(Boolean).pop() ?? item.image.id,
            path: item.path,
            rating: item.selection?.star_rating ?? 0,
            decision: item.selection?.decision ?? 'undecided',
            clientFavorite: fb?.favorite ?? false,
            clientComment: fb?.comment ?? '',
        };
    });

    try {
        const written = await saveTextToPath(target, buildDeliveryCsv(rows));
        showToast(`Delivery list saved (${rows.length} rows)`, { detail: written, type: 'success', duration: 6000 });
    } catch (e) {
        showToast('Delivery export failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

function workflowItems(): CommandPaletteItem[] {
    return readWorkflows().map((workflow): CommandPaletteItem => ({
        id: workflow.id,
        title: workflow.name,
        subtitle: `Workflow · ${workflow.steps.length} step${workflow.steps.length === 1 ? '' : 's'}`,
        category: 'Workflow',
        kind: 'command',
        keywords: ['workflow', 'automation', 'sequence', 'run'],
        run: () => executeWorkflow(workflow),
    }));
}

function commandItems(): CommandPaletteItem[] {
    const hasImage = Boolean(get(images)[get(focusedIndex)]);
    const selectedCount = get(selectedIds).size;
    const unselectedCount = Math.max(0, get(images).length - selectedCount);
    const collectTarget = get(collectModeTarget);
    const collectTargetName = get(collections).find(item => item[0] === collectTarget)?.[1] ?? 'collection';

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
            id: WORKFLOW_CREATE_COMMAND_ID,
            title: 'Save Workflow from Recent Commands',
            subtitle: 'Capture your recent command sequence as a runnable workflow',
            category: 'Workflow',
            kind: 'command',
            keywords: ['workflow', 'automation', 'sequence', 'macro', 'save'],
            run: createWorkflowFromRecents,
        },
        {
            id: 'app.keyboard-shortcuts',
            title: 'View Keyboard Shortcuts',
            subtitle: 'Browse, customize, and reset command shortcuts',
            category: 'App',
            kind: 'command',
            keywords: ['hotkeys', 'keybindings', 'shortcuts', 'help', 'customize'],
            run: () => shortcutsOpen.set(true),
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
            id: 'agent.toggle-panel',
            title: 'Toggle Claude Agent Panel',
            subtitle: get(agentPanelVisible) || get(agentPanelPinned)
                ? 'Hide agent chat and proposals'
                : 'Show agent chat and proposals',
            category: 'Agent',
            kind: 'command',
            keywords: ['agent', 'claude', 'chat', 'proposal', 'panel', 'assistant'],
            run: toggleAgentPanel,
        },
        {
            id: 'agent.cycle-visual-level',
            title: 'Cycle Agent Visual Level',
            subtitle: `Current context: ${get(agentVisualLevel)}`,
            category: 'Agent',
            kind: 'command',
            keywords: ['agent', 'claude', 'context', 'visual', 'tokens', 'thumbnail', 'cost'],
            run: cycleAgentVisualLevel,
        },
        {
            id: 'agent.create-test-proposal',
            title: 'Create Agent Proposal from Selection',
            subtitle: selectedCount > 0 ? `${selectedCount} selected` : 'Select images first',
            category: 'Agent',
            kind: 'command',
            keywords: ['agent', 'claude', 'proposal', 'selection', 'curate', 'trash'],
            disabled: selectedCount === 0,
            run: () => {
                window.dispatchEvent(new CustomEvent('create-agent-test-proposal'));
            },
        },
        {
            id: 'agent.capture-view-snapshot',
            title: 'Capture Agent Snapshot',
            subtitle: 'Save the current view for agent analysis',
            category: 'Agent',
            kind: 'command',
            keywords: ['mcp', 'screen', 'screenshot', 'vision', 'multimodal', 'select'],
            defaultShortcut: 'Cmd+Shift+C',
            run: () => {
                window.dispatchEvent(new CustomEvent('capture-agent-view-snapshot', {
                    detail: { clipboard: false },
                }));
            },
        },
        {
            id: 'agent.capture-view-snapshot-to-clipboard',
            title: 'Capture Agent Snapshot to Clipboard',
            subtitle: 'Save the current view and copy the annotated image',
            category: 'Agent',
            kind: 'command',
            keywords: ['mcp', 'screen', 'screenshot', 'vision', 'clipboard', 'copy'],
            run: () => {
                window.dispatchEvent(new CustomEvent('capture-agent-view-snapshot', {
                    detail: { clipboard: true },
                }));
            },
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
        {
            id: 'view.actual-size',
            title: 'Actual Size',
            subtitle: 'Reset loupe zoom and pan',
            category: 'View',
            kind: 'command',
            keywords: ['zoom', 'fit', 'loupe', 'reset'],
            defaultShortcut: 'Cmd+0',
            run: () => resetLoupeTransform(),
        },
        ...buildViewCommands()
            .map(({ mode, title, subtitle, shortcut }): CommandPaletteItem => ({
                id: `view.${mode}`,
                title,
                subtitle: get(viewMode) === mode ? 'Current view' : subtitle,
                category: 'View',
                kind: 'command',
                keywords: [mode, 'tab', 'mode', ...(mode === 'publish' ? ['static', 'site', 'publishing'] : [])],
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
            id: 'collection.export-to-folder',
            title: 'Export to Folder…',
            subtitle: 'Export the current scope with format conversion and naming template',
            category: 'Collections',
            kind: 'command',
            keywords: ['export', 'save', 'folder', 'convert', 'deliver', 'output'],
            run: () => exportFolderOpen.set(true),
        },
        ...(get(clientToolsEnabled) ? [{
            id: 'collection.export-delivery-csv',
            title: 'Export Delivery List (CSV)…',
            subtitle: 'CSV of the current scope with curator + client feedback columns',
            category: 'Collections',
            kind: 'command' as const,
            keywords: ['csv', 'delivery', 'list', 'client', 'export', 'proof', 'final'],
            run: exportDeliveryCsv,
        }] : []),
        {
            id: 'client.toggle-favorite',
            title: 'Toggle Client Favorite',
            subtitle: hasImage ? focusedImageTitle() : 'Focus an image first',
            category: 'Client',
            kind: 'command',
            keywords: ['client', 'favorite', 'feedback', 'proof'],
            disabled: !hasImage,
            run: toggleFocusedClientFavorite,
        },
        {
            id: 'client.add-comment',
            title: 'Add Client Comment…',
            subtitle: hasImage ? focusedImageTitle() : 'Focus an image first',
            category: 'Client',
            kind: 'command',
            keywords: ['client', 'comment', 'feedback', 'note'],
            disabled: !hasImage,
            run: addFocusedClientComment,
        },
        {
            id: 'collection.export-contact-sheet',
            title: 'Export Contact Sheet…',
            subtitle: 'Render a configurable grid of the current images as a PNG',
            category: 'Collections',
            kind: 'command',
            keywords: ['contact sheet', 'montage', 'grid', 'proof', 'thumbnails', 'export'],
            run: () => contactSheetOpen.set(true),
        },
        {
            id: 'collection.create-from-selection',
            title: 'Create Collection from Selection',
            subtitle: selectedCount === 0 ? 'Select images first' : `${selectedCount} selected`,
            category: 'Collections',
            kind: 'command',
            keywords: ['collection', 'selected', 'save'],
            disabled: selectedCount === 0,
            run: () => createCollectionFromImageSet(false),
        },
        {
            id: 'collection.create-from-unselected',
            title: 'Create Collection from Unselected',
            subtitle: unselectedCount === 0 ? 'No unselected images' : `${unselectedCount} unselected`,
            category: 'Collections',
            kind: 'command',
            keywords: ['collection', 'inverse', 'unselected'],
            disabled: unselectedCount === 0,
            run: () => createCollectionFromImageSet(true),
        },
        {
            id: 'collection.toggle-collect-mode',
            title: get(collectMode) ? 'Exit Collect Mode' : 'Start Collect Mode',
            subtitle: get(collectMode) ? `Collecting into ${collectTargetName}` : 'Choose a collection target for Space',
            category: 'Collections',
            kind: 'command',
            keywords: ['collect', 'collection', 'space'],
            run: toggleCollectMode,
        },
        {
            id: 'collection.add-focused-to-collect-target',
            title: 'Add Focused Image to Collect Target',
            subtitle: collectTarget ? collectTargetName : 'Start collect mode first',
            category: 'Collections',
            kind: 'command',
            keywords: ['collect', 'append', 'collection'],
            disabled: !hasImage || !collectTarget,
            run: addFocusedImageToCollectTarget,
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
            id: 'curation.find-similar',
            title: 'Find Similar to Focused Image',
            subtitle: hasImage ? focusedImageTitle() : 'Focus an image first',
            category: 'AI',
            kind: 'command',
            keywords: ['similar', 'semantic', 'embedding', 'clip', 'look alike', 'duplicate'],
            disabled: !hasImage,
            run: async () => {
                const image = get(images)[get(focusedIndex)];
                if (!image) return;
                try {
                    const count = await loadSimilarImages(image.image.id, 30);
                    statusHint.set(count > 0 ? `Showing ${count} similar images` : 'No similar images found');
                } catch (e) {
                    showToast('Similarity search failed', { detail: String(e), type: 'error', duration: 6000 });
                }
                setTimeout(() => statusHint.set(null), 2500);
            },
        },
        {
            id: 'curation.best-of-group',
            title: 'Best of Group Ranking…',
            subtitle: 'Suggested winner per similarity group with score components',
            category: 'AI',
            kind: 'command',
            keywords: ['best', 'group', 'rank', 'winner', 'similar', 'duplicate', 'pick'],
            run: () => groupRankingOpen.set(true),
        },
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
    const activeSessionId = get(activeSession)?.id ?? null;
    const activeCanvasId = get(activeCanvas)?.id ?? null;
    const activeClass = get(activeDetectedClass);

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
        ...get(sessions).map((session): CommandPaletteItem => ({
            id: `scope.session.${session.id}`,
            title: session.name,
            subtitle: session.id === activeSessionId ? 'Current session' : `${session.image_count} images`,
            category: 'Session',
            kind: 'destination',
            keywords: ['session', session.id, session.description ?? ''],
            run: () => openSession(session),
        })),
        ...get(sessionCanvases).map((canvas): CommandPaletteItem => ({
            id: `scope.canvas.${canvas.id}`,
            title: canvas.name,
            subtitle: canvas.id === activeCanvasId ? 'Current canvas' : `Canvas · ${canvas.canvas_type}`,
            category: 'Canvas',
            kind: 'destination',
            keywords: ['canvas', canvas.id, canvas.canvas_type],
            run: () => openCanvas(canvas),
        })),
        ...get(detectedClasses).map(([className, count]): CommandPaletteItem => ({
            id: `scope.detected.${className}`,
            title: className,
            subtitle: className === activeClass ? 'Current detection filter' : `${count} images`,
            category: 'Detection',
            kind: 'destination',
            keywords: ['detected', 'object', 'class', className],
            run: () => openDetectedClass(className),
        })),
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
    // Plugin-contributed commands: the registry itself returns [] unless the
    // module_plugins flag is on, so no plugin surface leaks here ungated.
    const commands = [...workflowItems(), ...commandItems(), ...getPluginPaletteCommands()];
    const all = mode === 'commands' ? commands : [...commands, ...destinationItems()];
    return applyCommandAliases(all, readCommandAliases());
}

export function isCommandPaletteItemVisible(item: CommandPaletteItem): boolean {
    return item.when ? item.when() : true;
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
    const recent = (options.recentIds ?? []).slice(0, 5);
    const frequencies = options.frequencies ?? {};
    const hotkeys = options.hotkeys ?? {};
    const mode = options.mode ?? 'all';
    const hasQuery = normalize(query).length > 0;

    return items
        .filter(item => isCommandPaletteItemVisible(item))
        .filter(item => mode === 'all' || item.kind === 'command')
        .map(item => ({
            item,
            score: scoreCommandPaletteItem(query, item),
            pinnedRank: pinned.has(item.id) ? 1 : 0,
            recentRank: recent.includes(item.id) ? recent.length - recent.indexOf(item.id) : 0,
            frequentRank: frequencies[item.id] ?? 0,
            hasShortcut: shortcutForItem(item, hotkeys) ? 1 : 0,
        }))
        .filter(entry => entry.score > 0)
        .sort((a, b) =>
            (!hasQuery ? b.recentRank - a.recentRank : 0) ||
            b.pinnedRank - a.pinnedRank ||
            b.score - a.score ||
            (hasQuery ? b.recentRank - a.recentRank : 0) ||
            b.frequentRank - a.frequentRank ||
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

export function findDuplicateCommandHotkeys(
    items: CommandPaletteItem[],
    hotkeys: Record<string, string>,
): CommandHotkeyDuplicate[] {
    const byShortcut = new Map<string, CommandPaletteItem[]>();
    for (const item of items) {
        const shortcut = shortcutForItem(item, hotkeys);
        if (!shortcut) continue;
        const group = byShortcut.get(shortcut) ?? [];
        group.push(item);
        byShortcut.set(shortcut, group);
    }

    return [...byShortcut.entries()]
        .filter(([, group]) => group.length > 1)
        .map(([shortcut, group]) => ({
            shortcut,
            commandIds: group.map(item => item.id),
            titles: group.map(item => item.title),
        }));
}

export async function runCommandPaletteItem(item: CommandPaletteItem) {
    if (item.disabled || !isCommandPaletteItemVisible(item)) return;
    await item.run();
}

export function commandForKeyboardEvent(event: KeyboardEvent): CommandPaletteItem | null {
    const hotkeys = readCommandHotkeys();
    const items = getCommandPaletteItems('all');
    const item = items.find(candidate => {
        if (!isCommandPaletteItemVisible(candidate)) return false;
        const shortcut = hotkeys[candidate.id];
        return shortcut ? eventMatchesShortcut(event, shortcut) : false;
    });
    return item && !item.disabled ? item : null;
}

export async function runCommandForKeyboardEvent(event: KeyboardEvent): Promise<boolean> {
    const item = commandForKeyboardEvent(event);
    if (!item) return false;
    await runCommandPaletteItem(item);
    return true;
}

export function openCommandPalette(mode: CommandPaletteMode = 'all') {
    commandPaletteMode.set(mode);
    commandPaletteOpen.set(true);
}
