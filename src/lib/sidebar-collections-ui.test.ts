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
        expect(sidebar).toContain('buildPinnedCollectionRows');
        expect(sidebar).not.toContain('pinned-indicator');
        expect(sidebar).not.toContain('📎');
        expect(sidebar).not.toContain('📌');
    });

    it('exposes collection preview and context actions', () => {
        expect(sidebar).toContain('collection-preview-popover');
        expect(sidebar).toContain('setTimeout(async () =>');
        expect(sidebar).toContain('}, 1000)');
        expect(sidebar).toContain('collection-context-menu');
        expect(sidebar).toContain('Rename...');
        expect(sidebar).toContain('Export to Folder...');
        expect(sidebar).toContain('Use for Collect Mode');
    });
});
