import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const sidebar = readFileSync(join(process.cwd(), 'src/lib/components/Sidebar.svelte'), 'utf8');
const sessionSwitcher = readFileSync(join(process.cwd(), 'src/lib/components/SessionSwitcher.svelte'), 'utf8');
const page = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');
const stores = readFileSync(join(process.cwd(), 'src/lib/stores.ts'), 'utf8');
const palette = readFileSync(join(process.cwd(), 'src/lib/command-palette.ts'), 'utf8');

describe('sidebar audit fixes contract', () => {
    it('renders errors in error styling, not success green (H1)', () => {
        expect(sidebar).toContain('lastResultKind');
        expect(sidebar).toContain('class:error={lastResultKind');
        expect(sidebar).toContain('.import-result.error');
    });

    it('wraps size filter presets instead of clipping them (H2)', () => {
        const presets = sidebar.match(/\.filter-presets\s*\{[^}]*\}/)?.[0] ?? '';
        expect(presets).toContain('flex-wrap: wrap');
    });

    it('does not expose a fake ARIA tree (H3)', () => {
        expect(sidebar).not.toContain('role="tree"');
        expect(sidebar).not.toContain('role="treeitem"');
        expect(sidebar).not.toContain('aria-level');
    });

    it('session switcher dropdown is dismissible and announced (H4)', () => {
        expect(sessionSwitcher).toContain('aria-expanded={open}');
        expect(sessionSwitcher).toContain('aria-haspopup');
        // Escape and outside-click both close the dropdown
        expect(sessionSwitcher).toMatch(/Escape/);
        expect(sessionSwitcher).toMatch(/onfocusout|pointerdown|svelte:window|svelte:document/);
    });

    it('destructive actions use the app confirm dialog, not window.confirm (H5)', () => {
        expect(sidebar).not.toContain('window.confirm');
        expect(sidebar).toContain('requestConfirm');
        expect(stores).toContain('export function requestConfirm');
        expect(page).toContain('<ConfirmDialog');
    });

    it('orders content sections before utilities, with Collections and Clipboard high (M1)', () => {
        const order = ['LIBRARY', 'COLLECTIONS', 'CLIPBOARD MONITOR', 'SMART', 'FILTERS'];
        const positions = order.map(label => sidebar.indexOf(`>${label}<`) !== -1
            ? sidebar.indexOf(`>${label}<`)
            : sidebar.indexOf(label));
        for (const pos of positions) expect(pos).toBeGreaterThan(-1);
        expect([...positions].sort((a, b) => a - b)).toEqual(positions);
    });

    it('labels clipboard monitor status values (M2)', () => {
        expect(sidebar).toContain('Access:');
        expect(sidebar).toContain('Folder:');
        // publish URL is actionable, not a dead truncated div
        expect(sidebar).toContain('copyPublishUrl');
    });

    it('moves distinctly named batch analysis actions to the command palette (M6)', () => {
        expect(palette).toContain('Detect Objects in Library');
        expect(palette).toContain('Describe Images in Library');
        expect(palette).toContain('only on pending images');
        expect(sidebar).not.toContain('Analyze uncatalogued images');
    });

    it('footer maintenance buttons state their action (M7)', () => {
        expect(sidebar).toContain('Rebuild thumbnails');
        expect(sidebar).toContain('Rescan sources');
    });
});
