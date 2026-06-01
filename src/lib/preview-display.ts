import type { ImageWithFile, PreviewDisplayMode, PreviewOverlayConfig, PreviewState } from './api';
import { isRawFormat, updatePreviewState } from './api';
import { chooseLoupeImagePath } from './view-utils';

export const DEFAULT_PREVIEW_OVERLAY: PreviewOverlayConfig = {
    showFilename: false,
    showRating: false,
    showDecision: false,
    showMetadataRail: false,
};

export function isPreviewDisplayRoute(search?: string): boolean {
    const query = search ?? (typeof window !== 'undefined' ? window.location.search : '');
    const params = new URLSearchParams(query);
    return params.get('previewDisplay') === '1'
        || params.get('previewDisplay') === 'true'
        || params.get('window') === 'preview-display';
}

export function nextPreviewFocusPayload(image: ImageWithFile | null, current: PreviewState | null) {
    return {
        imageId: image?.image.id ?? null,
        displayMode: current?.display_mode ?? 'image_only',
        overlay: current?.overlay ?? DEFAULT_PREVIEW_OVERLAY,
    } as const;
}

export function overlayForPreviewDisplayMode(mode: PreviewDisplayMode): PreviewOverlayConfig {
    if (mode === 'client_review') {
        return {
            showFilename: true,
            showRating: true,
            showDecision: true,
            showMetadataRail: false,
        };
    }
    if (mode === 'metadata_review') {
        return {
            showFilename: true,
            showRating: true,
            showDecision: true,
            showMetadataRail: true,
        };
    }
    return DEFAULT_PREVIEW_OVERLAY;
}

export function previewSyncImageId(
    image: ImageWithFile | null,
    current: PreviewState | null,
    frozen: boolean,
    blanked: boolean
): string | null {
    if (blanked) return null;
    if (frozen) return current?.image_id ?? image?.image.id ?? null;
    return image?.image.id ?? null;
}

export function previewDisplayStatusLabel(frozen: boolean, blanked: boolean): string | null {
    if (blanked) return 'Preview blanked';
    if (frozen) return 'Preview frozen';
    return null;
}

export async function syncPreviewFocus(image: ImageWithFile | null, current: PreviewState | null): Promise<PreviewState> {
    const payload = nextPreviewFocusPayload(image, current);
    return updatePreviewState(payload.imageId, payload.displayMode, payload.overlay);
}

export function previewDisplayImageSourcePath(image: ImageWithFile, sourceLoadFailed: boolean): string {
    return chooseLoupeImagePath(image, isRawFormat(image.image.format), sourceLoadFailed);
}
