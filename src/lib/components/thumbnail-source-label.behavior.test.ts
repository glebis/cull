// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/svelte';

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: (path: string) => `asset://${path}` }));
vi.mock('$lib/api', () => ({
    getMediaAssetForImage: vi.fn().mockResolvedValue(null),
    regenerateSingleThumbnail: vi.fn(),
}));
vi.mock('$lib/diagnostics', () => ({ recordImageLoadFailure: vi.fn() }));

import Thumbnail from './Thumbnail.svelte';

afterEach(() => cleanup());

describe('Thumbnail source labels', () => {
    it('presents the internal camera source label as RAW', () => {
        render(Thumbnail, {
            item: {
                image: {
                    id: 'raw-1', sha256_hash: 'hash', width: 6000, height: 4000, format: 'raf',
                    file_size: 12_000_000, created_at: '2026-08-31T00:00:00Z',
                    imported_at: '2026-08-31T00:00:00Z', ai_prompt: null, raw_metadata: null,
                },
                path: '/Volumes/CARD/DCIM/DSCF0001.RAF',
                thumbnail_path: '/tmp/raw-1.jpg', selection: null, source_label: 'camera', missing_at: null,
            },
            size: 160, focused: true, selected: false, onclick: vi.fn(), ondblclick: vi.fn(),
        });

        expect(screen.getByText('RAW')).toBeVisible();
        expect(screen.queryByText('camera')).not.toBeInTheDocument();
        expect(screen.getByRole('gridcell')).toHaveAccessibleName(/source RAW/);
    });
});
