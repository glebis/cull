import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function readProjectFile(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('static publishing navigation contract', () => {
    it('wires Publish as a gated native View menu item before Export', () => {
        const menu = readProjectFile('src-tauri/src/menu.rs');
        const frontendMenu = readProjectFile('src/lib/menu.ts');

        expect(menu).toContain('VIEW_PUBLISH_ID');
        expect(menu).toContain('sync_static_publishing_menu_item');
        expect(menu.indexOf('(VIEW_PUBLISH_ID, "publish")')).toBeLessThan(menu.indexOf('(VIEW_EXPORT_ID, "export")'));
        expect(menu).toContain('insert_menu_item_before(app, VIEW_EXPORT_ID');
        expect(menu).toContain('"view_publish"');
        expect(frontendMenu).toContain("case 'view_publish'");
        expect(frontendMenu).toContain('staticPublishingEnabled');
    });

    it('keeps the publishing workflow out of Settings content', () => {
        const settings = readProjectFile('src/lib/components/McpSettings.svelte');

        expect(settings).toContain('module_static_publishing');
        expect(settings).not.toContain("activeSettingsTab === 'static-publishing'");
        expect(settings).not.toContain('<StaticPublishingSettings');
    });

    it('uses publishing language for the generated site result panel', () => {
        const publishView = readProjectFile('src/lib/components/StaticPublishingSettings.svelte');

        expect(publishView).toContain('Last Package');
        expect(publishView).not.toContain('Last Export');
    });
});
