import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

describe('CommandBar search presets contract', () => {
    const source = readFileSync(join(process.cwd(), 'src/lib/components/CommandBar.svelte'), 'utf8');

    it('refreshes search presets when imported images change the library', () => {
        expect(source).toContain("'images:changed'");
        expect(source).toContain("'session-events-refresh'");
        expect(source).toContain('loadSearchPresets');
    });

    it('renders saved and autosuggested preset lists separately', () => {
        expect(source).toContain('savedSearchPresets');
        expect(source).toContain('autoSearchPresets');
    });
});
