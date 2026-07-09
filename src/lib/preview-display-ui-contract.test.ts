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
        const grid = source('src/lib/components/Grid.svelte');
        const keys = source('src/lib/keys.ts');

        expect(page).toContain('syncFocusedImageToPreviewDisplay');
        expect(page).toContain('nextPreviewFocusPayload');
        expect(page).toContain('updatePreviewState');
        expect(page).toContain('$focusedImage');
        expect(grid).toContain('focusedIndex.set(index)');
        expect(keys).toContain('moveLoupeFocus');
        expect(keys).toContain('focusedIndex.update');
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

    it('keeps the Preview Display header draggable like a macOS window titlebar', () => {
        const component = source('src/lib/components/PreviewDisplay.svelte');

        expect(component).toContain('class="preview-header"');
        expect(component).toContain('class:hidden={!headerVisible}');
        expect(component).toContain('data-tauri-drag-region="deep"');
        expect(component).toContain('<span class="preview-title" data-tauri-drag-region="deep">Preview Display</span>');
        expect(component).toContain('aria-label="Preview Display window header"');
        expect(component).toContain('setTimeout(() => {');
        expect(component).toContain('}, 1500)');
        expect(component).toContain('position: absolute');
        expect(component).toContain('padding-left: var(--macos-window-controls-width)');
        expect(component).toContain('class="preview-stage"');
    });

    it('cycles Preview Display presets from Ctrl+Tab inside the preview window', () => {
        const component = source('src/lib/components/PreviewDisplay.svelte');

        expect(component).toContain('isPreviewDisplayPresetCycleShortcut');
        expect(component).toContain('nextPreviewDisplayPresetMode');
        expect(component).toContain('updatePreviewState');
        expect(component).toContain('setAppSetting');
        expect(component).toContain('<svelte:window onkeydown={handlePreviewKeydown} />');
        expect(component).toContain('event.preventDefault()');
    });

    it('keeps Preview Display images fit by default but zoomable and pannable', () => {
        const component = source('src/lib/components/PreviewDisplay.svelte');

        expect(component).toContain('clampPreviewDisplayZoom');
        expect(component).toContain('previewDisplayNormalizedFocus');
        expect(component).toContain('previewDisplayPanForNormalizedFocus');
        expect(component).toContain('bind:this={stageEl}');
        expect(component).toContain('onwheel={handlePreviewWheel}');
        expect(component).toContain('onpointerdown={handlePreviewPointerDown}');
        expect(component).toContain('resetPreviewZoomToFit');
        expect(component).toContain('max-width: 100%');
        expect(component).toContain('max-height: 100%');
        expect(component).toContain('transform-origin: center center');
    });

    it('shows blanking text for only the first three seconds after blanking', () => {
        const component = source('src/lib/components/PreviewDisplay.svelte');

        expect(component).toContain('showBlankMessageTemporarily');
        expect(component).toContain('blankMessageVisible = true');
        expect(component).toContain('}, 3000)');
        expect(component).toContain('Preview is Blanked');
        expect(component).not.toContain('Preview blanked</div>');
    });
});
