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
