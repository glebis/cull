import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

const customViewMenuActions = [
    'view_grid',
    'view_loupe',
    'view_compare',
    'view_canvas',
    'view_lineage',
    'view_embeddings',
    'view_publish',
    'view_export',
    'toggle_sidebar',
    'view_loupe_histogram',
    'view_preview_display',
    'preview_display_move_monitor',
    'preview_display_fullscreen',
    'preview_display_always_on_top',
    'preview_display_start_web_stream',
    'preview_display_start_lan_web_stream',
    'preview_display_copy_web_stream_url',
    'preview_display_stop_web_stream',
    'preview_display_freeze',
    'preview_display_blank',
    'preview_display_preset_image_only',
    'preview_display_preset_client_review',
    'preview_display_preset_metadata_review',
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
    'zoom_in',
    'zoom_out',
    'actual_size',
    'fit_in',
];

describe('native window reveal contract', () => {
    it('reveals the main window when macOS reopens a hidden tray instance', () => {
        const lib = source('src-tauri/src/lib.rs');
        const tray = source('src-tauri/src/tray.rs');

        expect(lib).toContain('pub(crate) fn reveal_main_window(app: &AppHandle)');
        expect(lib).toMatch(
            /fn reveal_main_window[\s\S]*window\.show\(\)[\s\S]*window\.unminimize\(\)[\s\S]*window\.set_focus\(\)/
        );
        expect(lib).toContain('tauri::RunEvent::Reopen');
        expect(lib).toContain('reveal_main_window(app);');
        expect(lib).toContain('tauri::RunEvent::Opened');
        expect(tray).toMatch(
            /window\.show\(\);[\s\S]*window\.unminimize\(\);[\s\S]*window\.set_focus\(\);/
        );
    });
});

describe('native View menu contract', () => {
    it('routes every custom View menu action to a frontend feature handler', () => {
        const nativeMenu = source('src-tauri/src/menu.rs');
        const frontendMenu = source('src/lib/menu.ts');

        for (const id of customViewMenuActions) {
            expect(nativeMenu).toMatch(new RegExp(`\\|\\s*"${id}"`));
            expect(frontendMenu).toContain(`case '${id}'`);
        }
    });

    it('keeps Preview Display Always on Top wired through native state and frontend handling', () => {
        const nativeMenu = source('src-tauri/src/menu.rs');
        const frontendMenu = source('src/lib/menu.ts');
        const api = source('src/lib/api.ts');
        const store = source('src/lib/preview-display-store.ts');

        expect(nativeMenu).toContain('"preview_display_always_on_top"');
        expect(nativeMenu).toContain('preview_display_always_on_top: bool');
        expect(nativeMenu).toContain('state.preview_display_always_on_top');
        expect(frontendMenu).toContain("case 'preview_display_always_on_top'");
        expect(frontendMenu).toContain('setPreviewDisplayAlwaysOnTopNative');
        expect(frontendMenu).toContain('previewDisplayAlwaysOnTop.subscribe(queueMenuStateUpdate)');
        expect(api).toContain("invoke<boolean>('set_preview_display_always_on_top'");
        expect(store).toContain('previewDisplayAlwaysOnTop');
    });

    it('lists native app windows in the Window menu and focuses selected windows', () => {
        const nativeMenu = source('src-tauri/src/menu.rs');
        const windowCommands = source('src-tauri/src/commands/window.rs');
        const previewCommands = source('src-tauri/src/commands/preview.rs');
        const lib = source('src-tauri/src/lib.rs');

        expect(nativeMenu).toContain('WINDOW_MENU_ID');
        expect(nativeMenu).toContain('Submenu::with_id(app, WINDOW_MENU_ID, "Window", true)');
        expect(nativeMenu).toContain('WINDOW_MENU_FOCUS_PREFIX');
        expect(nativeMenu).toContain('refresh_window_menu');
        expect(nativeMenu).toContain('focus_window_from_menu');
        expect(nativeMenu).toContain('window.is_focused().unwrap_or(false)');
        expect(windowCommands).toContain('crate::menu::refresh_window_menu(&app)');
        expect(previewCommands).toContain('crate::menu::refresh_window_menu(&app)');
        expect(lib).toContain('tauri::WindowEvent::Destroyed | tauri::WindowEvent::Focused(_)');
    });
});
