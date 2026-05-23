import { writable, derived, get, type Writable } from 'svelte/store';
import type { ImageWithFile, SmartCollection, Session, Canvas } from './api';

export type ViewMode = 'grid' | 'compare' | 'loupe' | 'canvas' | 'lineage' | 'embeddings' | 'export' | 'tinder';

export const images = writable<ImageWithFile[]>([]);

export interface SelectionStore extends Writable<Set<string>> {
    undo(): boolean;
    redo(): boolean;
    reset(ids?: Set<string>): void;
    clearHistory(): void;
}

function cloneSelection(ids: Set<string>): Set<string> {
    return new Set(ids);
}

function selectionsEqual(a: Set<string>, b: Set<string>): boolean {
    if (a.size !== b.size) return false;
    for (const id of a) {
        if (!b.has(id)) return false;
    }
    return true;
}

function createSelectionStore(historyLimit = 100): SelectionStore {
    const store = writable<Set<string>>(new Set());
    let current = new Set<string>();
    let undoStack: Set<string>[] = [];
    let redoStack: Set<string>[] = [];

    function publish(nextIds: Set<string>, recordHistory: boolean) {
        const next = cloneSelection(nextIds);
        if (selectionsEqual(current, next)) return;

        if (recordHistory) {
            undoStack.push(cloneSelection(current));
            if (undoStack.length > historyLimit) undoStack = undoStack.slice(-historyLimit);
            redoStack = [];
        }

        current = next;
        store.set(current);
    }

    return {
        subscribe: store.subscribe,
        set(ids: Set<string>) {
            publish(ids, true);
        },
        update(updater: (ids: Set<string>) => Set<string>) {
            publish(updater(cloneSelection(current)), true);
        },
        undo() {
            const previous = undoStack.pop();
            if (!previous) return false;
            redoStack.push(cloneSelection(current));
            current = cloneSelection(previous);
            store.set(current);
            return true;
        },
        redo() {
            const next = redoStack.pop();
            if (!next) return false;
            undoStack.push(cloneSelection(current));
            current = cloneSelection(next);
            store.set(current);
            return true;
        },
        reset(ids: Set<string> = new Set()) {
            undoStack = [];
            redoStack = [];
            current = cloneSelection(ids);
            store.set(current);
        },
        clearHistory() {
            undoStack = [];
            redoStack = [];
        },
    };
}

export const selectedIds = createSelectionStore();
export const selectionAnchorIndex = writable<number | null>(null);
export const focusedImageOverride = writable<ImageWithFile | null>(null);

function createFocusedIndexStore() {
    const store = writable<number>(0);

    return {
        subscribe: store.subscribe,
        set(index: number) {
            focusedImageOverride.set(null);
            store.set(index);
        },
        update(updater: (index: number) => number) {
            focusedImageOverride.set(null);
            store.update(updater);
        },
    };
}

export const focusedIndex = createFocusedIndexStore();
export const totalCount = writable<number>(0);
export const viewMode = writable<ViewMode>('grid');
export const gridScrollTop = writable<number>(0);

export interface ImageLoadState {
    loading: boolean;
    loadingMore: boolean;
    hasMore: boolean;
}

export const imageLoadState = writable<ImageLoadState>({
    loading: false,
    loadingMore: false,
    hasMore: false,
});

// Navigation history stack
export const viewHistory = writable<{ mode: ViewMode; focusedIndex: number }[]>([]);

export function navigateTo(mode: ViewMode) {
    const currentMode = get(viewMode);
    if (currentMode === mode) return;
    viewHistory.update(h => [...h, { mode: currentMode, focusedIndex: get(focusedIndex) }]);
    viewMode.set(mode);
}

export function navigateBack(): boolean {
    const history = get(viewHistory);
    if (history.length === 0) return false;
    const prev = history[history.length - 1];
    viewHistory.update(h => h.slice(0, -1));
    viewMode.set(prev.mode);
    focusedIndex.set(prev.focusedIndex);
    return true;
}

// Window identity
export const windowName = writable<string>('Cull');
export const windowLabel = writable<string>('main');

export const thumbnailSize = writable<number>(160);

