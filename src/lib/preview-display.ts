import type {
    ImageWithFile,
    PreviewDisplayMode,
    PreviewOverlayConfig,
    PreviewRailSide,
    PreviewRailTextSize,
    PreviewRailWidth,
    PreviewState,
} from './api';
import { isRawFormat, updatePreviewState } from './api';

export const DEFAULT_PREVIEW_OVERLAY: PreviewOverlayConfig = {
    showFilename: false,
    showRating: false,
    showDecision: false,
    showMetadataRail: false,
    showDimensions: false,
    showFormat: false,
    showSource: false,
    showPrompt: false,
    showTags: false,
    showHistogram: false,
    railSide: 'right',
    railWidth: 'medium',
    railTextSize: 'medium',
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
            ...DEFAULT_PREVIEW_OVERLAY,
            showFilename: true,
            showRating: true,
            showDecision: true,
            showMetadataRail: false,
        };
    }
    if (mode === 'metadata_review') {
        return {
            ...DEFAULT_PREVIEW_OVERLAY,
            showFilename: true,
            showRating: true,
            showDecision: true,
            showMetadataRail: true,
            showDimensions: true,
            showFormat: true,
            showSource: true,
            showPrompt: true,
            showTags: true,
        };
    }
    return DEFAULT_PREVIEW_OVERLAY;
}

export type PreviewDisplayField =
    | 'showFilename'
    | 'showRating'
    | 'showDecision'
    | 'showDimensions'
    | 'showFormat'
    | 'showSource'
    | 'showPrompt'
    | 'showTags'
    | 'showHistogram';

function previewDisplayRailFieldVisible(overlay: PreviewOverlayConfig): boolean {
    return overlay.showDimensions
        || overlay.showFormat
        || overlay.showSource
        || overlay.showPrompt
        || overlay.showTags
        || overlay.showHistogram;
}

export function previewDisplayRailVisible(overlay: PreviewOverlayConfig): boolean {
    return overlay.showMetadataRail
        || previewDisplayRailFieldVisible(overlay);
}

export function withPreviewDisplayField(
    overlay: PreviewOverlayConfig,
    field: PreviewDisplayField,
    value: boolean
): PreviewOverlayConfig {
    const next = {
        ...overlay,
        [field]: value,
    };
    return {
        ...next,
        showMetadataRail: previewDisplayRailFieldVisible(next),
    };
}

export function withPreviewDisplayRailSide(
    overlay: PreviewOverlayConfig,
    railSide: PreviewRailSide
): PreviewOverlayConfig {
    return { ...overlay, railSide };
}

export function withPreviewDisplayRailWidth(
    overlay: PreviewOverlayConfig,
    railWidth: PreviewRailWidth
): PreviewOverlayConfig {
    return { ...overlay, railWidth };
}

export function withPreviewDisplayRailTextSize(
    overlay: PreviewOverlayConfig,
    railTextSize: PreviewRailTextSize
): PreviewOverlayConfig {
    return { ...overlay, railTextSize };
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
    // The Preview Display is a presentation surface, so it shows the full-resolution
    // original by default. It falls back to the thumbnail for non-browser-decodable
    // formats (e.g. RAW/PDF) or when loading the original failed. (main's
    // chooseLoupeImagePath now always prefers the thumbnail for grid/loupe perf,
    // which is the opposite of what a preview display wants.)
    if ((isRawFormat(image.image.format) || sourceLoadFailed) && image.thumbnail_path) {
        return image.thumbnail_path;
    }
    return image.path;
}
