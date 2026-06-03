import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Preview Display control contract', () => {
    it('adds View menu actions for freeze, blank, and presets', () => {
        const menu = source('src-tauri/src/menu.rs');

        for (const id of [
            'preview_display_freeze',
            'preview_display_blank',
            'preview_display_preset_image_only',
            'preview_display_preset_client_review',
            'preview_display_preset_metadata_review',
        ]) {
            expect(menu).toContain(`"${id}"`);
        }
        expect(menu).toContain('preview_display_frozen');
        expect(menu).toContain('preview_display_blanked');
        expect(menu).toContain('preview_display_mode');
    });

    it('handles menu actions in the main window and persists preset settings', () => {
        const frontendMenu = source('src/lib/menu.ts');

        expect(frontendMenu).toContain("case 'preview_display_freeze'");
        expect(frontendMenu).toContain("case 'preview_display_blank'");
        expect(frontendMenu).toContain("case 'preview_display_preset_client_review'");
        expect(frontendMenu).toContain('setPreviewDisplayMode');
        expect(frontendMenu).toContain('setAppSetting');
    });

    it('shows a status indicator and prevents focus churn while frozen or blanked', () => {
        const statusBar = source('src/lib/components/StatusBar.svelte');
        const page = source('src/routes/+page.svelte');

        expect(statusBar).toContain('previewDisplayStatusLabel');
        expect(statusBar).toContain('preview-status');
        expect(page).toContain('previewDisplayFrozen');
        expect(page).toContain('previewDisplayBlanked');
        expect(page).toContain('previewSyncImageId');
    });
});
