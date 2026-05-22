import { get } from 'svelte/store';
import { afterEach, describe, expect, it } from 'vitest';
import type { ImageWithFile } from './api';
import { focusedImage, focusedImageOverride, focusedIndex, images } from './stores';

function makeImage(id: string): ImageWithFile {
    return {
        image: {
            id,
            sha256_hash: '',
            width: 100,
            height: 100,
            format: 'jpeg',
            file_size: 1000,
            created_at: '',
            imported_at: '',
            ai_prompt: null,
            raw_metadata: null,
        },
        source_label: null,
        path: `/photos/${id}.jpg`,
        thumbnail_path: null,
        selection: null,
        missing_at: null,
    };
}

describe('focusedImage', () => {
    afterEach(() => {
        focusedImageOverride.set(null);
        focusedIndex.set(0);
        images.set([]);
    });

    it('uses the focused index after an override when focus is set directly', () => {
        images.set([makeImage('grid-1'), makeImage('grid-2')]);
        focusedImageOverride.set(makeImage('old-override'));

        focusedIndex.set(1);

        expect(get(focusedImage)?.image.id).toBe('grid-2');
    });

    it('uses the focused index after an override when focus is updated', () => {
        images.set([makeImage('grid-1'), makeImage('grid-2')]);
        focusedImageOverride.set(makeImage('old-override'));

        focusedIndex.update((index) => index + 1);

        expect(get(focusedImage)?.image.id).toBe('grid-2');
    });
});
