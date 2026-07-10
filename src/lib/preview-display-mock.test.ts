import { describe, expect, it } from 'vitest';
import { emit, invoke, listen } from './tauri-mock';
import { DEFAULT_PREVIEW_OVERLAY } from './preview-display';

describe('Preview Display E2E mock', () => {
    it('exposes the event listen and emit API used by browser tests', async () => {
        const payloads: Array<{ ok: boolean }> = [];
        const unlisten = await listen<{ ok: boolean }>('mock:event', event => {
            payloads.push(event.payload);
        });

        await emit('mock:event', { ok: true });
        unlisten();
        await emit('mock:event', { ok: false });

        expect(payloads).toEqual([{ ok: true }]);
    });

    it('supports the native preview window and shared preview state commands', async () => {
        await expect(invoke<string>('open_preview_display')).resolves.toBe('preview-display');
        await expect(invoke('place_preview_display', { monitorId: 'sidecar-ipad', fullscreen: true }))
            .resolves.toBe('preview-display');

        const monitors = await invoke<Array<{ id: string; primary: boolean }>>('list_preview_display_monitors');
        expect(monitors.some((monitor) => monitor.primary)).toBe(true);

        const updated = await invoke<any>('update_preview_state', {
            imageId: 'img-2',
            imageIds: ['img-2', 'img-3'],
            displayMode: 'client_review',
            layout: 'compare',
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
            image_ids: ['img-2', 'img-3'],
            display_mode: 'client_review',
            layout: 'compare',
            frozen: true,
            blanked: false,
        });

        await expect(invoke('get_preview_state')).resolves.toMatchObject({
            image_id: 'img-2',
            image_ids: ['img-2', 'img-3'],
            display_mode: 'client_review',
            layout: 'compare',
        });
    });
});
