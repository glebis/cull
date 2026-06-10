import { describe, expect, it } from 'vitest';
import {
    buildHtmlToImageOptions,
    formatExportError,
    imageSourceForExportAsset,
} from './export-renderer';

describe('export renderer helpers', () => {
    it('formats image load error events instead of leaking [object Event]', () => {
        const image = { src: 'asset://asset.localhost/missing-preview.png' } as HTMLImageElement;
        const event = new Event('error');
        Object.defineProperty(event, 'target', { value: image });

        expect(formatExportError(event)).toBe(
            'Render image failed while loading asset://asset.localhost/missing-preview.png'
        );
    });

    it('summarizes generated SVG load errors instead of showing a data URL', () => {
        const image = { src: 'data:image/svg+xml;charset=utf-8,<svg>...</svg>' } as HTMLImageElement;
        const event = new Event('error');
        Object.defineProperty(event, 'target', { value: image });

        expect(formatExportError(event, 'Rendering slide 1')).toBe(
            'Rendering slide 1: Render image failed while loading generated SVG export snapshot'
        );
    });

    it('preserves real Error messages with the failing slide label', () => {
        expect(formatExportError(new Error('canvas is too large'), 'Slide 2')).toBe(
            'Slide 2: canvas is too large'
        );
    });

    it('removes preview-only transforms before rasterizing a slide', () => {
        expect(buildHtmlToImageOptions(2480, 3508)).toMatchObject({
            width: 2480,
            height: 3508,
            pixelRatio: 1,
            cacheBust: true,
            backgroundColor: '#08080c',
            style: {
                transform: 'none',
                transformOrigin: 'top left',
            },
        });
    });

    it('uses backend-provided data URLs for export slide images', () => {
        expect(
            imageSourceForExportAsset(
                {
                    path: '/tmp/source-preview.jpg',
                    mime: 'image/jpeg',
                    width: 800,
                    height: 600,
                    data_url: 'data:image/jpeg;base64,abc123',
                },
                path => `tauri://localhost/${path}`
            )
        ).toBe('data:image/jpeg;base64,abc123');
    });

    it('falls back to converted file URLs for older asset responses', () => {
        expect(
            imageSourceForExportAsset(
                {
                    path: '/tmp/source-preview.jpg',
                    mime: 'image/jpeg',
                    width: 800,
                    height: 600,
                },
                path => `tauri://localhost/${path}`
            )
        ).toBe('tauri://localhost//tmp/source-preview.jpg');
    });
});
