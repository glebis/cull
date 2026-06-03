import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Preview Display web stream contract', () => {
    it('registers web stream commands in Rust, permissions, and API layer', () => {
        const lib = source('src-tauri/src/lib.rs');
        const permissions = source('src-tauri/permissions/app-ui.toml');
        const api = source('src/lib/api.ts');

        expect(lib).toContain('commands::preview::start_preview_display_web_stream');
        expect(lib).toContain('commands::preview::stop_preview_display_web_stream');
        expect(lib).toContain('commands::preview::get_preview_display_web_stream_status');
        expect(permissions).toContain('"start_preview_display_web_stream"');
        expect(permissions).toContain('"stop_preview_display_web_stream"');
        expect(permissions).toContain('"get_preview_display_web_stream_status"');
        expect(api).toContain("invoke<PreviewWebStreamStatus>('start_preview_display_web_stream'");
        expect(api).toContain("invoke<PreviewWebStreamStatus>('stop_preview_display_web_stream'");
        expect(api).toContain("invoke<PreviewWebStreamStatus>('get_preview_display_web_stream_status'");
    });

    it('adds View menu actions for starting, copying, and stopping the web stream', () => {
        const menu = source('src-tauri/src/menu.rs');
        const frontendMenu = source('src/lib/menu.ts');

        expect(menu).toContain('"preview_display_start_web_stream"');
        expect(menu).toContain('"preview_display_copy_web_stream_url"');
        expect(menu).toContain('"preview_display_stop_web_stream"');
        expect(frontendMenu).toContain("case 'preview_display_start_web_stream'");
        expect(frontendMenu).toContain("case 'preview_display_copy_web_stream_url'");
        expect(frontendMenu).toContain("case 'preview_display_stop_web_stream'");
    });

    it('surfaces active web stream state in menu state and status bar', () => {
        const api = source('src/lib/api.ts');
        const menu = source('src-tauri/src/menu.rs');
        const statusBar = source('src/lib/components/StatusBar.svelte');

        expect(api).toContain('previewDisplayWebStreamActive: boolean');
        expect(menu).toContain('preview_display_web_stream_active');
        expect(statusBar).toContain('$previewDisplayWebStreamStatus.active');
    });
});
