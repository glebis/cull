import { getImageByPath } from './api';
import { focusedImageOverride, focusedIndex, images } from './stores';

export async function focusImagePath(path: string): Promise<boolean> {
    const image = await getImageByPath(path);
    if (!image) return false;

    let targetIndex = -1;
    images.update(current => {
        targetIndex = current.findIndex(item => item.image.id === image.image.id);
        if (targetIndex < 0) return current;
        const next = [...current];
        next[targetIndex] = image;
        return next;
    });

    if (targetIndex >= 0) {
        focusedImageOverride.set(null);
        focusedIndex.set(targetIndex);
    } else {
        focusedImageOverride.set(image);
    }

    return true;
}
