import { get } from 'svelte/store';
import {
    compareActiveSide,
    focusedIndex,
    images,
    selectedIds,
    viewMode,
} from './stores';

export function currentImageIndex(): number {
    const index = get(focusedIndex);
    if (get(viewMode) !== 'compare') return index;

    const allImages = get(images);
    const selected = get(selectedIds);
    const side = get(compareActiveSide);
    if (selected.size >= 2) {
        const selectedImageIds = Array.from(selected);
        const targetId = selectedImageIds[side] ?? selectedImageIds[0];
        const selectedIndex = allImages.findIndex(item => item.image.id === targetId);
        return selectedIndex >= 0 ? selectedIndex : index;
    }
    return index + side;
}
