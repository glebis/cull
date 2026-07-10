import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const source = readFileSync(join(process.cwd(), 'src/lib/components/Sidebar.svelte'), 'utf8');

describe('sidebar detected-object filters', () => {
    it('keeps detected classes under Filters after removing the AI Models block', () => {
        const filters = source.indexOf('<div class="section-header">FILTERS</div>');
        const detected = source.indexOf('DETECTED OBJECTS', filters);
        expect(filters).toBeGreaterThan(-1);
        expect(detected).toBeGreaterThan(filters);
        expect(source).not.toContain('AI MODELS');
        expect(source).toContain("window.addEventListener('detected-classes-changed'");
    });
});
