import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const sidebar = readFileSync(join(process.cwd(), 'src/lib/components/Sidebar.svelte'), 'utf8');
const sessionSwitcher = readFileSync(join(process.cwd(), 'src/lib/components/SessionSwitcher.svelte'), 'utf8');
const page = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');
const stores = readFileSync(join(process.cwd(), 'src/lib/stores.ts'), 'utf8');

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

    // Supersedes the earlier "does not expose a fake ARIA tree" contract. That
    // rule existed because the roles were present with no keyboard behaviour
    // behind them. The behaviour now exists, so the requirement inverts: the
    // roles must be there, and they must stay backed by real key handling.
    it('exposes a real ARIA tree backed by keyboard navigation (H3)', () => {
        expect(sidebar).toContain('role="tree"');
        expect(sidebar).toContain('role="treeitem"');
        expect(sidebar).toContain('aria-level');
        expect(sidebar).toContain('handleTreeKeydown');
        for (const key of ['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End']) {
            expect(sidebar).toContain(`'${key}'`);
        }
        // Roving tabindex: exactly one row is tabbable at a time, and the index
        // is clamped so a shrinking list (filter typed, ancestor collapsed)
        // cannot leave the tree with no tabbable row.
        expect(sidebar).toContain('tabindex={i === treeTabIndex ? 0 : -1}');
        expect(sidebar).toContain('Math.min(treeFocusIndex, visibleFolders.length - 1)');
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

    // Revises M1's ordering. The original principle — content first, utilities
    // last — is kept, but Clipboard Monitor is a capture utility, not content,
    // so ranking it above Smart contradicted the rule it was written under.
    // Navigation targets (Library/Collections/Smart) now come first.
    it('orders navigation targets before utilities (M1)', () => {
        const order = ['LIBRARY', 'COLLECTIONS', 'Smart', 'FILTERS', 'Clipboard Monitor', 'AI MODELS'];
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

    it('names batch analysis actions distinctly with remaining counts (M6)', () => {
        expect(sidebar).toContain('Detect objects');
        expect(sidebar).toContain('Describe images');
        expect(sidebar).toContain('remaining');
        expect(sidebar).not.toContain('Analyze uncatalogued images');
    });

    it('footer maintenance buttons state their action (M7)', () => {
        expect(sidebar).toContain('Rebuild thumbnails');
        expect(sidebar).toContain('Rescan sources');
    });
});