export const selectedCount = derived(selectedIds, ($ids) => $ids.size);
export const statusHint = writable<string | null>(null);

export const sidebarVisible = writable<boolean>(true);

export const GRID_PRESETS = [
    { name: 'compact', size: 80, gap: 2 },
    { name: 'normal', size: 160, gap: 4 },
    { name: 'large', size: 280, gap: 8 },
    { name: 'xl', size: 400, gap: 12 },
];

export const gridPreset = writable<number>(1);
export const gridGap = writable<number>(4);

export const zenMode = writable<boolean>(false);

export const compareImages = writable<ImageWithFile[]>([]);
export const compareIndex = writable<number>(0);
export const compareActiveSide = writable<0 | 1>(0);
export const loupeScale = writable<number>(1);
export const loupePanX = writable<number>(0);
export const loupePanY = writable<number>(0);

export const folders = writable<[string, number][]>([]);
export const activeFolder = writable<string | null>(null);
export const minSizeFilter = writable<number>(0);
export const showMissing = writable<boolean>(false);
export const activeDetectedClass = writable<string | null>(null);

// Custom dialogs
export interface TextInputDialogOptions {
    title: string;
    label?: string;
    description?: string;
    initialValue?: string;
    placeholder?: string;
    confirmLabel?: string;
    cancelLabel?: string;
}

export interface TextInputDialogRequest extends TextInputDialogOptions {
    id: number;
    resolve: (value: string | null) => void;
}

export type CollectionTargetDialogResult =
    | { type: 'existing'; collectionId: string }
    | { type: 'new'; name: string };

export interface CollectionTargetDialogOptions {
    title: string;
    description?: string;
    collections: [string, string, number][];
    initialName?: string;
    confirmLabel?: string;
    cancelLabel?: string;
}

export interface CollectionTargetDialogRequest extends CollectionTargetDialogOptions {
    id: number;
    resolve: (value: CollectionTargetDialogResult | null) => void;
}

export const textInputDialog = writable<TextInputDialogRequest | null>(null);
export const collectionTargetDialog = writable<CollectionTargetDialogRequest | null>(null);

let textInputDialogId = 0;
let collectionTargetDialogId = 0;

function cancelActiveCollectionTargetDialog() {
    const active = get(collectionTargetDialog);
    if (!active) return;
    collectionTargetDialog.set(null);
    active.resolve(null);
}

function cancelActiveTextInputDialog() {
    const active = get(textInputDialog);
    if (!active) return;
    textInputDialog.set(null);
    active.resolve(null);
}

export function requestTextInput(options: TextInputDialogOptions): Promise<string | null> {
    cancelActiveCollectionTargetDialog();
    cancelActiveTextInputDialog();
    return new Promise(resolve => {
        textInputDialog.set({
            id: ++textInputDialogId,
            ...options,
            resolve,
        });
    });
}

export function resolveTextInputDialog(value: string | null) {
    const active = get(textInputDialog);
    if (!active) return;
    textInputDialog.set(null);
    active.resolve(value);
}

export function requestCollectionTarget(options: CollectionTargetDialogOptions): Promise<CollectionTargetDialogResult | null> {
    cancelActiveTextInputDialog();
    cancelActiveCollectionTargetDialog();
    return new Promise(resolve => {
        collectionTargetDialog.set({
            id: ++collectionTargetDialogId,
            ...options,
            collections: options.collections.map(([id, name, count]) => [id, name, count]),
            resolve,
        });
    });
}

export function resolveCollectionTargetDialog(value: CollectionTargetDialogResult | null) {
    const active = get(collectionTargetDialog);
    if (!active) return;
    collectionTargetDialog.set(null);
    active.resolve(value);
}

// Collections
export const collections = writable<[string, string, number][]>([]); // [id, name, count]
export const activeCollection = writable<string | null>(null);
export const collectMode = writable<boolean>(false);
export const collectModeTarget = writable<string | null>(null); // collection id being collected into

// Smart Collections
export const smartCollections = writable<SmartCollection[]>([]);
export const activeSmartCollection = writable<SmartCollection | null>(null);

