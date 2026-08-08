import { get } from 'svelte/store';
import { copyImageToClipboard } from './api';
import { filenameForPath } from './clipboard-actions';
import { images, showToast } from './stores';
import { currentImageIndex } from './current-image-target';
import type { ImageWithFile } from './api';

export async function copyImageWithToast(image: ImageWithFile | null | undefined) {
    if (!image) {
        showToast('No current image to copy', { type: 'warning', duration: 3000 });
        return;
    }

    try {
        await copyImageToClipboard(image.image.id);
        showToast('Copied image', {
            detail: filenameForPath(image.path),
            type: 'success',
            duration: 2500,
        });
    } catch (error) {
        console.error('Failed to copy image:', error);
        showToast('Could not copy image', {
            detail: String(error),
            type: 'error',
            duration: 5000,
        });
    }
}

export function copyCurrentImageToClipboard() {
    return copyImageWithToast(get(images)[currentImageIndex()]);
}
