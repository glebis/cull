import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Preview Display UI contract', () => {
    it('routes preview display windows to the dedicated component before main app initialization', () => {
        const page = source('src/routes/+page.svelte');

        expect(page).toContain("import PreviewDisplay from '$lib/components/PreviewDisplay.svelte'");
        expect(page).toContain('isPreviewDisplayRoute');
        expect(page).toContain('const previewDisplayWindow');
        expect(page).toMatch(/if \(previewDisplayWindow\) return/);
        expect(page).toContain('<PreviewDisplay />');
    });

    it('keeps the PreviewDisplay component read-only and subscribed to canonical state', () => {
        const component = source('src/lib/components/PreviewDisplay.svelte');

        expect(component).toContain("listen<PreviewState>('preview:state-changed'");
        expect(component).toContain('getPreviewState');
        expect(component).toContain('getImagesByIds');
        expect(component).toContain('listImageTags');
        expect(component).toContain('getGenerationRun');
        expect(component).toContain('getImageHistogram');
        expect(component).toContain('convertFileSrc');
        expect(component).toContain('previewDisplayImageSourcePath');
        expect(component).not.toContain('trashImages');
        expect(component).not.toContain('setRating');
        expect(component).not.toContain('setDecision');
        expect(component).not.toContain('cropImage');
        expect(component).not.toContain('ContextMenu');
    });

    it('syncs main-window focus changes into PreviewState through the shared API', () => {
        const page = source('src/routes/+page.svelte');

        expect(page).toContain('syncFocusedImageToPreviewDisplay');
        expect(page).toContain('nextPreviewFocusPayload');
        expect(page).toContain('updatePreviewState');
        expect(page).toContain('$focusedImage');
    });

    it('renders bounded info rail fields without overlapping the image', () => {
        const component = source('src/lib/components/PreviewDisplay.svelte');

        expect(component).toContain("data-side={previewState?.overlay.railSide");
        expect(component).toContain("data-width={previewState?.overlay.railWidth");
        expect(component).toContain("data-text={previewState?.overlay.railTextSize");
        expect(component).toContain('prompt-preview');
        expect(component).toContain('tag-list');
        expect(component).toContain('histogram-panel');
        expect(component).toContain('line-clamp');
        expect(component).toContain('max-height');
    });
});
