import { describe, expect, it } from 'vitest';
import { invoke } from './tauri-mock';
import { DEFAULT_PREVIEW_OVERLAY } from './preview-display';

describe('Preview Display E2E mock', () => {
    it('supports the native preview window and shared preview state commands', async () => {
        await expect(invoke<string>('open_preview_display')).resolves.toBe('preview-display');
        await expect(invoke('place_preview_display', { monitorId: 'sidecar-ipad', fullscreen: true }))
            .resolves.toBe('preview-display');

        const monitors = await invoke<Array<{ id: string; primary: boolean }>>('list_preview_display_monitors');
        expect(monitors.some((monitor) => monitor.primary)).toBe(true);

        const updated = await invoke<any>('update_preview_state', {
            imageId: 'img-2',
            displayMode: 'client_review',
            overlay: {
                ...DEFAULT_PREVIEW_OVERLAY,
                showFilename: true,
                showRating: true,
                showDecision: true,
                showMetadataRail: false,
            },
            frozen: true,
            blanked: false,
        });
        expect(updated).toMatchObject({
            image_id: 'img-2',
            display_mode: 'client_review',
            frozen: true,
            blanked: false,
        });

        await expect(invoke('get_preview_state')).resolves.toMatchObject({
            image_id: 'img-2',
            display_mode: 'client_review',
        });
    });
});
