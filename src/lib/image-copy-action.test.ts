import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ImageWithFile } from './api';
import { compareActiveSide, focusedIndex, images, selectedIds, viewMode } from './stores';

const mocks = vi.hoisted(() => ({
    copyImageToClipboard: vi.fn(),
}));

vi.mock('./api', async importOriginal => ({
    ...(await importOriginal<typeof import('./api')>()),
    copyImageToClipboard: mocks.copyImageToClipboard,
}));

import { copyCurrentImageToClipboard } from './image-copy-action';

function image(id: string): ImageWithFile {
    return {
        image: {
            id,
            sha256_hash: `hash-${id}`,
            width: 100,
            height: 100,
            format: 'png',
            file_size: 100,
            created_at: '2026-08-08T00:00:00Z',
            imported_at: '2026-08-08T00:00:00Z',
            ai_prompt: null,
            raw_metadata: null,
        },
        path: `/images/${id}.png`,
        thumbnail_path: null,
        selection: null,
        source_label: null,
        missing_at: null,
    };
}

describe('copyCurrentImageToClipboard', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        images.set([image('one'), image('two')]);
        selectedIds.set(new Set());
        focusedIndex.set(0);
        compareActiveSide.set(0);
        viewMode.set('grid');
    });

    it('copies the focused image in the library', async () => {
        await copyCurrentImageToClipboard();
        expect(mocks.copyImageToClipboard).toHaveBeenCalledWith('one');
    });

    it('copies the active comparison side', async () => {
        viewMode.set('compare');
        selectedIds.set(new Set(['one', 'two']));
        compareActiveSide.set(1);

        await copyCurrentImageToClipboard();
        expect(mocks.copyImageToClipboard).toHaveBeenCalledWith('two');
    });
});
