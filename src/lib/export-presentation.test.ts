import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const exportSource = readFileSync(join(process.cwd(), 'src/lib/components/Export.svelte'), 'utf8');
const keySource = readFileSync(join(process.cwd(), 'src/lib/keys.ts'), 'utf8');

describe('export presentation modes', () => {
    it('routes Shift+. in export mode through the three-stage presentation state', () => {
        expect(keySource).toContain("mode === 'export'");
        expect(keySource).toContain('nextExportPresentationState');
        expect(keySource).toContain('exportImageOnly.set(next.imageOnly)');
    });

    it('has an image-only export layout with no visible controls or slide text', () => {
        expect(exportSource).toContain('class:images-only={imageOnly}');
        expect(exportSource).toContain('{#if !imageOnly}');
        expect(exportSource).toContain('class="image-only-grid"');
        expect(exportSource).toContain('{#each selectedImages as selectedImage}');
        expect(exportSource).toContain('alt=""');
        expect(exportSource).toContain('.export-view.images-only .export-toolbar');
        expect(exportSource).toContain('.export-view.images-only .preview-label');
    });
});
