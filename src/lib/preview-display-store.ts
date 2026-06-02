import { writable } from 'svelte/store';
import type {
    PreviewDisplayMode,
    PreviewOverlayConfig,
    PreviewRailSide,
    PreviewRailTextSize,
    PreviewRailWidth,
    PreviewWebStreamStatus,
} from './api';
import { DEFAULT_PREVIEW_OVERLAY, overlayForPreviewDisplayMode } from './preview-display';

export const PREVIEW_DISPLAY_MODE_SETTING = 'preview_display_mode';
export const PREVIEW_DISPLAY_OVERLAY_SETTING = 'preview_display_overlay';
export const PREVIEW_DISPLAY_ALWAYS_ON_TOP_SETTING = 'preview_display_always_on_top';

export const previewDisplayFrozen = writable(false);
export const previewDisplayBlanked = writable(false);
export const previewDisplayAlwaysOnTop = writable(false);
export const previewDisplayMode = writable<PreviewDisplayMode>('image_only');
export const previewDisplayOverlay = writable<PreviewOverlayConfig>(DEFAULT_PREVIEW_OVERLAY);
export const previewDisplayWebStreamStatus = writable<PreviewWebStreamStatus>({
    active: false,
    url: null,
    host: null,
    bound_host: null,
    port: null,
    remote_access: false,
});

export function setPreviewDisplayFrozen(value: boolean) {
    previewDisplayFrozen.set(value);
}

export function setPreviewDisplayBlanked(value: boolean) {
    previewDisplayBlanked.set(value);
}

export function setPreviewDisplayAlwaysOnTop(value: boolean) {
    previewDisplayAlwaysOnTop.set(value);
}

export function setPreviewDisplayMode(mode: PreviewDisplayMode) {
    previewDisplayMode.set(mode);
    previewDisplayOverlay.set(overlayForPreviewDisplayMode(mode));
}

export function setPreviewDisplayOverlay(overlay: PreviewOverlayConfig) {
    previewDisplayOverlay.set(overlay);
}

export function setPreviewDisplayWebStreamStatus(status: PreviewWebStreamStatus) {
    previewDisplayWebStreamStatus.set(status);
}

export function parsePreviewDisplayMode(value: string | null): PreviewDisplayMode {
    if (value === 'client_review' || value === 'metadata_review') return value;
    return 'image_only';
}

export function parsePreviewDisplayOverlay(value: string | null): PreviewOverlayConfig | null {
    if (!value) return null;
    try {
        const parsed = JSON.parse(value) as Partial<PreviewOverlayConfig>;
        return {
            ...DEFAULT_PREVIEW_OVERLAY,
            showFilename: parsed.showFilename === true,
            showRating: parsed.showRating === true,
            showDecision: parsed.showDecision === true,
            showMetadataRail: parsed.showMetadataRail === true,
            showDimensions: parsed.showDimensions === true,
            showFormat: parsed.showFormat === true,
            showSource: parsed.showSource === true,
            showPrompt: parsed.showPrompt === true,
            showTags: parsed.showTags === true,
            showHistogram: parsed.showHistogram === true,
            railSide: validRailSide(parsed.railSide) ? parsed.railSide : DEFAULT_PREVIEW_OVERLAY.railSide,
            railWidth: validRailWidth(parsed.railWidth) ? parsed.railWidth : DEFAULT_PREVIEW_OVERLAY.railWidth,
            railTextSize: validRailTextSize(parsed.railTextSize)
                ? parsed.railTextSize
                : DEFAULT_PREVIEW_OVERLAY.railTextSize,
        };
    } catch {
        return null;
    }
}

function validRailSide(value: unknown): value is PreviewRailSide {
    return value === 'left' || value === 'right';
}

function validRailWidth(value: unknown): value is PreviewRailWidth {
    return value === 'narrow' || value === 'medium' || value === 'wide';
}

function validRailTextSize(value: unknown): value is PreviewRailTextSize {
    return value === 'small' || value === 'medium' || value === 'large';
}
