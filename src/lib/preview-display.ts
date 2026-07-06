import type {
    ImageWithFile,
    PreviewDisplayLayout,
    PreviewDisplayMode,
    PreviewOverlayConfig,
    PreviewRailSide,
    PreviewRailTextSize,
    PreviewRailWidth,
    PreviewState,
} from './api';
import { isRawFormat, updatePreviewState } from './api';

const PREVIEW_DISPLAY_PRESET_ORDER: PreviewDisplayMode[] = ['image_only', 'client_review', 'metadata_review'];

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

export const PREVIEW_DISPLAY_LAYOUT_LIMITS: Record<PreviewDisplayLayout, number> = {
    single: 1,
    compare: 2,
    grid: 4,
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
        imageIds: image ? [image.image.id] : [],
        displayMode: current?.display_mode ?? 'image_only',
        layout: current?.layout ?? 'single',
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

export function nextPreviewDisplayPresetMode(mode: PreviewDisplayMode): PreviewDisplayMode {
    const index = PREVIEW_DISPLAY_PRESET_ORDER.indexOf(mode);
    const nextIndex = index === -1 ? 0 : (index + 1) % PREVIEW_DISPLAY_PRESET_ORDER.length;
    return PREVIEW_DISPLAY_PRESET_ORDER[nextIndex];
}

export function isPreviewDisplayPresetCycleShortcut(input: {
    key: string;
    ctrlKey?: boolean;
    metaKey?: boolean;
    altKey?: boolean;
}): boolean {
    return input.key === 'Tab' && input.ctrlKey === true && input.metaKey !== true && input.altKey !== true;
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

export function previewStateImageIds(state: PreviewState | null): string[] {
    if (!state) return [];
    if (state.image_ids?.length) return state.image_ids;
    return state.image_id ? [state.image_id] : [];
}

export function previewSyncImageIds(
    image: ImageWithFile | null,
    allImages: ImageWithFile[],
    selectedIds: Set<string>,
    current: PreviewState | null,
    frozen: boolean,
    blanked: boolean,
    layout: PreviewDisplayLayout
): string[] {
    if (blanked) return [];
    if (frozen) {
        const frozenIds = previewStateImageIds(current);
        return frozenIds.length ? frozenIds : image ? [image.image.id] : [];
    }
    if (!image) return [];

    const limit = PREVIEW_DISPLAY_LAYOUT_LIMITS[layout] ?? 1;
    const ids: string[] = [image.image.id];
    const append = (id: string) => {
        if (ids.length >= limit || ids.includes(id)) return;
        ids.push(id);
    };

    if (selectedIds.size > 0) {
        for (const candidate of allImages) {
            if (selectedIds.has(candidate.image.id)) append(candidate.image.id);
        }
        return ids;
    }

    const focusedIndex = allImages.findIndex((candidate) => candidate.image.id === image.image.id);
    const ordered = focusedIndex === -1
        ? allImages
        : allImages.slice(focusedIndex + 1).concat(allImages.slice(0, focusedIndex));
    for (const candidate of ordered) {
        append(candidate.image.id);
    }

    return ids;
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