// Import batch filter (transient — shows only batch images after import)
export const importBatchFilter = writable<string | null>(null);
export const importBatchImageIds = writable<string[]>([]);

// Pinned (active) collection — new imports auto-append here
export const pinnedCollection = writable<string | null>(null);

// Lineage tab layout preference
export type LineageLayout = 'timeline' | 'comparison';
export const lineageLayout = writable<LineageLayout>('timeline');

// Detection overlay state
export type NsfwMode = 'blur' | 'hide' | 'show';
export const showDetectionBoxes = writable<boolean>(false);
export const showDetectionInspector = writable<boolean>(false);
export const nsfwMode = writable<NsfwMode>('blur');

// Toast notifications
export interface ToastAction {
    label: string;
    onclick: () => void;
}

export interface Toast {
    id: number;
    message: string;
    detail?: string;
    type: 'info' | 'success' | 'warning' | 'error';
    duration: number;
    actions?: ToastAction[];
}
export const toasts = writable<Toast[]>([]);
let toastId = 0;
export function showToast(message: string, opts?: { detail?: string; type?: Toast['type']; duration?: number; actions?: ToastAction[] }) {
    const id = ++toastId;
    const toast: Toast = {
        id,
        message,
        detail: opts?.detail,
        type: opts?.type ?? 'info',
        duration: opts?.duration ?? 7000,
        actions: opts?.actions,
    };
    toasts.update(t => [...t, toast]);
    setTimeout(() => {
        toasts.update(t => t.filter(x => x.id !== id));
    }, toast.duration);
}

// Embedding view state (persists across tab switches)
export type EmbeddingInteractionMode = 'map' | 'stack' | 'review' | 'text';
export type EmbeddingZPreset = 'projection' | 'cluster' | 'source' | 'rating' | 'decision' | 'recency' | 'resolution';
export type EmbeddingSpacePreset = 'balanced' | 'compact' | 'gallery' | 'deep' | 'custom';
export type EmbeddingProvider = 'clip' | 'dinov2' | 'gemini' | 'cohere' | 'openai' | 'ollama';

export interface EmbeddingViewState {
    panX: number;
    panY: number;
    scale: number;
    selectedPointId: string | null;
    highlightedCluster: number | null;
    provider: EmbeddingProvider;
    projectionKey: string | null;
    hasUserView: boolean;
    interactionMode: EmbeddingInteractionMode;
    zPreset: EmbeddingZPreset;
    activeZLayerKey: string | null;
    focusActiveLayer: boolean;
    largePreviewOpen: boolean;
    textOutputOpen: boolean;
    canvasLabelsOpen: boolean;
    spacePreset: EmbeddingSpacePreset;
    spaceSpacing: number;
    spaceDepth: number;
    spaceScale: number;
    spacePerspective: number;
}

export const embeddingViewState = writable<EmbeddingViewState>({
    panX: 0,
    panY: 0,
    scale: 1,
    selectedPointId: null,
    highlightedCluster: null,
    provider: 'clip',
    projectionKey: null,
    hasUserView: false,
    interactionMode: 'map',
    zPreset: 'cluster',
    activeZLayerKey: null,
    focusActiveLayer: false,
    largePreviewOpen: true,
    textOutputOpen: false,
    canvasLabelsOpen: false,
    spacePreset: 'balanced',
    spaceSpacing: 1,
    spaceDepth: 0.35,
    spaceScale: 1,
    spacePerspective: 0.3,
});

// Sessions
export const sessions = writable<Session[]>([]);
export const activeSession = writable<Session | null>(null);
export const sessionCanvases = writable<Canvas[]>([]);
export const activeCanvas = writable<Canvas | null>(null);

// Settings panel
export const settingsOpen = writable<boolean>(false);
export const aboutOpen = writable<boolean>(false);
export const searchOpen = writable<boolean>(false);
export type CommandPaletteMode = 'all' | 'commands';
export const commandPaletteOpen = writable<boolean>(false);
export const commandPaletteMode = writable<CommandPaletteMode>('all');

export const focusedImage = derived(
    [images, focusedIndex, focusedImageOverride],
    ([$images, $idx, $override]) => $override ?? $images[$idx] ?? null
);
