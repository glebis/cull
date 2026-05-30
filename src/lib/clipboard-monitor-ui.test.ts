import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const sidebar = readFileSync(join(process.cwd(), 'src/lib/components/Sidebar.svelte'), 'utf8');

describe('clipboard monitor sidebar UI contract', () => {
    it('renders operational clipboard monitor controls in the sidebar', () => {
        expect(sidebar).toContain('CLIPBOARD MONITOR');
        expect(sidebar).toContain('startClipboardMonitor');
        expect(sidebar).toContain('stopClipboardMonitor');
        expect(sidebar).toContain('publishClipboardCollection');
        expect(sidebar).toContain('navigator.clipboard.writeText');
    });
});
