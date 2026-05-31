import { getImageByPath } from './api';
import { focusedImageOverride, focusedIndex, images } from './stores';

export async function focusImagePath(path: string): Promise<boolean> {
    const image = await getImageByPath(path);
    if (!image) return false;

    let targetIndex = -1;
    images.update(current => {
        targetIndex = current.findIndex(item => item.image.id === image.image.id);
        if (targetIndex < 0) {
            targetIndex = current.length;
            return [...current, image];
        }
        const next = [...current];
        next[targetIndex] = image;
        return next;
    });

    focusedImageOverride.set(null);
    focusedIndex.set(targetIndex);

    return true;
}
