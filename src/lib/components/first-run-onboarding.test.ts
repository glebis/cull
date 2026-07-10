import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

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
            expect(grid).toContain("(libraryViewState === 'scope-empty' || libraryViewState === 'empty') && isClipboardMonitorEmpty");
            expect(grid).toContain('Clipboard monitor is waiting');
            expect(grid).toContain('Copied images will appear here as they arrive.');
        });
    });

    describe('AI model setup stays out of first-run navigation', () => {
        it('keeps the sidebar focused on library content', () => {
            const sidebar = source('src/lib/components/Sidebar.svelte');
            expect(sidebar).not.toContain('AI MODELS');
            expect(sidebar).not.toContain('resolveAiSectionExpanded');
        });
    });

    describe('model jargon and dead-ends are rewritten', () => {
        const settings = source('src/lib/components/AiSettings.svelte');

        it("replaces the 'Install model manually' dead-end with a setup-guide affordance", () => {
            expect(settings).not.toContain('Install model manually');
            expect(settings).toContain('MODEL_SETUP_GUIDE_URL');
            expect(settings).toMatch(/setup guide/i);
        });

        it("softens 'manual install' / 'offline' statuses to optional-integration framing", () => {
            expect(settings).not.toContain('>manual install<');
            expect(settings).not.toContain('>offline<');
            expect(settings.toLowerCase()).toContain('optional');
        });
    });
});
