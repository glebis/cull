import { writable, derived } from 'svelte/store';
import type { ImageWithFile } from './api';

export type ViewMode = 'grid' | 'compare' | 'loupe' | 'canvas' | 'lineage' | 'embeddings' | 'export';

export const images = writable<ImageWithFile[]>([]);
export const selectedIds = writable<Set<string>>(new Set());
export const focusedIndex = writable<number>(0);
export const totalCount = writable<number>(0);
export const viewMode = writable<ViewMode>('grid');
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

export const focusedImage = derived(
    [images, focusedIndex],
    ([$images, $idx]) => $images[$idx] ?? null
);
