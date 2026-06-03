import { writable } from 'svelte/store';
import type { PreviewDisplayMode, PreviewOverlayConfig, PreviewWebStreamStatus } from './api';
import { DEFAULT_PREVIEW_OVERLAY, overlayForPreviewDisplayMode } from './preview-display';

export const PREVIEW_DISPLAY_MODE_SETTING = 'preview_display_mode';
export const PREVIEW_DISPLAY_OVERLAY_SETTING = 'preview_display_overlay';

export const previewDisplayFrozen = writable(false);
export const previewDisplayBlanked = writable(false);
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
            showFilename: parsed.showFilename === true,
            showRating: parsed.showRating === true,
            showDecision: parsed.showDecision === true,
            showMetadataRail: parsed.showMetadataRail === true,
        };
    } catch {
        return null;
    }
}
