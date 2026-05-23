import { get } from 'svelte/store';
import { afterEach, describe, expect, it } from 'vitest';
import type { ImageWithFile } from './api';
import { focusedImage, focusedImageOverride, focusedIndex, images, selectedIds } from './stores';

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
        selectedIds.reset(new Set());
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

describe('selectedIds history', () => {
    afterEach(() => {
        selectedIds.reset(new Set());
    });

    it('undo restores the previous selection state', () => {
        selectedIds.set(new Set(['a']));
        selectedIds.set(new Set(['a', 'b']));

        expect(selectedIds.undo()).toBe(true);
        expect(get(selectedIds)).toEqual(new Set(['a']));
    });

    it('redo reapplies an undone selection state', () => {
        selectedIds.set(new Set(['a']));
        selectedIds.set(new Set(['a', 'b']));
        selectedIds.undo();

        expect(selectedIds.redo()).toBe(true);
        expect(get(selectedIds)).toEqual(new Set(['a', 'b']));
    });

    it('does not create a history entry for equivalent selections', () => {
        selectedIds.set(new Set(['a']));
        selectedIds.set(new Set(['a']));

        expect(selectedIds.undo()).toBe(true);
        expect(get(selectedIds)).toEqual(new Set());
        expect(selectedIds.undo()).toBe(false);
    });
});
