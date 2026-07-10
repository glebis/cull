import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(fileURLToPath(new URL('./McpSettings.svelte', import.meta.url)), 'utf8');

describe('Settings tabs', () => {
    it('exposes the approved tabs in order with accessible tab semantics', () => {
        const labels = ['General', 'Appearance', 'AI', 'Agent Access', 'Privacy', 'Plugins'];
        let cursor = -1;
        for (const label of labels) {
            const next = source.indexOf(`label: '${label}'`, cursor + 1);
            expect(next, `${label} tab`).toBeGreaterThan(cursor);
            cursor = next;
        }
        expect(source).toContain('role="tablist"');
        expect(source).toContain('role="tab"');
        expect(source).toContain('aria-selected=');
        expect(source).toContain('role="tabpanel"');
    });
});
