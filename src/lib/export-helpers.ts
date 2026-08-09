import type { ExportImagesParams } from './api';

export interface ExportScope {
    // An explicit smart-collection export is an action target, so it takes
    // precedence over any incidental grid selection.
    smartCollectionId: string | null;
    smartCollectionImageIds: string[];
    activeCollection: string | null;
    activeFolder: string | null;
    selectedIds: string[];
    // Fallback when no collection/folder/selection scopes the view.
    allImageIds: string[];
}

export interface ExportRequest {
    outputDir: string;
    format: string;
    naming: string;
    flatten?: boolean;
}

export type ExportScopeKind = 'smart' | 'selection' | 'collection' | 'folder' | 'all';

export function describeExportScope(scope: ExportScope): { kind: ExportScopeKind; count: number } {
    if (scope.smartCollectionId) return { kind: 'smart', count: scope.smartCollectionImageIds.length };
    if (scope.selectedIds.length > 0) return { kind: 'selection', count: scope.selectedIds.length };
    if (scope.activeCollection) return { kind: 'collection', count: 0 };
    if (scope.activeFolder) return { kind: 'folder', count: 0 };
    return { kind: 'all', count: scope.allImageIds.length };
}

// Build export params with exactly one selector, honoring the most specific
// scope available: an explicit selection wins, then the active collection,
// then the active folder, then the whole library.
export function buildExportParams(scope: ExportScope, request: ExportRequest): ExportImagesParams {
    const base = {
        output_dir: request.outputDir,
        format: request.format,
        naming: request.naming,
        flatten: request.flatten ?? true,
    };

    if (scope.smartCollectionId) {
        return { ...base, image_ids: scope.smartCollectionImageIds };
    }
    if (scope.selectedIds.length > 0) {
        return { ...base, image_ids: scope.selectedIds };
    }
    if (scope.activeCollection) {
        return { ...base, collection_id: scope.activeCollection };
    }
    if (scope.activeFolder) {
        return { ...base, folder_path: scope.activeFolder, flatten: request.flatten ?? false };
    }
    return { ...base, image_ids: scope.allImageIds };
}
