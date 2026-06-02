import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { VIEW_TABS, visibleViewTabs } from './view-tabs';

const tabBarSource = readFileSync(join(process.cwd(), 'src/lib/components/TabBar.svelte'), 'utf8');
const pageSource = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');
const appCssSource = readFileSync(join(process.cwd(), 'src/app.css'), 'utf8');
const viewTabIconPath = join(process.cwd(), 'src/lib/components/ViewTabIcon.svelte');
const viewTabIconSource = existsSync(viewTabIconPath)
    ? readFileSync(viewTabIconPath, 'utf8')
    : '';

describe('view tabs', () => {
    it('hides Publish while the Static Publishing module is disabled', () => {
        const ids = visibleViewTabs(false).map(tab => tab.id);

        expect(ids).not.toContain('publish');
        expect(ids).toContain('export');
    });

    it('shows Publish immediately before Export when Static Publishing is enabled', () => {
        const ids = visibleViewTabs(true).map(tab => tab.id);

        expect(ids).toContain('publish');
        expect(ids.indexOf('publish')).toBe(ids.indexOf('export') - 1);
    });

    it('uses purpose-built icon ids for the primary image workflow tabs', () => {
        const icons = Object.fromEntries(VIEW_TABS.map(tab => [tab.id, tab.icon]));

        expect(icons.grid).toBe('grid-contact-sheet');
        expect(icons.loupe).toBe('loupe-focus');
        expect(icons.export).toBe('export-package');
    });

    it('renders tab icons as vector SVGs instead of font glyphs for Retina clarity', () => {
        expect(tabBarSource).toContain('<ViewTabIcon');
        expect(tabBarSource).not.toContain('{tab.icon}</span>');
        expect(viewTabIconSource).toContain('<svg');
        expect(viewTabIconSource).toContain('viewBox="0 0 24 24"');
    });

    it('marks the app header as a native Tauri drag region', () => {
        expect(tabBarSource).toContain('<div class="tabbar" data-tauri-drag-region="deep">');
    });

    it('uses an icon-only Preview Display launcher instead of showing window title text', () => {
        expect(tabBarSource).toContain("import { openPreviewDisplay } from '$lib/api';");
        expect(tabBarSource).toContain('class="preview-display-launch"');
        expect(tabBarSource).toContain('class="preview-display-icon"');
        expect(tabBarSource).toContain('onclick={openPreviewDisplayWindow}');
        expect(tabBarSource).toContain('openPreviewDisplay().catch');
        expect(tabBarSource).not.toContain('windowName');
        expect(tabBarSource).not.toContain('class="window-name"');
        expect(tabBarSource).not.toContain('Cull Preview Display');
    });

    it('keeps zen-mode content below the macOS window controls', () => {
        expect(appCssSource).toContain('--macos-titlebar-safe-area: 40px;');
        expect(tabBarSource).toContain('padding-left: var(--macos-window-controls-width);');
        expect(pageSource).toContain('padding-top: var(--macos-titlebar-safe-area);');
    });
});
