import type { ImageWithFile } from './api';

export interface LineageImageFocus {
    focusedIndex: number | null;
    focusedImageOverride: ImageWithFile | null;
}

export function resolveLineageImageFocus(
    images: ImageWithFile[],
    image: ImageWithFile,
): LineageImageFocus {
    const focusedIndex = images.findIndex((candidate) => candidate.image.id === image.image.id);
    if (focusedIndex >= 0) {
        return {
            focusedIndex,
            focusedImageOverride: null,
        };
    }

    return {
        focusedIndex: null,
        focusedImageOverride: image,
    };
}
