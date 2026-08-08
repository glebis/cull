import { describe, expect, it } from 'vitest';
import type { ImageWithFile } from './api';
import { applyVisibleDecision } from './rejected-visibility';
function image(id: string): ImageWithFile { return { image: { id, sha256_hash: `hash-${id}`, width: 100, height: 100, format: 'png', file_size: 100, created_at: '2026-01-01', imported_at: '2026-01-01', ai_prompt: null, raw_metadata: null }, path: `/images/${id}.png`, thumbnail_path: null, selection: null, source_label: null, missing_at: null }; }
describe('visible decision updates', () => {
    it('removes a newly rejected row and keeps focus on the next row when rejects are hidden', () => {
        const result = applyVisibleDecision([image('before'), image('focused'), image('after')], 'focused', 'reject', false, 1);
        expect(result.items.map(item => item.image.id)).toEqual(['before', 'after']); expect(result.focusedIndex).toBe(1); expect(result.hidden).toBe(true);
    });
    it('updates the row in place when rejected rows are visible', () => {
        const result = applyVisibleDecision([image('focused')], 'focused', 'reject', true, 0);
        expect(result.items[0].selection?.decision).toBe('reject'); expect(result.focusedIndex).toBe(0); expect(result.hidden).toBe(false);
    });
});
