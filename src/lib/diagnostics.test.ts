import { describe, expect, it } from 'vitest';
import {
    buildAssetLoadEvent,
    pathBasename,
    pathFingerprint,
    type DiagnosticImage,
} from './diagnostics';

const image: DiagnosticImage = {
    image: {
        id: 'img-1',
        format: 'png',
    },
    path: '/Users/test/Pictures/full/ig_abc.png',
    thumbnail_path: '/Users/test/Library/Application Support/com.glebkalinin.cull/thumbs/img-1.jpg',
};

describe('pathBasename', () => {
    it('returns only the filename for POSIX and Windows-style paths', () => {
        expect(pathBasename('/Users/test/Pictures/ig_abc.png')).toBe('ig_abc.png');
        expect(pathBasename('C:\\Users\\test\\Pictures\\ig_abc.png')).toBe('ig_abc.png');
    });
});

describe('pathFingerprint', () => {
    it('is deterministic without exposing the path text', () => {
        const first = pathFingerprint('/Users/test/Pictures/ig_abc.png');
        const second = pathFingerprint('/Users/test/Pictures/ig_abc.png');

        expect(first).toBe(second);
        expect(first).not.toContain('/Users/test');
    });
});

describe('buildAssetLoadEvent', () => {
    it('builds a privacy-safe loupe source load failure event', () => {
        const event = buildAssetLoadEvent({
            view: 'loupe',
            image,
            assetKind: 'source',
            errorKind: 'img_onerror',
            fallbackUsed: true,
            fallbackSucceeded: null,
            phase: 'source',
        });

        expect(event).toMatchObject({
            view: 'loupe',
            imageId: 'img-1',
            assetKind: 'source',
            imageFormat: 'png',
            fallbackUsed: true,
            fallbackSucceeded: null,
            pathBasename: 'ig_abc.png',
            errorKind: 'img_onerror',
        });
        expect(event.pathHash).toBeTruthy();
        expect(JSON.stringify(event)).not.toContain('/Users/test');
        expect(JSON.stringify(event)).not.toContain('Application Support');
    });

    it('uses the thumbnail path when the thumbnail asset failed', () => {
        const event = buildAssetLoadEvent({
            view: 'thumbnail',
            image,
            assetKind: 'thumbnail',
            errorKind: 'img_onerror',
            fallbackUsed: false,
            fallbackSucceeded: false,
            phase: 'thumbnail',
        });

        expect(event.pathBasename).toBe('img-1.jpg');
    });
});
