import { describe, expect, it } from 'vitest';
import type { ImageWithFile } from './api';
import { resolveLineageImageFocus } from './lineage-utils';

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

describe('resolveLineageImageFocus', () => {
    it('focuses the clicked lineage image by current image index when it is loaded', () => {
        const clicked = makeImage('b');
        const result = resolveLineageImageFocus([makeImage('a'), clicked, makeImage('c')], clicked);

        expect(result).toEqual({
            focusedIndex: 1,
            focusedImageOverride: null,
        });
    });

    it('uses the clicked lineage image as the focus override when it is not loaded', () => {
        const clicked = makeImage('lineage-only');
        const result = resolveLineageImageFocus([makeImage('a'), makeImage('b')], clicked);

        expect(result).toEqual({
            focusedIndex: null,
            focusedImageOverride: clicked,
        });
    });
});
