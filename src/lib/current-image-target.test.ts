import { beforeEach, describe, expect, it } from 'vitest';
import type { ImageWithFile } from './api';
import { compareActiveSide, focusedIndex, images, selectedIds, viewMode } from './stores';
import { currentImageIndex } from './current-image-target';

function image(id: string): ImageWithFile {
    return {
        image: {
            id, sha256_hash: `hash-${id}`, width: 100, height: 100, format: 'png',
            file_size: 100, created_at: '2026-08-08T00:00:00Z',
            imported_at: '2026-08-08T00:00:00Z', ai_prompt: null, raw_metadata: null,
        },
        path: `/images/${id}.png`, thumbnail_path: null, selection: null,
        source_label: null, missing_at: null,
    };
}

describe('currentImageIndex', () => {
    beforeEach(() => {
        images.set([image('one'), image('two'), image('three')]);
        selectedIds.set(new Set());
        focusedIndex.set(0);
        compareActiveSide.set(0);
        viewMode.set('grid');
    });

    it('uses the active selected side in Compare', () => {
        viewMode.set('compare');
        selectedIds.set(new Set(['one', 'three']));
        compareActiveSide.set(1);
        expect(currentImageIndex()).toBe(2);
    });
});
