import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Loupe histogram contract', () => {
    it('uses the canonical histogram command and shows clipping warnings', () => {
        const loupe = source('src/lib/components/Loupe.svelte');

        expect(loupe).toContain('getImageHistogram');
        expect(loupe).toContain('histogramExposureWarnings');
        expect(loupe).toContain('loupe-histogram');
        expect(loupe).toContain('Clipped shadows');
        expect(loupe).toContain('Clipped highlights');
    });

    it('keeps the Loupe histogram hidden by default behind the View menu preference', () => {
        const stores = source('src/lib/stores.ts');
        const loupe = source('src/lib/components/Loupe.svelte');

        expect(stores).toContain('export const showLoupeHistogram = writable<boolean>(false)');
        expect(loupe).toContain('showLoupeHistogram');
        expect(loupe).toContain('{#if !hideOverlays && $showLoupeHistogram && histogram}');
    });

    it('labels the histogram with a title, meaning, and readable source', () => {
        const loupe = source('src/lib/components/Loupe.svelte');

        expect(loupe).toContain('class="histogram-title"');
        expect(loupe).toContain('Histogram');
        expect(loupe).toContain('Luma + RGB tonal distribution');
        expect(loupe).toContain('histogramSourceLabel');
        expect(loupe).not.toContain('<span class="histogram-source">{histogram.source}</span>');
    });
});
