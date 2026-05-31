import { get } from 'svelte/store';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ImageWithFile } from './api';
import { getImageByPath } from './api';
import { focusedImage, focusedImageOverride, focusedIndex, images } from './stores';
import { focusImagePath } from './transform-results';

vi.mock('./api', async (importOriginal) => {
    const actual = await importOriginal<typeof import('./api')>();
    return {
        ...actual,
        getImageByPath: vi.fn(),
    };
});

function makeImage(id: string, path = `/photos/${id}.png`): ImageWithFile {
    return {
        image: {
            id,
            sha256_hash: '',
            width: 100,
            height: 100,
            format: 'png',
            file_size: 1000,
            created_at: '',
            imported_at: '',
            ai_prompt: null,
            raw_metadata: null,
        },
        source_label: null,
        path,
        thumbnail_path: null,
        selection: null,
        missing_at: null,
    };
}

describe('focusImagePath', () => {
    afterEach(() => {
        vi.clearAllMocks();
        focusedImageOverride.set(null);
        focusedIndex.set(0);
        images.set([]);
    });

    it('appends and focuses a transformed image that is not in the current list', async () => {
        const source = makeImage('source');
        const derivative = makeImage('derivative', '/photos/source_crop.png');
        images.set([source]);
        vi.mocked(getImageByPath).mockResolvedValue(derivative);

        const focused = await focusImagePath('/photos/source_crop.png');

        expect(focused).toBe(true);
        expect(get(images).map(item => item.image.id)).toEqual(['source', 'derivative']);
        expect(get(focusedIndex)).toBe(1);
        expect(get(focusedImageOverride)).toBeNull();
        expect(get(focusedImage)?.image.id).toBe('derivative');
    });

    it('updates and focuses an existing transformed image', async () => {
        const source = makeImage('source');
        const oldDerivative = makeImage('derivative', '/photos/source_crop.png');
        const updatedDerivative = {
            ...oldDerivative,
            image: { ...oldDerivative.image, width: 42 },
        };
        images.set([source, oldDerivative]);
        vi.mocked(getImageByPath).mockResolvedValue(updatedDerivative);

        const focused = await focusImagePath('/photos/source_crop.png');

        expect(focused).toBe(true);
        expect(get(images)[1].image.width).toBe(42);
        expect(get(focusedIndex)).toBe(1);
        expect(get(focusedImageOverride)).toBeNull();
    });

    it('returns false when the backend cannot resolve the derivative path', async () => {
        images.set([makeImage('source')]);
        vi.mocked(getImageByPath).mockResolvedValue(null);

        const focused = await focusImagePath('/photos/missing_crop.png');

        expect(focused).toBe(false);
        expect(get(images).map(item => item.image.id)).toEqual(['source']);
        expect(get(focusedIndex)).toBe(0);
    });
});
