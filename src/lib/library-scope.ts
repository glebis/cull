import { derived, get } from 'svelte/store';
import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeReferencedFolder,
    activeSmartCollection,
    importBatchFilter,
    minSizeFilter,
    showRejected,
} from './stores';

export type LibraryScope =
    | { type: 'import_batch'; batch_id: string; include_rejected: boolean }
    | { type: 'smart'; id: string; filter_json: string; include_rejected: boolean }
    | { type: 'collection'; id: string; include_rejected: boolean }
    | { type: 'detected_class'; class_name: string; include_rejected: boolean }
    | { type: 'folder'; path: string; min_size: number; include_rejected: boolean }
    | { type: 'referenced_folder'; source_id: string; relative_path: string; recursive: boolean; include_rejected: boolean }
    | { type: 'filtered'; min_size: number; include_rejected: boolean }
    | { type: 'all'; include_rejected: boolean };

function buildLibraryScope(
    batch_id: string | null,
    smart: { id: string; filter_json: string | null } | null,
    collection: string | null,
    class_name: string | null,
    path: string | null,
    referenced: { source_id: string; relative_path: string; recursive: boolean } | null,
    min_size: number,
    include_rejected: boolean,
): LibraryScope {
    if (batch_id) return { type: 'import_batch', batch_id, include_rejected };
    if (smart?.filter_json) {
        return {
            type: 'smart',
            id: smart.id,
            filter_json: smart.filter_json,
            include_rejected,
        };
    }

    if (collection) return { type: 'collection', id: collection, include_rejected };
    if (class_name) return { type: 'detected_class', class_name, include_rejected };
    if (path) return { type: 'folder', path, min_size, include_rejected };
    if (referenced) return {
        type: 'referenced_folder',
        source_id: referenced.source_id,
        relative_path: referenced.relative_path,
        recursive: referenced.recursive,
        include_rejected,
    };
    if (min_size > 0) return { type: 'filtered', min_size, include_rejected };
    return { type: 'all', include_rejected };
}

export const libraryScope = derived(
    [
        importBatchFilter,
        activeSmartCollection,
        activeCollection,
        activeDetectedClass,
        activeFolder,
        activeReferencedFolder,
        minSizeFilter,
        showRejected,
    ],
    ([$batch, $smart, $collection, $detectedClass, $folder, $referenced, $minSize, $showRejected]) =>
        buildLibraryScope(
            $batch,
            $smart,
            $collection,
            $detectedClass,
            $folder,
            $referenced,
            $minSize,
            $showRejected,
        ),
);

export function currentLibraryScope(): LibraryScope {
    return get(libraryScope);
}

export function libraryScopeKey(scope: LibraryScope): string {
    const visibility = scope.include_rejected ? 'with-rejected' : 'without-rejected';
    switch (scope.type) {
        case 'import_batch': return `import-batch:${scope.batch_id}:${visibility}`;
        case 'smart': return `smart:${scope.id}:${scope.filter_json}:${visibility}`;
        case 'collection': return `collection:${scope.id}:${visibility}`;
        case 'detected_class': return `detected-class:${scope.class_name}:${visibility}`;
        case 'folder': return `folder:${scope.path}:${scope.min_size}:${visibility}`;
        case 'referenced_folder': return `referenced:${scope.source_id}:${scope.relative_path}:${scope.recursive}:${visibility}`;
        case 'filtered': return `filtered:${scope.min_size}:${visibility}`;
        case 'all': return `all:${visibility}`;
    }
}
