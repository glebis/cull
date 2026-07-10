import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Preview Display control contract', () => {
    function viewMenuSource(): string {
        const menu = source('src-tauri/src/menu.rs');
        const start = menu.indexOf('// View menu');
        const end = menu.indexOf('menu.append(&view_menu)?;', start);

        return menu.slice(start, end);
    }

    it('adds View menu actions for freeze, blank, and presets', () => {
        const menu = source('src-tauri/src/menu.rs');

        for (const id of [
            'preview_display_freeze',
            'preview_display_blank',
            'preview_display_preset_image_only',
            'preview_display_preset_client_review',
            'preview_display_preset_metadata_review',
            'preview_display_layout_single',
            'preview_display_layout_compare',
            'preview_display_layout_grid',
            'preview_display_copy_to_clipboard',
            'preview_display_export_png',
        ]) {
            expect(menu).toContain(`"${id}"`);
        }
        expect(menu).toContain('preview_display_frozen');
        expect(menu).toContain('preview_display_blanked');
        expect(menu).toContain('preview_display_mode');
        expect(menu).toContain('preview_display_layout');
    });

    it('adds View menu field toggles and bounded info rail controls', () => {
        const menu = source('src-tauri/src/menu.rs');
        const frontendMenu = source('src/lib/menu.ts');

        for (const id of [
            'preview_display_field_filename',
            'preview_display_field_rating',
            'preview_display_field_decision',
            'preview_display_field_dimensions',
            'preview_display_field_format',
            'preview_display_field_source',
            'preview_display_field_prompt',
            'preview_display_field_tags',
            'preview_display_field_histogram',
            'preview_display_rail_left',
            'preview_display_rail_right',
            'preview_display_rail_width_narrow',
            'preview_display_rail_width_medium',
            'preview_display_rail_width_wide',
            'preview_display_text_small',
            'preview_display_text_medium',
            'preview_display_text_large',
        ]) {
            expect(menu).toContain(`"${id}"`);
            expect(frontendMenu).toContain(`case '${id}'`);
        }

        expect(menu).toContain('preview_display_overlay');
    });

    it('handles menu actions in the main window and persists preset settings', () => {
        const frontendMenu = source('src/lib/menu.ts');

        expect(frontendMenu).toContain("case 'preview_display_freeze'");
        expect(frontendMenu).toContain("case 'preview_display_blank'");
        expect(frontendMenu).toContain("case 'preview_display_preset_client_review'");
        expect(frontendMenu).toContain("case 'preview_display_layout_grid'");
        expect(frontendMenu).toContain("case 'preview_display_export_png'");
        expect(frontendMenu).toContain('setPreviewDisplayMode');
        expect(frontendMenu).toContain('setPreviewDisplayLayout');
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

    it('keeps Preview Display controls nested instead of crowding the top-level View menu', () => {
        const viewMenu = viewMenuSource();

        expect(viewMenu).toContain('Submenu::new(app, "Preview Display", true)?');
        expect(viewMenu).toContain('view_menu.append(&preview_display_menu)?');
        expect(viewMenu).not.toMatch(/view_menu\.append\(&(?:Check)?MenuItem::with_id\(\s*app,\s*"preview_display_/);
    });
});
