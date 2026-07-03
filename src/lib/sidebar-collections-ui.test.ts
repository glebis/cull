import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const sidebar = readFileSync(join(process.cwd(), 'src/lib/components/Sidebar.svelte'), 'utf8');

describe('collections sidebar UI contract', () => {
    it('uses a generated hover pin action for collection rows', () => {
        expect(sidebar).toContain('class="pin-btn"');
        expect(sidebar).toContain('class="generated-pin"');
        expect(sidebar).toContain('.folder-row:hover .pin-btn');
        expect(sidebar).toContain('.pin-btn.active');
        expect(sidebar).not.toContain('📎');
        expect(sidebar).not.toContain('📌');
    });
});
