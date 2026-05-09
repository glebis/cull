import { writable, derived, get } from 'svelte/store';
import type { ImageWithFile, SmartCollection } from './api';

export type ViewMode = 'grid' | 'compare' | 'loupe' | 'canvas' | 'lineage' | 'embeddings' | 'export';

export const images = writable<ImageWithFile[]>([]);
export const selectedIds = writable<Set<string>>(new Set());
export const focusedIndex = writable<number>(0);
export const totalCount = writable<number>(0);
export const viewMode = writable<ViewMode>('grid');

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
export const windowName = writable<string>('ImageView');
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

// Collections
export const collections = writable<[string, string, number][]>([]); // [id, name, count]
export const activeCollection = writable<string | null>(null);
export const collectMode = writable<boolean>(false);
export const collectModeTarget = writable<string | null>(null); // collection id being collected into

// Smart Collections
export const smartCollections = writable<SmartCollection[]>([]);
export const activeSmartCollection = writable<SmartCollection | null>(null);

// Detection overlay state
export type NsfwMode = 'blur' | 'hide' | 'show';
export const showDetectionBoxes = writable<boolean>(false);
export const showDetectionInspector = writable<boolean>(false);
export const nsfwMode = writable<NsfwMode>('blur');

// Toast notifications
export interface Toast {
    id: number;
    message: string;
    detail?: string;
    type: 'info' | 'success' | 'warning' | 'error';
    duration: number;
}
export const toasts = writable<Toast[]>([]);
let toastId = 0;
export function showToast(message: string, opts?: { detail?: string; type?: Toast['type']; duration?: number }) {
    const id = ++toastId;
    const toast: Toast = {
        id,
        message,
        detail: opts?.detail,
        type: opts?.type ?? 'info',
        duration: opts?.duration ?? 7000,
    };
    toasts.update(t => [...t, toast]);
    setTimeout(() => {
        toasts.update(t => t.filter(x => x.id !== id));
    }, toast.duration);
}

export const focusedImage = derived(
    [images, focusedIndex],
    ([$images, $idx]) => $images[$idx] ?? null
);
