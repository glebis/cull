import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

function readProjectFile(path: string): string {
    return readFileSync(resolve(process.cwd(), path), 'utf8');
}

describe('GitHub update system contract', () => {
    it('builds updater artifacts and points update checks at the GitHub release feed', () => {
        const config = JSON.parse(readProjectFile('src-tauri/tauri.conf.json'));

        expect(config.bundle.createUpdaterArtifacts).toBe(true);
        expect(config.plugins.updater.endpoints).toContain(
            'https://github.com/glebis/cull/releases/latest/download/latest.json'
        );
        expect(config.plugins.updater.pubkey).toEqual(expect.any(String));
    });

    it('allows the frontend to check, install, and restart through Tauri permissions', () => {
        const defaultCapability = JSON.parse(readProjectFile('src-tauri/capabilities/default.json'));

        expect(defaultCapability.permissions).toContain('updater:default');
        expect(defaultCapability.permissions).toContain('process:default');
    });

    it('exposes Check for Update from the Cull menu and forwards it to the webview', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');

        expect(menuSource).toContain('"check_update"');
        expect(menuSource).toContain('"Check for Update..."');
        expect(menuSource).toMatch(/"check_update"[\s\S]*app\.emit\("menu-action", id\)/);
    });

    it('exposes the default-on auto update toggle in Settings', () => {
        const settingsSource = readProjectFile('src/lib/components/GeneralSettings.svelte');

        expect(settingsSource).toContain("getAppSetting('auto_update_enabled')");
        expect(settingsSource).toContain("toggle('auto_update_enabled'");
        expect(settingsSource).toContain('Auto update');
    });
});
