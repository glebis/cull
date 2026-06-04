import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Preview Display native window contract', () => {
    it('wires the View menu item, dedicated shortcut, and frontend handler', () => {
        const menu = source('src-tauri/src/menu.rs');
        const frontendMenu = source('src/lib/menu.ts');

        expect(menu).toContain('"view_preview_display"');
        expect(menu).toContain('"Preview Display"');
        // Cmd+Shift+D ("Display") — Cmd+Shift+P is reserved for the command palette alias.
        expect(menu).toContain('Some::<&str>("CmdOrCtrl+Shift+D")');
        expect(menu).toMatch(/"view_preview_display"[\s\S]*app\.emit\("menu-action", id\)/);
        expect(frontendMenu).toContain('openPreviewDisplay');
        expect(frontendMenu).toContain("case 'view_preview_display'");
    });

    it('registers open_preview_display and grants it through app-ui', () => {
        const lib = source('src-tauri/src/lib.rs');
        const permissions = source('src-tauri/permissions/app-ui.toml');
        const api = source('src/lib/api.ts');

        expect(lib).toContain('commands::preview::open_preview_display');
        expect(lib).toContain('commands::preview::set_preview_display_always_on_top');
        expect(permissions).toContain('"open_preview_display"');
        expect(permissions).toContain('"set_preview_display_always_on_top"');
        expect(api).toContain("invoke<string>('open_preview_display')");
        expect(api).toContain("invoke<boolean>('set_preview_display_always_on_top'");
    });

    it('scopes the preview-display window to read-only and UI capabilities', () => {
        const capabilityPath = join(root, 'src-tauri/capabilities/preview-display.json');
        expect(existsSync(capabilityPath)).toBe(true);

        const capability = JSON.parse(readFileSync(capabilityPath, 'utf8'));
        expect(capability.windows).toEqual(['preview-display']);
        expect(capability.permissions).toEqual([
            'core:default',
            'opener:default',
            'app-read',
            'app-ui',
        ]);
        expect(capability.permissions).not.toContain('app-curation');
        expect(capability.permissions).not.toContain('app-file-access');
        expect(capability.permissions).not.toContain('app-ai-processing');
        expect(capability.permissions).not.toContain('app-export-publishing');
    });
});
