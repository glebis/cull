import {
    recordAssetLoadEvent,
    type AssetLoadEventRequest,
} from './api';

export interface DiagnosticImage {
    image: {
        id: string;
        format: string;
    };
    path: string;
    thumbnail_path?: string | null;
}

export interface BuildAssetLoadEventOptions {
    view: string;
    image: DiagnosticImage;
    assetKind: 'source' | 'thumbnail';
    errorKind: string;
    fallbackUsed: boolean;
    fallbackSucceeded: boolean | null;
    phase: string;
}

export function pathBasename(path: string, fallback: string = 'image'): string {
    const normalized = path.replace(/\\/g, '/');
    return normalized.split('/').filter(Boolean).pop() || fallback;
}

export function pathFingerprint(path: string): string {
    let hash = 0x811c9dc5;
    for (let i = 0; i < path.length; i++) {
        hash ^= path.charCodeAt(i);
        hash = Math.imul(hash, 0x01000193);
    }
    return `fnv1a-${(hash >>> 0).toString(16).padStart(8, '0')}`;
}

function diagnosticPath(image: DiagnosticImage, assetKind: 'source' | 'thumbnail'): string {
    if (assetKind === 'thumbnail') {
        return image.thumbnail_path || image.path;
    }
    return image.path;
}

export function buildAssetLoadEvent(options: BuildAssetLoadEventOptions): AssetLoadEventRequest {
    const path = diagnosticPath(options.image, options.assetKind);
    return {
        view: options.view,
        imageId: options.image.image.id,
        assetKind: options.assetKind,
        imageFormat: options.image.image.format || null,
        fallbackUsed: options.fallbackUsed,
        fallbackSucceeded: options.fallbackSucceeded,
        pathBasename: pathBasename(path),
        pathHash: pathFingerprint(path),
        errorKind: options.errorKind,
        detailsJson: JSON.stringify({ phase: options.phase }),
    };
}

export function recordImageLoadFailure(options: BuildAssetLoadEventOptions): void {
    try {
        void recordAssetLoadEvent(buildAssetLoadEvent(options)).catch(() => {});
    } catch {
        // Diagnostics should never make image rendering worse.
    }
}
