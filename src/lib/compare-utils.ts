import type { ImageWithFile } from './api';

export type ComparePair = readonly [ImageWithFile | null, ImageWithFile | null];

export function resolveComparePair(
    images: ImageWithFile[],
    selectedIds: Set<string>,
    focusedIndex: number
): ComparePair {
    if (selectedIds.size >= 2) {
        const selArr = Array.from(selectedIds);
        const a = images.find(i => i.image.id === selArr[0]);
        const b = images.find(i => i.image.id === selArr[1]);
        if (a && b) return [a, b] as const;
    }

    const a = images[focusedIndex];
    const b = images[focusedIndex + 1];
    if (a && b) return [a, b] as const;
    if (a) return [a, null] as const;
    return [null, null] as const;
}

export function ratingStars(img: ImageWithFile | null): number {
    return img?.selection?.star_rating ?? 0;
}

export function decisionLabel(img: ImageWithFile | null): string {
    return img?.selection?.decision ?? 'undecided';
}

export interface SwapResult {
    newSelectedIds: Set<string>;
}

export interface ComparePresentationFlags {
    zen: boolean;
    imageOnly: boolean;
}

export function nextComparePresentationState(current: ComparePresentationFlags): ComparePresentationFlags {
    if (!current.zen) {
        return { zen: true, imageOnly: false };
    }
    if (!current.imageOnly) {
        return { zen: true, imageOnly: true };
    }
    return { zen: false, imageOnly: false };
}

export function computeCompareSwap(
    imageIds: string[],
    selectedIds: Set<string>,
    focusedIndex: number,
    activeSide: 0 | 1,
    direction: 1 | -1
): SwapResult | null {
    if (imageIds.length < 2) return null;

    if (selectedIds.size >= 2) {
        const selArr = Array.from(selectedIds);
        const targetId = selArr[activeSide];
        const currentIdx = imageIds.indexOf(targetId);
        if (currentIdx < 0) return null;
        const newIdx = Math.max(0, Math.min(imageIds.length - 1, currentIdx + direction));
        const newId = imageIds[newIdx];
        if (!newId || newId === selArr[1 - activeSide]) return null;
        selArr[activeSide] = newId;
        return { newSelectedIds: new Set(selArr) };
    }

    const leftId = imageIds[focusedIndex];
    const rightId = imageIds[focusedIndex + 1];
    if (!leftId || !rightId) return null;

    const selArr = [leftId, rightId];
    const currentIdx = activeSide === 0 ? focusedIndex : focusedIndex + 1;
    const newIdx = Math.max(0, Math.min(imageIds.length - 1, currentIdx + direction));
    const newId = imageIds[newIdx];
    if (!newId || newId === selArr[1 - activeSide]) return null;
    selArr[activeSide] = newId;
    return { newSelectedIds: new Set(selArr) };
}
