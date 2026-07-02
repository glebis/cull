import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { resolveAiSectionExpanded } from '$lib/onboarding';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('first-run onboarding (UX-03 + UX-04)', () => {
    describe('empty-library state is actionable', () => {
        const grid = source('src/lib/components/Grid.svelte');

        it('renders a working Import Folder button in the empty state', () => {
            expect(grid).toMatch(/<button[^<]*class="empty-import-btn"/);
            expect(grid).toMatch(/import folder/i);
            expect(grid).toContain('importFolderFromEmptyState');
        });

        it('hints at drag-and-drop import', () => {
            expect(grid.toLowerCase()).toContain('drag');
            expect(grid.toLowerCase()).toContain('drop');
        });

        it('mentions the agent/MCP import path', () => {
            expect(grid).toContain('MCP');
        });

        it('no longer points users at the sidebar as the only path', () => {
            expect(grid).not.toContain('Use the sidebar to import a folder');
        });

        it('uses a distinct clipboard monitor empty state for the active monitor collection', () => {
            expect(grid).toContain("clipboardMonitorEmptySrc");
            expect(grid).toContain("libraryViewState === 'empty' && isClipboardMonitorEmpty");
            expect(grid).toContain('Clipboard monitor is waiting');
            expect(grid).toContain('Copied images will appear here as they arrive.');
        });
    });

    describe('AI MODELS section defers to content on first run', () => {
        it('is collapsed by default while the library is empty', () => {
            expect(resolveAiSectionExpanded(null, 0)).toBe(false);
        });

        it('expands by default once the library has images', () => {
            expect(resolveAiSectionExpanded(null, 12)).toBe(true);
        });

        it('a manual toggle always wins over the default', () => {
            expect(resolveAiSectionExpanded(true, 0)).toBe(true);
            expect(resolveAiSectionExpanded(false, 12)).toBe(false);
        });

        it('Sidebar drives the section from resolveAiSectionExpanded', () => {
            const sidebar = source('src/lib/components/Sidebar.svelte');
            expect(sidebar).toContain('resolveAiSectionExpanded');
            expect(sidebar).not.toContain('aiExpanded = $state(true)');
        });
    });

    describe('model jargon and dead-ends are rewritten', () => {
        const sidebar = source('src/lib/components/Sidebar.svelte');

        it("replaces the 'Install model manually' dead-end with a setup-guide affordance", () => {
            expect(sidebar).not.toContain('Install model manually');
            expect(sidebar).toContain('MODEL_SETUP_GUIDE_URL');
            expect(sidebar).toMatch(/setup guide/i);
        });

        it("softens 'manual install' / 'offline' statuses to optional-integration framing", () => {
            expect(sidebar).not.toContain('>manual install<');
            expect(sidebar).not.toContain('>offline<');
            expect(sidebar.toLowerCase()).toContain('optional');
        });
    });
});
