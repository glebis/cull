import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const exportSource = readFileSync(join(process.cwd(), 'src/lib/components/Export.svelte'), 'utf8');

describe('Export launch event contract', () => {
    it('listens for global export launch events and runs the export action', () => {
        expect(exportSource).toContain('cull-export-launch');
        expect(exportSource).toContain('window.addEventListener');
        expect(exportSource).toContain('window.removeEventListener');
        expect(exportSource).toContain('exportSlides');
    });
});
