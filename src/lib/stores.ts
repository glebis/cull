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

export const focusedImage = derived(
    [images, focusedIndex],
    ([$images, $idx]) => $images[$idx] ?? null
);
