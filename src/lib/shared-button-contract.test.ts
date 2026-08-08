import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

const sharedButtonConsumers = [
    'src/lib/components/TextInputDialog.svelte',
    'src/lib/components/CollectionTargetDialog.svelte',
    'src/lib/components/PromptResubmitDialog.svelte',
    'src/lib/components/TrashConfirmDialog.svelte',
    'src/lib/components/ConfirmDialog.svelte',
];

describe('shared dialog button contract', () => {
    it('defines canonical button variants and interaction states globally', () => {
        const appCss = source('src/app.css');

        expect(appCss).toMatch(/\.dialog \.btn\s*{/);
        expect(appCss).toMatch(/\.dialog \.btn\.primary\s*{/);
        expect(appCss).toMatch(/\.dialog \.btn\.secondary\s*{/);
        expect(appCss).toMatch(/\.dialog \.btn\.danger\s*{/);
        expect(appCss).toMatch(/\.dialog \.close-btn\s*{/);
        expect(appCss).toMatch(/\.dialog \.btn:hover:not\(:disabled\)/);
        expect(appCss).toMatch(/\.dialog \.btn:disabled/);
        expect(appCss).toMatch(/\.dialog \.btn:focus-visible/);
        expect(appCss).toMatch(/\.dialog \.close-btn:hover/);
        expect(appCss).toMatch(/\.dialog \.close-btn:focus-visible/);
        expect(appCss).not.toMatch(/(?:^|\n)\.close-btn\s*{/);
        expect(appCss).toContain('border-radius: 0;');
        expect(appCss).toContain('padding: var(--spacing) calc(var(--spacing) * 2);');
    });

    it('keeps dialog components from redeclaring the shared button layer', () => {
        for (const path of sharedButtonConsumers) {
            const component = source(path);
            const hasDialogScope = component.includes('class="dialog"')
                || /panelClass="[^"]*\bdialog\b[^"]*"/.test(component);

            expect(hasDialogScope, `${path} must opt into the shared dialog button scope`).toBe(true);
            expect(component, path).not.toMatch(/\n\s*\.btn(?:[.:\s{])/);
            expect(component, path).not.toMatch(/\n\s*\.close-btn(?:[.:\s{])/);
        }
    });

    it('marks the Trash confirmation action as destructive', () => {
        const dialog = source('src/lib/components/TrashConfirmDialog.svelte');

        expect(dialog).toContain('class="btn danger"');
        expect(dialog).not.toContain('class="btn primary" data-modal-initial-focus');
    });

    it('keeps generic confirmation variants mutually exclusive', () => {
        const dialog = source('src/lib/components/ConfirmDialog.svelte');

        expect(dialog).toContain('class:primary={!$confirmDialog.danger}');
        expect(dialog).toContain('class:danger={$confirmDialog.danger}');
        expect(dialog).not.toContain('class="btn primary" class:danger');
    });
});
