import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

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
