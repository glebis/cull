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

        expect(publishView).toContain('Latest package');
        expect(publishView).not.toContain('Last Export');
    });

    it('keeps the Publish view organized as an accessible two-to-three column workflow', () => {
        const publishView = readProjectFile('src/lib/components/StaticPublishingSettings.svelte');

        expect(publishView).toContain('class="publish-grid"');
        expect(publishView).toContain('grid-template-columns: repeat(3, minmax(0, 1fr))');
        expect(publishView).toContain('grid-template-columns: repeat(2, minmax(0, 1fr))');
        expect(publishView).toContain('aria-live="polite"');
        expect(publishView).toContain('aria-pressed={indexable}');
        expect(publishView).toContain('Search visibility');
        expect(publishView).toContain('Copy agent notes');
        expect(publishView).not.toContain('Search engines');
        expect(publishView).not.toContain('Build Static Site');
    });

    it('keeps the generated static review site readable and accessible', () => {
        const staticPublishing = readProjectFile('src-tauri/src/commands/static_publishing.rs');

        expect(staticPublishing).toContain('class="review-layout"');
        expect(staticPublishing).toContain('class="review-aside"');
        expect(staticPublishing).toContain('grid-template-columns: repeat(3, minmax(0, 1fr))');
        expect(staticPublishing).toContain('grid-template-columns: repeat(2, minmax(0, 1fr))');
        expect(staticPublishing).toContain('aria-live="polite"');
        expect(staticPublishing).toContain("toLocaleString('en-GB'");
        expect(staticPublishing).toContain('img.alt = imageName');
        expect(staticPublishing).toContain('rel="icon" href="qr.svg"');
        expect(staticPublishing).toContain('Share link');
    });
});
