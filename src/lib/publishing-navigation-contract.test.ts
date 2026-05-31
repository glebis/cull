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

    it('shows publish handoff items as openable, copyable, shareable rows with an in-app QR image', () => {
        const publishView = readProjectFile('src/lib/components/StaticPublishingSettings.svelte');
        const api = readProjectFile('src/lib/api.ts');

        expect(publishView).toContain('Stop preview');
        expect(publishView).toContain('stopServer');
        expect(publishView).toContain('qrImageSrc');
        expect(publishView).toContain('lastResult.qr_svg_data_url');
        expect(publishView).toContain('alt="QR code for target URL"');
        expect(publishView).toContain('copyPublishItem');
        expect(publishView).toContain('sharePublishItem');
        expect(publishView).toContain('openPublishItem');
        expect(publishView).toContain('buildStaticPublishShareItems');
        expect(publishView).toContain("item.kind === 'url'");
        expect(publishView).toContain('Tailscale');
        expect(publishView).toContain('ngrok');
        expect(api).toContain('qr_svg_data_url: string');
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

    it('opens generated image cards in a fullscreen viewer with keyboard and swipe navigation', () => {
        const staticPublishing = readProjectFile('src-tauri/src/commands/static_publishing.rs');

        expect(staticPublishing).toContain('id="image-viewer"');
        expect(staticPublishing).toContain('role="dialog"');
        expect(staticPublishing).toContain('aria-modal="true"');
        expect(staticPublishing).toContain('class="viewer-image"');
        expect(staticPublishing).toContain('viewer-prev');
        expect(staticPublishing).toContain('viewer-next');
        expect(staticPublishing).toContain('viewer-close');
        expect(staticPublishing).toContain('card.addEventListener(\'click\'');
        expect(staticPublishing).toContain('viewerItems.push({');
        expect(staticPublishing).toContain('openViewer(viewerItemIndex, card)');
        expect(staticPublishing).toContain('event.key === \'Escape\'');
        expect(staticPublishing).toContain('event.key === \'ArrowRight\' || event.key === \'l\' || event.key === \'j\'');
        expect(staticPublishing).toContain('event.key === \'ArrowLeft\' || event.key === \'h\' || event.key === \'k\'');
        expect(staticPublishing).toContain('event.key === \'Home\'');
        expect(staticPublishing).toContain('event.key === \'End\'');
        expect(staticPublishing).toContain('viewer.addEventListener(\'touchstart\'');
        expect(staticPublishing).toContain('viewer.addEventListener(\'touchend\'');
        expect(staticPublishing).toContain('lastFocusedCard?.focus()');
    });
});
