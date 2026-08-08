import { beforeEach, describe, expect, it } from 'vitest';
import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSmartCollection,
    importBatchFilter,
    minSizeFilter,
    showRejected,
} from './stores';
import { currentLibraryScope, libraryScopeKey } from './library-scope';

beforeEach(() => {
    activeCollection.set(null);
    activeDetectedClass.set(null);
    activeFolder.set(null);
    activeSmartCollection.set(null);
    importBatchFilter.set(null);
    minSizeFilter.set(0);
    showRejected.set(false);
});

describe('currentLibraryScope', () => {
    it('represents the unfiltered library explicitly', () => {
        const scope = currentLibraryScope();
        expect(scope).toEqual({ type: 'all', include_rejected: false });
        expect(libraryScopeKey(scope)).toBe('all:without-rejected');
    });

    it('captures folder dimensions and rejected visibility', () => {
        activeFolder.set('/Photos/2026_Trips');
        minSizeFilter.set(1024);
        showRejected.set(true);

        const scope = currentLibraryScope();
        expect(scope).toEqual({
            type: 'folder',
            path: '/Photos/2026_Trips',
            min_size: 1024,
            include_rejected: true,
        });
        expect(libraryScopeKey(scope)).toBe('folder:/Photos/2026_Trips:1024:with-rejected');
    });

    it('uses the same precedence as library browsing', () => {
        activeFolder.set('/Photos');
        activeDetectedClass.set('person');
        activeCollection.set('collection-1');
        activeSmartCollection.set({
            id: 'smart-1',
            name: 'Recent',
            description: null,
            collection_type: 'smart',
            filter_json: '{"type":"rule","field":"rating","op":"gte","value":4}',
            nl_query: null,
            is_preset: false,
            sort_order: 0,
            created_at: '2026-01-01',
            image_count: 2,
        });
        importBatchFilter.set('batch-1');

        expect(currentLibraryScope()).toEqual({
            type: 'import_batch',
            batch_id: 'batch-1',
            include_rejected: false,
        });

        importBatchFilter.set(null);
        expect(currentLibraryScope()).toMatchObject({ type: 'smart', id: 'smart-1' });
        activeSmartCollection.set(null);
        expect(currentLibraryScope()).toEqual({
            type: 'collection',
            id: 'collection-1',
            include_rejected: false,
        });
        activeCollection.set(null);
        expect(currentLibraryScope()).toEqual({
            type: 'detected_class',
            class_name: 'person',
            include_rejected: false,
        });
    });
});
