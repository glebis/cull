import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const sidebar = readFileSync(join(process.cwd(), 'src/lib/components/Sidebar.svelte'), 'utf8');
const sessionSwitcher = readFileSync(join(process.cwd(), 'src/lib/components/SessionSwitcher.svelte'), 'utf8');
const page = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');
const stores = readFileSync(join(process.cwd(), 'src/lib/stores.ts'), 'utf8');
const palette = readFileSync(join(process.cwd(), 'src/lib/command-palette.ts'), 'utf8');
const commandBar = readFileSync(join(process.cwd(), 'src/lib/components/CommandBar.svelte'), 'utf8');

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

    it('keeps folder removal available from the keyboard tree', () => {
        expect(sidebar).toContain("case 'Delete':");
        expect(sidebar).toContain('handleDeleteFolder(row.fullPath)');
        expect(sidebar).toContain("aria-keyshortcuts={folder.isGroup ? undefined : 'Delete'}");
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
        const order = ['LIBRARY', 'COLLECTIONS', 'Smart', 'FILTERS', 'Clipboard Monitor'];
        const positions = order.map(label => sidebar.indexOf(`>${label}<`) !== -1
            ? sidebar.indexOf(`>${label}<`)
            : sidebar.indexOf(label));
        for (const pos of positions) expect(pos).toBeGreaterThan(-1);
        expect([...positions].sort((a, b) => a - b)).toEqual(positions);
    });

    it('labels clipboard monitor status values (M2)', () => {
        // Status card reads "Running" / "Stopped" with a captured-count
        // counter; folder + collection name share one muted row.
        expect(sidebar).toContain('clipboard-status-label');
        expect(sidebar).toContain('clipboard-status-count');
        expect(sidebar).toContain('clipboard-status-folder');
        expect(sidebar).toContain('clipboard-status-collection');
        // Access warning only surfaces when not 'supported'
        expect(sidebar).toContain('clipboardStatus.access_status !== \'supported\'');
        // Publish URL is actionable and surfaces just the host + copy glyph
        expect(sidebar).toContain('copyPublishUrl');
        expect(sidebar).toContain('function publishHost');
        expect(sidebar).toContain('class="publish-url-host"');
    });

    it('moves distinctly named batch analysis actions to the command palette (M6)', () => {
        expect(palette).toContain('Detect Objects in Library');
        expect(palette).toContain('Describe Images in Library');
        expect(palette).toContain('only on pending images');
        expect(sidebar).not.toContain('Analyze uncatalogued images');
    });

    it('keeps the sidebar footer to one contextual import action (M7)', () => {
        expect(sidebar).toContain('class="import-btn"');
        expect(sidebar).toContain('aria-label={importing ? \'Importing folder\' : \'Import folder\'}');
        expect(sidebar).toContain('<span aria-hidden="true">+</span>');
        expect(sidebar).not.toContain('footer-secondary-actions');
    });

    // imageview-1i2k.1 — post-import findability (JTBD o6, opp 11). The toast
    // expires in 8s; the rail must not.
    it('keeps the just-imported folder visible after the toast expires (recency rail)', () => {
        expect(sidebar).toContain('RECENT');
        expect(sidebar).toContain('recent-scope');
        expect(sidebar).toContain('just-imported');
        expect(sidebar).toContain('fresh: true');
        expect(sidebar).toContain('revealImportedFolderInTree');
        expect(sidebar).toContain('ancestorFolderPaths');
        // Recording happens on real selections and imports, persisted via stores
        expect(sidebar).toContain('recordRecentScope');
        expect(stores).toContain('recentScopes');
        // The toast action stays — the rail replaces the hunt, not the toast
        expect(sidebar).toContain("label: 'View imported'");
    });

    // imageview-1i2k.3 — zero counts on every row are noise (JTBD o10).
    it('omits zero counts and offers a hide-empty option', () => {
        // Unit-level behaviour lives in sidebar-utils.test.ts; here we pin
        // that rows actually use the omitting formatters.
        expect(sidebar).toContain('formatSidebarCount(count)');
        expect(sidebar).toContain('formatFolderCount(folder.count, folder.subtreeCount)');
        // The option exists, is wired to a persisted store, and filters rows
        expect(sidebar).toContain('bind:checked={$sidebarHideEmpty}');
        expect(sidebar).toContain('!$sidebarHideEmpty || f.subtreeCount > 0');
        expect(stores).toContain('sidebarHideEmpty');
    });

    // imageview-1i2k.7 — #7a7fa0 on #0c0c12 is APCA Lc ~40 at the sidebar's
    // 9-11px caption sizes. The sidebar overrides the token with a
    // lightness-only raise (Lc ~62); body-size text elsewhere keeps the
    // WCAG-AA-passing original.
    it('raises secondary-text contrast at caption sizes', () => {
        const appCss = readFileSync(join(process.cwd(), 'src/app.css'), 'utf8');
        expect(appCss).toContain('--text-caption:');
        const sidebarRule = sidebar.match(/\.sidebar\s*\{[^}]*\}/)?.[0] ?? '';
        expect(sidebarRule).toContain('--text-secondary: var(--text-caption)');
    });

    // imageview-1i2k.8 — sub-24px controls (8px twisty, ~19px preset chips)
    // expand their tap target with a negative-inset pseudo-element instead
    // of growing visually.
    it('gives twisty and preset chips a 24px hit-area floor', () => {
        const twisty = sidebar.match(/\.twisty::after\s*\{[^}]*\}/)?.[0] ?? '';
        expect(twisty).toContain("content: ''");
        expect(twisty).toContain('inset: -8px -5px');
        const preset = sidebar.match(/\.preset-btn::after\s*\{[^}]*\}/)?.[0] ?? '';
        expect(preset).toContain("content: ''");
        expect(preset).toContain('inset: -3px 0');
    });

    // imageview-1i2k.2 — one adaptive search: the sidebar filter covers every
    // scope list (detected classes and canvases were silently excluded), and
    // Enter promotes the query to a grid search through the CommandBar.
    it('sidebar filter covers detected classes and canvases; Enter promotes to grid', () => {
        expect(sidebar).toContain('visibleDetectedClasses');
        expect(sidebar).toContain('matchingCanvases');
        expect(sidebar).toContain('promoteFilterToGrid');
        expect(sidebar).toContain('pendingGridSearch.set(query)');
        // The store (not an event) carries the query because the CommandBar
        // only mounts in grid view; it consumes and clears it.
        expect(commandBar).toContain('$pendingGridSearch');
        expect(commandBar).toContain('pendingGridSearch.set(null)');
        expect(stores).toContain('pendingGridSearch');
    });

    // imageview-1i2k.4 — one geometric language, no emoji dingbats. The five
    // row-icon dialects (◼ ⏰ ◇ ★ + text-glyph ●) are gone; state markers
    // (pin, running dot) are CSS-drawn shapes.
    it('uses one geometric icon language with no emoji row glyphs', () => {
        for (const entity of ['&#9632;', '&#9200;', '&#9671;', '&#9733;']) {
            expect(sidebar).not.toContain(entity);
        }
        expect(sidebar).not.toContain('>●</span>');
        expect(sidebar).not.toContain('generated-pin::before');
        expect(sidebar).not.toContain('generated-pin::after');
        expect(sidebar).toContain('.pin-btn.active .generated-pin');
        expect(sidebar).toContain('border-radius: 50%');
        // Functional controls keep their glyphs: twisties and media controls
        expect(sidebar).toContain("'▾' : '▸'");
    });

    // imageview-1i2k.5 — one meaning per color: blue=interactive,
    // green=positive state, orange=active mode, purple=class tag + shortlist
    // membership (Selection Mode), red=error.
    it('keeps one meaning per accent color', () => {
        // Collect Mode was retired; Selection Mode carries membership in --purple.
        const selectionBar = readFileSync(join(process.cwd(), 'src/lib/components/SelectionModeBar.svelte'), 'utf8');
        expect(selectionBar).not.toContain('collect-indicator');
        // Green is positive state only: running dot + import success
        const runningDot = sidebar.match(/\.running-dot\s*\{[^}]*\}/)?.[0] ?? '';
        expect(runningDot).toContain('background: var(--green)');
        const importResult = sidebar.match(/\.import-result\s*\{[^}]*\}/)?.[0] ?? '';
        expect(importResult).toContain('color: var(--green)');
        // Purple is the detected-class tag and the shortlist marker token
        const classTag = sidebar.match(/\.class-tag\s*\{[^}]*\}/)?.[0] ?? '';
        expect(classTag).toContain('color: var(--purple)');
        const thumbnail = readFileSync(join(process.cwd(), 'src/lib/components/Thumbnail.svelte'), 'utf8');
        expect(thumbnail).toContain('badge.shortlisted');
        expect(thumbnail).toContain('var(--purple)');
        // The semantics are documented at the tokens
        const appCss = readFileSync(join(process.cwd(), 'src/app.css'), 'utf8');
        expect(appCss).toContain('one meaning per color');
    });

    // imageview-1i2k.6 — clipboard monitor promotion (guardrail: don't lose
    // it, make it easier). A persistent footer chip shows live status and
    // opens quick controls; the bottom section stays for detail.
    it('promotes the clipboard monitor to a persistent footer chip', () => {
        expect(sidebar).toContain('class="clipboard-chip"');
        expect(sidebar).toContain('clipboardPopoverOpen');
        expect(sidebar).toContain('class="clipboard-popover"');
        // The chip reflects live status and opens quick controls
        expect(sidebar).toContain('class:running={clipboardStatus?.running}');
        expect(sidebar).toContain('revealClipboardSection');
        // The detail section stays
        expect(sidebar).toContain('Clipboard Monitor');
    });

    // imageview-1i2k.9 — empty states teach the next action instead of
    // showing a bare label or vanishing.
    it('empty states teach the next action', () => {
        expect(sidebar).toContain('No collections yet — the + above creates one.');
        // The Smart section no longer vanishes when every collection is
        // empty; it renders an actionable empty state.
        expect(sidebar).toContain('{#if $smartCollections.length > 0}');
        expect(sidebar).toContain('Save Collection in the search bar');
        // The .3 hide-empty option gets its own honest empty state
        expect(sidebar).toContain('hidden while "Hide empty" is on');
    });

    // Clipboard monitor redesign — group by intent (status card, primary
    // toggle, secondary actions). Status uses captured_count for the
    // primary counter; folder + collection live on one muted row;
    // Move Folder + Publish are ghost buttons in a two-column grid;
    // the publish URL surfaces just the host + a copy glyph.
    it('clipboard monitor groups status, primary toggle, and secondaries', () => {
        expect(sidebar).toContain('class="clipboard-status"');
        expect(sidebar).toContain('clipboard-primary');
        expect(sidebar).toContain('class="section-actions"');
        expect(sidebar).toContain('function publishHost');
        expect(sidebar).toContain('class="publish-url-host"');
        expect(sidebar).toContain('class="publish-url-copy"');
        // Status block: dot, label, captured count, folder, collection
        expect(sidebar).toContain('clipboard-status-dot');
        expect(sidebar).toContain('clipboard-status-count');
        expect(sidebar).toContain('clipboard-status-folder');
        expect(sidebar).toContain('clipboard-status-collection');
    });

    // Alt-hover folder preview: the popover only opens when the Option key
    // is held (no surprise popups on a normal hover), uses listImagesByFolder
    // so the thumbnails match the actual folder scope, and closes on keyup so
    // an orphan preview can't linger after the user releases Alt.
    it('opens a folder preview only while Option is held', () => {
        expect(sidebar).toContain('scheduleFolderPreview');
        expect(sidebar).toContain('hideFolderPreview');
        expect(sidebar).toContain('listImagesByFolder');
        expect(sidebar).toContain('folder-preview-popover');
        expect(sidebar).toContain('folderPreviewSrc');
        // The handler returns immediately when altKey isn't held
        expect(sidebar).toContain('native.altKey === true');
        // Release of Alt clears the popover
        expect(sidebar).toContain("event.key === 'Alt'");
    });
});
