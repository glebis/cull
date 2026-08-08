import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

function source(path: string): string {
    return readFileSync(path, 'utf8');
}

describe('shared destructive-action safety contract', () => {
    it('routes context, native-menu, palette, and keyboard Trash through one request', () => {
        const contextMenu = source('src/lib/components/ContextMenu.svelte');
        const menu = source('src/lib/menu.ts');
        const palette = source('src/lib/command-palette.ts');
        const keys = source('src/lib/keys.ts');
        const page = source('src/routes/+page.svelte');

        expect(contextMenu).toContain('requestTrashImages(targetIds)');
        expect(menu).toContain('requestTrashImages(ids)');
        expect(palette).toContain('run: () => requestTrashImages()');
        expect(keys).toContain('requestTrashImages()');
        expect(page).toContain('window.addEventListener(TRASH_IMAGES_REQUESTED_EVENT, handleTrashRequest)');
        expect(page).toContain('await trashImagesDetailed(ids)');
        expect(contextMenu).not.toContain('trashImages(');
        expect(menu).not.toContain('trashImages(');
    });

    it('keeps permanent deletion on a distinct irreversible confirmation path', () => {
        const page = source('src/routes/+page.svelte');

        expect(page).toContain('Permanently delete "${name}"? This cannot be undone.');
        expect(page).toContain('await deleteImagesPermanently([img.image.id])');
        expect(page).toContain("window.addEventListener('delete-focused-image', handlePermanentDelete)");
    });
});
