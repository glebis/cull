import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Preview Display monitor placement contract', () => {
    it('registers monitor listing and placement commands with UI permission', () => {
        const lib = source('src-tauri/src/lib.rs');
        const permissions = source('src-tauri/permissions/app-ui.toml');
        const api = source('src/lib/api.ts');

        expect(lib).toContain('commands::preview::list_preview_display_monitors');
        expect(lib).toContain('commands::preview::place_preview_display');
        expect(permissions).toContain('"list_preview_display_monitors"');
        expect(permissions).toContain('"place_preview_display"');
        expect(api).toContain("invoke<PreviewDisplayMonitor[]>('list_preview_display_monitors')");
        expect(api).toContain("invoke<string>('place_preview_display'");
    });

    it('adds View menu actions for monitor move and fullscreen placement', () => {
        const menu = source('src-tauri/src/menu.rs');
        const frontendMenu = source('src/lib/menu.ts');

        expect(menu).toContain('"preview_display_move_monitor"');
        expect(menu).toContain('"preview_display_fullscreen"');
        expect(frontendMenu).toContain("case 'preview_display_move_monitor'");
        expect(frontendMenu).toContain("case 'preview_display_fullscreen'");
        expect(frontendMenu).toContain('listPreviewDisplayMonitors');
        expect(frontendMenu).toContain('placePreviewDisplay');
    });
});
