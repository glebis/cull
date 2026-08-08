import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(component: string): string {
    return readFileSync(join(root, `src/lib/components/${component}.svelte`), 'utf8');
}

const dialogs = [
    { name: 'AboutDialog', overlay: 'about-overlay', panelClass: 'about-dialog', panelStyle: 'about-dialog' },
    { name: 'AgentSkillsDialog', overlay: 'agent-skills-overlay', panelClass: 'agent-skills-dialog', panelStyle: 'agent-skills-dialog' },
    { name: 'ContactSheetDialog', overlay: 'cs-backdrop', panelClass: 'cs-panel', panelStyle: 'cs-panel' },
    { name: 'ExportFolderDialog', overlay: 'export-backdrop', panelClass: 'export-panel', panelStyle: 'export-panel' },
    { name: 'GroupRankingDialog', overlay: 'gr-backdrop', panelClass: 'gr-panel', panelStyle: 'gr-panel' },
    { name: 'KeyboardShortcuts', overlay: 'shortcuts-backdrop', panelClass: 'shortcuts-panel', panelStyle: 'shortcuts-panel' },
    { name: 'TextInputDialog', overlay: 'text-input-dialog-overlay', panelClass: 'dialog text-input-dialog', panelStyle: 'dialog.text-input-dialog' },
    { name: 'CollectionTargetDialog', overlay: 'collection-target-dialog-overlay', panelClass: 'dialog collection-target-dialog', panelStyle: 'dialog.collection-target-dialog' },
    { name: 'McpSettings', overlay: 'settings-overlay', panelClass: 'settings-panel', panelStyle: 'settings-panel' },
] as const;

describe('ModalDialog migration contract', () => {
    it.each(dialogs)('$name delegates modal semantics and focus management to ModalDialog', ({ name, overlay, panelClass, panelStyle }) => {
        const component = source(name);

        expect(component).toContain("import ModalDialog from '$lib/components/ModalDialog.svelte';");
        expect(component).toContain('<ModalDialog');
        expect(component).toMatch(/titleId="[^"]+"/);
        expect(component).toMatch(/onclose=/);
        expect(component).toContain(`overlayClass="${overlay}"`);
        expect(component).toContain(`panelClass="${panelClass}"`);
        expect(component).not.toContain('role="dialog"');
        expect(component).not.toContain('aria-modal="true"');
        expect(component).not.toContain('<dialog');
        expect(component).not.toContain('<svelte:window onkeydown=');
        expect(component).toContain(`:global(.${overlay})`);
        expect(component).toContain(`:global(.${panelStyle})`);
    });

    it('removes duplicate backdrop Escape handlers from store-driven dialogs', () => {
        for (const name of ['ContactSheetDialog', 'ExportFolderDialog', 'GroupRankingDialog', 'KeyboardShortcuts']) {
            expect(source(name)).not.toMatch(/function (?:on|handle)BackdropKeydown/);
        }
    });

    it('preserves the top-aligned layout of tall store-driven dialogs', () => {
        expect(source('ModalDialog')).toContain('align-items: var(--modal-align-items, center);');
        for (const name of ['ContactSheetDialog', 'ExportFolderDialog', 'GroupRankingDialog', 'KeyboardShortcuts']) {
            expect(source(name)).toContain('--modal-align-items: flex-start;');
        }
    });

    it('removes manual panel focus from Settings in favor of an explicit ModalDialog target', () => {
        const settings = source('McpSettings');

        expect(settings).toContain('initialFocus=".settings-tab.active"');
        expect(settings).not.toContain('panelElement');
        expect(settings).not.toMatch(/:global\(\.(?:panel|overlay)\)/);
    });

    it('submits text-entry dialogs only from their text inputs', () => {
        expect(source('TextInputDialog')).toContain("e.key === 'Enter' && e.target === inputEl");
        expect(source('CollectionTargetDialog')).toContain(
            "e.key === 'Enter' && (e.target === searchInputEl || e.target === nameInputEl)"
        );
    });
});
