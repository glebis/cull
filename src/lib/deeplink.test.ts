import { describe, it, expect } from 'vitest';
import { parseDeepLinkUrl, inferViewFromAction } from './deeplink';

describe('parseDeepLinkUrl', () => {
    it('parses view mode', () => {
        const params = parseDeepLinkUrl('cull://open?view=loupe');
        expect(params.view).toBe('loupe');
    });

    it('parses folder path', () => {
        const params = parseDeepLinkUrl('cull://open?folder=/Users/test/Photos');
        expect(params.folder).toBe('/Users/test/Photos');
    });

    it('parses single path', () => {
        const params = parseDeepLinkUrl('cull://open?path=/Users/test/img.jpg');
        expect(params.path).toBe('/Users/test/img.jpg');
    });

    it('parses multiple paths', () => {
        const params = parseDeepLinkUrl('cull://open?paths=/a.jpg,/b.png,/c.tiff');
        expect(params.paths).toEqual(['/a.jpg', '/b.png', '/c.tiff']);
    });

    it('parses numeric params', () => {
        const params = parseDeepLinkUrl('cull://open?size=120&zoom=200&focus=5&gap=4');
        expect(params.size).toBe(120);
        expect(params.zoom).toBe(200);
        expect(params.focus).toBe(5);
        expect(params.gap).toBe(4);
    });

    it('parses image_id focus params', () => {
        const params = parseDeepLinkUrl('cull://loupe?image_id=img-123');
        expect(params.view).toBe('loupe');
        expect(params.image_id).toBe('img-123');
    });

    it('parses fullscreen flag', () => {
        const params = parseDeepLinkUrl('cull://open?fullscreen=true');
        expect(params.fullscreen).toBe(true);
    });

    it('returns empty for invalid URL', () => {
        const params = parseDeepLinkUrl('not a url at all %%%');
        expect(params).toEqual({});
    });

    it('infers view from hostname action', () => {
        const params = parseDeepLinkUrl('cull://grid');
        expect(params.view).toBe('grid');
    });

    it('handles combined params', () => {
        const params = parseDeepLinkUrl('cull://loupe?folder=/test&size=200&zoom=150');
        expect(params.view).toBe('loupe');
        expect(params.folder).toBe('/test');
        expect(params.size).toBe(200);
        expect(params.zoom).toBe(150);
    });

    it('returns null for missing optional params', () => {
        const params = parseDeepLinkUrl('cull://open');
        expect(params.path).toBeNull();
        expect(params.folder).toBeNull();
        expect(params.size).toBeNull();
        expect(params.zoom).toBeNull();
        expect(params.focus).toBeNull();
        expect(params.gap).toBeNull();
    });

    it('returns NaN for non-numeric size', () => {
        const params = parseDeepLinkUrl('cull://open?size=abc');
        expect(params.size).toBeNaN();
    });

    it('parseInt truncates trailing text (size=12px → 12)', () => {
        const params = parseDeepLinkUrl('cull://open?size=12px');
        expect(params.size).toBe(12);
    });

    it('handles paths with trailing comma', () => {
        const params = parseDeepLinkUrl('cull://open?paths=/a.jpg,/b.png,');
        expect(params.paths).toEqual(['/a.jpg', '/b.png', '']);
    });

    it('fullscreen=false is not truthy', () => {
        const params = parseDeepLinkUrl('cull://open?fullscreen=false');
        expect(params.fullscreen).toBe(false);
    });

    it('fullscreen missing defaults to false', () => {
        const params = parseDeepLinkUrl('cull://open');
        expect(params.fullscreen).toBe(false);
    });

    it('accepts all VALID_VIEWS via ?view= param', () => {
        for (const view of ['grid', 'compare', 'loupe', 'canvas', 'lineage', 'embeddings', 'export']) {
            const params = parseDeepLinkUrl(`cull://open?view=${view}`);
            expect(params.view).toBe(view);
        }
    });
});

describe('inferViewFromAction', () => {
    it('returns valid view modes for deep-link aliases', () => {
        expect(inferViewFromAction('grid')).toBe('grid');
        expect(inferViewFromAction('loupe')).toBe('loupe');
        expect(inferViewFromAction('compare')).toBe('compare');
    });

    it('returns null for valid views that are not deep-link aliases', () => {
        expect(inferViewFromAction('canvas')).toBeNull();
        expect(inferViewFromAction('lineage')).toBeNull();
        expect(inferViewFromAction('embeddings')).toBeNull();
        expect(inferViewFromAction('export')).toBeNull();
    });

    it('returns null for unknown actions', () => {
        expect(inferViewFromAction('open')).toBeNull();
        expect(inferViewFromAction('import')).toBeNull();
        expect(inferViewFromAction('')).toBeNull();
    });
});
