import { describe, expect, it } from 'vitest';
import {
    resolveLibraryViewState,
    scopeEmptyCopy,
    type LibraryScopeKind,
} from './library-view-state';

const loadedBase = { loading: false, error: null, loaded: true, imageCount: 0 };

describe('resolveLibraryViewState with scope', () => {
    it('returns empty for an unscoped (whole-library) zero-image view', () => {
        expect(resolveLibraryViewState({ ...loadedBase, scopeKind: 'all' })).toBe('empty');
    });

    it('returns scope-empty when a collection scope has zero images', () => {
        expect(resolveLibraryViewState({ ...loadedBase, scopeKind: 'collection' })).toBe('scope-empty');
    });

    it('returns scope-empty for smart, detected-class, folder, and filtered scopes', () => {
        for (const scopeKind of ['smart', 'detected-class', 'folder', 'filtered'] as LibraryScopeKind[]) {
            expect(resolveLibraryViewState({ ...loadedBase, scopeKind })).toBe('scope-empty');
        }
    });

    it('treats a missing scopeKind as unscoped for backward compatibility', () => {
        expect(resolveLibraryViewState(loadedBase)).toBe('empty');
    });

    it('keeps error and loading precedence over scope-empty', () => {
        expect(resolveLibraryViewState({ ...loadedBase, error: 'boom', scopeKind: 'collection' })).toBe('error');
        expect(resolveLibraryViewState({ ...loadedBase, loading: true, scopeKind: 'collection' })).toBe('loading');
        expect(resolveLibraryViewState({ ...loadedBase, loaded: false, scopeKind: 'collection' })).toBe('loading');
    });

    it('returns loaded when images exist regardless of scope', () => {
        expect(resolveLibraryViewState({ ...loadedBase, imageCount: 3, scopeKind: 'collection' })).toBe('loaded');
    });
});

describe('scopeEmptyCopy', () => {
    it('describes an empty collection without suggesting import', () => {
        const copy = scopeEmptyCopy('collection');
        expect(copy.title).toBe('No images in this collection yet');
        expect(copy.hint.toLowerCase()).toContain('add');
        expect(copy.clearFilters).toBe(false);
    });

    it('offers to clear filters for filter-like scopes', () => {
        expect(scopeEmptyCopy('detected-class').clearFilters).toBe(true);
        expect(scopeEmptyCopy('filtered').clearFilters).toBe(true);
        expect(scopeEmptyCopy('detected-class').title).toBe('No images match these filters');
    });

    it('describes folder and smart-collection scopes specifically', () => {
        expect(scopeEmptyCopy('folder').title).toBe('No images in this folder');
        expect(scopeEmptyCopy('smart').title).toBe('No images match this smart collection');
    });
});
