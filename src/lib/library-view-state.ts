/**
 * Discriminates what the main library view should render, so a backend/init
 * failure is never mistaken for a genuinely empty library.
 */
export type LibraryViewState = 'loading' | 'error' | 'empty' | 'scope-empty' | 'loaded';

/** Mirrors the ImageScope discriminants in image-loading.ts. */
export type LibraryScopeKind = 'all' | 'smart' | 'collection' | 'detected-class' | 'folder' | 'filtered';

export interface LibraryViewStateInput {
    /** A load is currently in flight. */
    loading: boolean;
    /** The last load attempt failed with this message. */
    error: string | null;
    /** At least one load has completed successfully. */
    loaded: boolean;
    /** Number of images currently in the view. */
    imageCount: number;
    /** What the current view is scoped to; anything but 'all' means the
     *  library itself may still have images even when this view has zero. */
    scopeKind?: LibraryScopeKind;
}

export function resolveLibraryViewState(input: LibraryViewStateInput): LibraryViewState {
    if (input.imageCount > 0) return 'loaded';
    if (input.error) return 'error';
    // Before the first query has ever succeeded, never claim the library is
    // empty — that is what made init failures look like a healthy 0-image
    // library.
    if (input.loading || !input.loaded) return 'loading';
    // A zero-result collection/folder/filter must not claim the whole library
    // is empty (and must not push the first-run Import CTA).
    if (input.scopeKind && input.scopeKind !== 'all') return 'scope-empty';
    return 'empty';
}

export interface ScopeEmptyCopy {
    title: string;
    hint: string;
    /** Whether a "Clear filters" action makes sense for this scope. */
    clearFilters: boolean;
}

export function scopeEmptyCopy(scopeKind: LibraryScopeKind): ScopeEmptyCopy {
    switch (scopeKind) {
        case 'collection':
            return {
                title: 'No images in this collection yet',
                hint: 'Add images from the grid via right-click → Add to Collection',
                clearFilters: false,
            };
        case 'smart':
            return {
                title: 'No images match this smart collection',
                hint: 'Edit its rules or add matching images to the library',
                clearFilters: false,
            };
        case 'folder':
            return {
                title: 'No images in this folder',
                hint: 'Pick another folder in the sidebar or rescan sources',
                clearFilters: false,
            };
        case 'detected-class':
        case 'filtered':
            return {
                title: 'No images match these filters',
                hint: 'Loosen or clear the filters to see your images again',
                clearFilters: true,
            };
        default:
            return {
                title: 'No images here',
                hint: 'Pick a folder or collection in the sidebar',
                clearFilters: false,
            };
    }
}

export const LIBRARY_DB_PATH = '~/Library/Application Support/com.glebkalinin.cull/cull.db';

export function formatLibraryLoadError(e: unknown): string {
    const detail = e instanceof Error ? e.message : String(e ?? 'unknown error');
    return `Could not load the library from ${LIBRARY_DB_PATH}: ${detail}`;
}
