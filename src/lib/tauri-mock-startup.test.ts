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

    it('matches the detailed Trash response consumed by the app and supports undo', async () => {
        const before = await invoke<Array<{ image: { id: string }; path: string }>>('list_images');
        const image = before[0];

        const result = await invoke<{
            requested: number;
            succeeded: number;
            failed: number;
            results: Array<{ image_id: string; path: string | null; status: string; error: string | null }>;
        }>('trash_images_detailed', { imageIds: [image.image.id] });

        expect(result).toEqual({
            requested: 1,
            succeeded: 1,
            failed: 0,
            results: [{ image_id: image.image.id, path: image.path, status: 'trashed', error: null }],
        });
        await expect(invoke<Array<{ image: { id: string } }>>('list_images'))
            .resolves.not.toContainEqual(expect.objectContaining({ image: { id: image.image.id } }));

        await invoke('undo');
        await expect(invoke<Array<{ image: { id: string } }>>('list_images'))
            .resolves.toContainEqual(expect.objectContaining({
                image: expect.objectContaining({ id: image.image.id }),
            }));
    });

    it('returns sessions with the current folder_path contract', async () => {
        const sessions = await invoke<Array<{ folder_path: string }>>('list_sessions');
        expect(sessions[0].folder_path).toBe('/mock/session');
    });

    it('implements scoped embedding pagination used by the explorer', async () => {
        const ids = await invoke<{ ids: string[]; total: number; has_more: boolean }>('list_scoped_image_ids', {
            scope: { type: 'all', include_rejected: false }, limit: 2, offset: 0,
        });
        expect(ids).toMatchObject({ ids: ['img-0', 'img-1'], total: 20, has_more: true });

        const embeddings = await invoke<{ ids: string[]; dims: number; total: number }>('get_scoped_embedding_page', {
            scope: { type: 'all', include_rejected: false }, model: 'dinov2-vits14', limit: 2, offset: 2,
        });
        expect(embeddings).toMatchObject({
            ids: ['img-2', 'img-3'], dims: 384, total: 8, offset: 2, limit: 2, has_more: true,
        });
    });
});
