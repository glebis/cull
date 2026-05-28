import { describe, expect, it } from 'vitest';
import type { ImageWithFile } from './api';
import { withDecision, withRating } from './selection-updates';

function image(selection: ImageWithFile['selection'] = null): ImageWithFile {
    return {
        image: {
            id: 'img-1',
            sha256_hash: 'hash-1',
            width: 100,
            height: 100,
            format: 'png',
            file_size: 1024,
            created_at: '2026-05-28T00:00:00Z',
            imported_at: '2026-05-28T00:00:00Z',
            ai_prompt: null,
            raw_metadata: null,
        },
        path: '/tmp/img-1.png',
        thumbnail_path: null,
        selection,
        source_label: null,
        missing_at: null,
    };
}

describe('selection update helpers', () => {
    it('creates selection state when rating an unrated image', () => {
        const result = withRating(image(null), 5);

        expect(result.selection).toEqual({
            image_id: 'img-1',
            project_id: null,
            star_rating: 5,
            color_label: null,
            decision: 'undecided',
        });
    });

    it('creates selection state when deciding an undecided image', () => {
        const result = withDecision(image(null), 'accept');

        expect(result.selection).toEqual({
            image_id: 'img-1',
            project_id: null,
            star_rating: null,
            color_label: null,
            decision: 'accept',
        });
    });

    it('preserves existing selection fields while updating one action', () => {
        const existing = image({
            image_id: 'img-1',
            project_id: null,
            star_rating: 3,
            color_label: 'blue',
            decision: 'reject',
        });

        expect(withRating(existing, 4).selection).toMatchObject({
            star_rating: 4,
            color_label: 'blue',
            decision: 'reject',
        });
        expect(withDecision(existing, 'undecided').selection).toMatchObject({
            star_rating: 3,
            color_label: 'blue',
            decision: 'undecided',
        });
    });
});
