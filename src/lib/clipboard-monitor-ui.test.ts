import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const sidebar = readFileSync(join(process.cwd(), 'src/lib/components/Sidebar.svelte'), 'utf8');

describe('clipboard monitor sidebar UI contract', () => {
    it('renders operational clipboard monitor controls in the sidebar', () => {
        // The section is now a collapse toggle; `.folders-toggle-label` applies
        // text-transform: uppercase, so it still reads CLIPBOARD MONITOR.
        expect(sidebar).toContain('Clipboard Monitor');
        expect(sidebar).toContain('startClipboardMonitor');
        expect(sidebar).toContain('stopClipboardMonitor');
        expect(sidebar).toContain('setClipboardMonitorCaptureExistingOnStart');
        expect(sidebar).toContain('Capture current image on start');
        expect(sidebar).toContain('publishClipboardCollection');
        expect(sidebar).toContain('navigator.clipboard.writeText');
    });
});
