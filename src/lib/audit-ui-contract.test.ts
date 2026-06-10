import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

function source(path: string): string {
    return readFileSync(join(process.cwd(), path), 'utf8');
}

const compare = source('src/lib/components/Compare.svelte');
const commandPalette = source('src/lib/components/CommandPalette.svelte');
const trashDialog = source('src/lib/components/TrashConfirmDialog.svelte');
const settings = source('src/lib/components/McpSettings.svelte');
const staticPublishing = source('src/lib/plugins/cull-publish/PublishView.svelte');
const embeddingExplorer = source('src/lib/components/EmbeddingExplorer.svelte');
const lineage = source('src/lib/components/LineageView.svelte');
const toast = source('src/lib/components/Toast.svelte');
const commandBar = source('src/lib/components/CommandBar.svelte');
const ruleBuilder = source('src/lib/components/RuleBuilder.svelte');
const tabBar = source('src/lib/components/TabBar.svelte');
const sidebar = source('src/lib/components/Sidebar.svelte');
const tauriConfig = JSON.parse(source('src-tauri/tauri.conf.json'));

describe('impeccable audit UI contracts', () => {
    it('makes Compare panels keyboard-operable and named', () => {
        expect(compare).not.toContain('onkeydown={() => {}}');
        expect(compare).toContain('function handlePanelKeydown');
        expect(compare).toContain('aria-label={comparePanelLabel');
        expect(compare).toContain('aria-pressed={$compareActiveSide === 0}');
        expect(compare).toContain('aria-pressed={$compareActiveSide === 1}');
    });

    it('uses complete modal semantics for trash confirmation and settings', () => {
        expect(trashDialog).toContain('import ModalDialog');
        expect(trashDialog).toContain('<ModalDialog');
        expect(trashDialog).toContain('titleId="trash-confirm-title"');
        expect(trashDialog).toContain('aria-label="Close trash confirmation"');
        expect(trashDialog).toContain('data-modal-initial-focus');
        expect(trashDialog).toContain('id="trash-confirm-title"');

        expect(settings).toContain('role="dialog"');
        expect(settings).toContain('aria-modal="true"');
        expect(settings).toContain('aria-labelledby="settings-title"');
        expect(settings).toContain('aria-label="Close settings"');
    });

    it('uses shared modal semantics for the command palette', () => {
        expect(commandPalette).toContain('import ModalDialog');
        expect(commandPalette).toContain('onclose={closePalette}');
        expect(commandPalette).toContain('titleId={COMMAND_PALETTE_TITLE_ID}');
        expect(commandPalette).toContain('titleId="set-hotkey-title"');
        expect(commandPalette).toContain('onclose={closeHotkeyCapture}');
        expect(commandPalette).toContain('overlayClass="command-palette-overlay"');
        expect(commandPalette).toContain('overlayClass="hotkey-modal-overlay"');
    });

    it('exposes toggle state through accessible button state', () => {
        for (const state of ['closeToTray', 'confirmTrash', 'autoUpdate', 'httpEnabled', 'autoPurge']) {
            expect(settings).toContain(`aria-pressed={${state}}`);
        }
        expect(staticPublishing).toContain('aria-pressed={indexable}');
        for (const state of ['largePreviewOpen', 'textOutputOpen', 'canvasLabelsOpen']) {
            expect(embeddingExplorer).toContain(`aria-pressed={${state}}`);
        }
    });

    it('keeps legacy surfaces on defined accessible design tokens', () => {
        expect(lineage).not.toMatch(/var\(--(?:text-primary|bg-elevated|accent-warm|bg-hover|accent),/);
        expect(toast).not.toContain('var(--text-primary');
        expect(toast).not.toContain('var(--accent');
        expect(toast).not.toContain('#565f89');
    });

    it('removes audited visual anti-patterns from product chrome', () => {
        expect(toast).not.toMatch(/border-left:\s*[2-9]px/);
        expect(commandBar).not.toContain('linear-gradient');
        expect(embeddingExplorer).not.toContain('backdrop-filter');
    });

    it('defines minimum window size and usable dense control targets', () => {
        expect(tauriConfig.app.windows[0].minWidth).toBeGreaterThanOrEqual(960);
        expect(tauriConfig.app.windows[0].minHeight).toBeGreaterThanOrEqual(640);

        expect(commandBar).toContain('min-height: 32px;');
        expect(ruleBuilder).toContain('width: 24px;');
        expect(ruleBuilder).toContain('height: 24px;');
        expect(tabBar).toContain('width: 16px;');
        expect(tabBar).toContain('height: 16px;');
    });

    it('clarifies destructive, privacy-sensitive, and publishing copy', () => {
        expect(sidebar).toContain('Remove folder from library');
        // Model setup must not imply an in-app auto-download (bd imageview-dkz.19
        // replaced the 'Install model manually' dead-end with a setup-guide link).
        expect(sidebar).toContain('Setup guide');
        expect(sidebar).not.toMatch(/Download model/i);
        expect(sidebar).toContain('Analyze uncatalogued images');
        expect(sidebar).toContain('Publish clipboard collection');
        expect(settings).toContain('Remote access settings');
        expect(staticPublishing).toContain('Allow search indexing');
        expect(staticPublishing).toContain('Start Local Preview');
    });
});
