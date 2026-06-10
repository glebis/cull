import type { Options } from 'html-to-image/lib/types';
import type { AssetResponse } from './export-types';

export function buildHtmlToImageOptions(width: number, height: number): Options {
    return {
        width,
        height,
        pixelRatio: 1,
        cacheBust: true,
        backgroundColor: '#08080c',
        style: {
            transform: 'none',
            transformOrigin: 'top left',
        },
    };
}

export function imageSourceForExportAsset(
    asset: AssetResponse,
    convertFileSrc: (path: string) => string
): string {
    return asset.data_url || convertFileSrc(asset.path);
}

function resourceLabel(src: string): string {
    if (src.startsWith('data:image/svg+xml')) {
        return 'generated SVG export snapshot';
    }
    if (src.startsWith('data:')) {
        return 'embedded export resource';
    }
    return src;
}

function eventTargetLabel(event: Event): string {
    const target = event.target;
    if (typeof HTMLImageElement !== 'undefined' && target instanceof HTMLImageElement) {
        const src = target.currentSrc || target.src;
        return src ? resourceLabel(src) : 'image resource';
    }
    if (typeof SVGImageElement !== 'undefined' && target instanceof SVGImageElement) {
        return target.href.baseVal ? resourceLabel(target.href.baseVal) : 'SVG image resource';
    }
    if (typeof FileReader !== 'undefined' && target instanceof FileReader) {
        return 'embedded export resource';
    }
    if (target && typeof target === 'object' && 'src' in target && typeof target.src === 'string') {
        return resourceLabel(target.src);
    }
    return 'export snapshot resource';
}

export function formatExportError(error: unknown, context?: string): string {
    let message: string;

    if (error instanceof Error) {
        message = error.message;
    } else if (typeof Event !== 'undefined' && error instanceof Event) {
        message = `Render image failed while loading ${eventTargetLabel(error)}`;
    } else if (typeof error === 'string') {
        message = error;
    } else {
        message = 'Unknown export renderer error';
    }

    return context ? `${context}: ${message}` : message;
}
