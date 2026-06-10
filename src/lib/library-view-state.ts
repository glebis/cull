/**
 * Discriminates what the main library view should render, so a backend/init
 * failure is never mistaken for a genuinely empty library.
 */
export type LibraryViewState = 'loading' | 'error' | 'empty' | 'loaded';

export interface LibraryViewStateInput {
    /** A load is currently in flight. */
    loading: boolean;
    /** The last load attempt failed with this message. */
    error: string | null;
    /** At least one load has completed successfully. */
    loaded: boolean;
    /** Number of images currently in the view. */
    imageCount: number;
}

export function resolveLibraryViewState(input: LibraryViewStateInput): LibraryViewState {
    if (input.imageCount > 0) return 'loaded';
    if (input.error) return 'error';
    // Before the first query has ever succeeded, never claim the library is
    // empty — that is what made init failures look like a healthy 0-image
    // library.
    if (input.loading || !input.loaded) return 'loading';
    return 'empty';
}

export const LIBRARY_DB_PATH = '~/Library/Application Support/com.glebkalinin.cull/cull.db';

export function formatLibraryLoadError(e: unknown): string {
    const detail = e instanceof Error ? e.message : String(e ?? 'unknown error');
    return `Could not load the library from ${LIBRARY_DB_PATH}: ${detail}`;
}
