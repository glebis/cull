import { describe, expect, it } from 'vitest';
import { buildExportParams, describeExportScope, type ExportScope } from './export-helpers';

const request = { outputDir: '/out', format: 'jpg', naming: '{index}_{name}' };

function scope(partial: Partial<ExportScope>): ExportScope {
    return { activeCollection: null, activeFolder: null, selectedIds: [], allImageIds: [], ...partial };
}

describe('export selector resolution', () => {
    it('prefers an explicit selection over other scopes', () => {
        const params = buildExportParams(
            scope({ selectedIds: ['a', 'b'], activeCollection: 'c1', activeFolder: '/p' }),
            request,
        );
        expect(params.image_ids).toEqual(['a', 'b']);
        expect(params.collection_id).toBeUndefined();
        expect(params.folder_path).toBeUndefined();
    });

    it('uses the active collection when nothing is selected', () => {
        const params = buildExportParams(scope({ activeCollection: 'c1', activeFolder: '/p' }), request);
        expect(params.collection_id).toBe('c1');
        expect(params.image_ids).toBeUndefined();
    });

    it('uses the active folder and preserves structure by default', () => {
        const params = buildExportParams(scope({ activeFolder: '/photos' }), request);
        expect(params.folder_path).toBe('/photos');
        expect(params.flatten).toBe(false);
    });

    it('falls back to the whole library', () => {
        const params = buildExportParams(scope({ allImageIds: ['x', 'y', 'z'] }), request);
        expect(params.image_ids).toEqual(['x', 'y', 'z']);
    });

    it('carries format and naming through unchanged', () => {
        const params = buildExportParams(scope({ activeCollection: 'c1' }), request);
        expect(params.format).toBe('jpg');
        expect(params.naming).toBe('{index}_{name}');
    });

    it('describes the active scope for UI copy', () => {
        expect(describeExportScope(scope({ selectedIds: ['a'] }))).toEqual({ kind: 'selection', count: 1 });
        expect(describeExportScope(scope({ activeCollection: 'c1' })).kind).toBe('collection');
        expect(describeExportScope(scope({ activeFolder: '/p' })).kind).toBe('folder');
        expect(describeExportScope(scope({ allImageIds: ['a', 'b'] }))).toEqual({ kind: 'all', count: 2 });
    });
});
