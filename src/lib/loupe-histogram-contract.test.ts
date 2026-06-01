import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

describe('Loupe histogram contract', () => {
    it('uses the canonical histogram command and shows clipping warnings', () => {
        const loupe = readFileSync(join(root, 'src/lib/components/Loupe.svelte'), 'utf8');

        expect(loupe).toContain('getImageHistogram');
        expect(loupe).toContain('histogramExposureWarnings');
        expect(loupe).toContain('loupe-histogram');
        expect(loupe).toContain('Clipped shadows');
        expect(loupe).toContain('Clipped highlights');
    });
});
