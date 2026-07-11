import { describe, expect, it } from 'vitest';
import { invoke } from './tauri-mock';

describe('E2E mock startup contract', () => {
    it('returns collections for every array-valued command used during app startup', async () => {
        await expect(invoke('drain_pending_open_params')).resolves.toEqual([]);
        await expect(invoke('list_action_proposals', { status: 'pending', limit: 20 })).resolves.toEqual([]);
        await expect(invoke('list_agent_selection_presets')).resolves.toEqual([]);
    });

    it('starts the focused smoke fixture with neutral curation state', async () => {
        const images = await invoke<Array<{
            image: { id: string };
            selection: { star_rating: number | null; decision: string } | null;
        }>>('list_images');

        expect(images[0]).toMatchObject({
            image: { id: 'img-0' },
            selection: null,
        });
    });

    it('implements browser media reads and teardown commands without fallback warnings', async () => {
        const payload = await invoke<{ bytes: number[]; mime_type: string }>('get_image_file_bytes', {
            imageId: 'img-0',
        });

        expect(payload.mime_type).toBe('image/svg+xml');
        expect(payload.bytes.length).toBeGreaterThan(0);
        await expect(invoke('stop_dictation')).resolves.toBeUndefined();
    });
});
