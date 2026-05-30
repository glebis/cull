import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSmartCollection,
    viewMode,
} from './stores';
import { applyClipboardMonitorCollection } from './clipboard-monitor';

vi.mock('./image-loading', () => ({
    loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined),
}));

describe('clipboard monitor frontend helpers', () => {
    beforeEach(() => {
        activeCollection.set(null);
        activeFolder.set('/old');
        activeSmartCollection.set({
            id: 'smart',
            name: 'Smart',
            description: null,
            collection_type: 'smart',
            filter_json: '{}',
            nl_query: null,
            is_preset: false,
            sort_order: 0,
            created_at: 'now',
            image_count: 0,
        });
        activeDetectedClass.set('person');
        viewMode.set('loupe');
    });

    it('focuses monitor collection in grid and clears other scopes', async () => {
        await applyClipboardMonitorCollection('col_clip');

        expect(get(activeCollection)).toBe('col_clip');
        expect(get(activeFolder)).toBeNull();
        expect(get(activeSmartCollection)).toBeNull();
        expect(get(activeDetectedClass)).toBeNull();
        expect(get(viewMode)).toBe('grid');
    });
});
