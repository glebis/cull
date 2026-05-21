import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const pageSource = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');

describe('drop overlay UX', () => {
    it('uses app design tokens instead of hardcoded accent fallbacks', () => {
        expect(pageSource).not.toContain('var(--accent, #4a9eff)');
        expect(pageSource).toContain('var(--blue)');
    });

    it('keeps the overlay non-interactive so the drop reaches the Tauri window', () => {
        expect(pageSource).toContain('pointer-events: none;');
    });
});
