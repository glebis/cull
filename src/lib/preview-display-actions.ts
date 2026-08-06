/**
 * Preview Display actions shared by the native menu (`menu.ts`) and the command
 * palette (`command-palette.ts`).
 *
 * Both surfaces must drive the same code path — a second implementation would
 * drift from this one the moment a toast string or a persistence call changes.
 * This module is deliberately a leaf: `menu.ts` already imports
 * `openCommandPalette` from `command-palette.ts`, so the palette importing
 * `menu.ts` would close an import cycle.
 */
import { emit } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import { get } from 'svelte/store';
import {
    openPreviewDisplay,
    setPreviewDisplayAlwaysOnTop as setPreviewDisplayAlwaysOnTopNative,
    listPreviewDisplayMonitors,
    placePreviewDisplay,
    startPreviewDisplayWebStream,
    stopPreviewDisplayWebStream,
    setAppSetting,
    type PreviewDisplayLayout,
    type PreviewDisplayMode,
    type PreviewWebStreamStatus,
} from './api';
import { showToast } from './stores';
import {
    PREVIEW_DISPLAY_MODE_SETTING,
    PREVIEW_DISPLAY_LAYOUT_SETTING,
    PREVIEW_DISPLAY_OVERLAY_SETTING,
    previewDisplayAlwaysOnTop,
    previewDisplayBlanked,
    previewDisplayFrozen,
    previewDisplayOverlay,
    previewDisplayWebStreamStatus,
    setPreviewDisplayBlanked,
    setPreviewDisplayAlwaysOnTop,
    setPreviewDisplayFrozen,
    setPreviewDisplayLayout,
    setPreviewDisplayMode,
    setPreviewDisplayOverlay,
    setPreviewDisplayWebStreamStatus,
} from './preview-display-store';
import {
    overlayForPreviewDisplayMode,
    withPreviewDisplayField,
    withPreviewDisplayRailSide,
    withPreviewDisplayRailTextSize,
    withPreviewDisplayRailWidth,
    type PreviewDisplayField,
} from './preview-display';
import type { PreviewRailSide, PreviewRailTextSize, PreviewRailWidth } from './api';

export function handleOpenPreviewDisplay() {
    openPreviewDisplay().catch((e) => {
        showToast('Preview Display failed', { detail: String(e), type: 'error', duration: 8000 });
    });
}

export function handlePreviewDisplayFreeze() {
    const next = !get(previewDisplayFrozen);
    setPreviewDisplayFrozen(next);
    showToast(next ? 'Preview Display frozen' : 'Preview Display live', { type: 'info', duration: 3000 });
}

export function handlePreviewDisplayBlank() {
    const next = !get(previewDisplayBlanked);
    setPreviewDisplayBlanked(next);
    showToast(next ? 'Preview Display blanked' : 'Preview Display visible', { type: 'info', duration: 3000 });
}

export async function handlePreviewDisplayAlwaysOnTop() {
    const previous = get(previewDisplayAlwaysOnTop);
    const next = !previous;
    setPreviewDisplayAlwaysOnTop(next);
    try {
        await setPreviewDisplayAlwaysOnTopNative(next);
        showToast(next ? 'Preview Display stays on top' : 'Preview Display normal stacking', {
            type: 'info',
            duration: 3000,
        });
    } catch (e) {
        setPreviewDisplayAlwaysOnTop(previous);
        showToast('Preview Display stacking failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

export async function handlePreviewDisplayPreset(mode: PreviewDisplayMode) {
    const overlay = overlayForPreviewDisplayMode(mode);
    setPreviewDisplayMode(mode);
    try {
        await setAppSetting(PREVIEW_DISPLAY_MODE_SETTING, mode);
        await setAppSetting(PREVIEW_DISPLAY_OVERLAY_SETTING, JSON.stringify(overlay));
    } catch (e) {
        showToast('Preview Display preset not saved', { detail: String(e), type: 'warning', duration: 6000 });
    }
}

export async function handlePreviewDisplayLayout(layout: PreviewDisplayLayout) {
    setPreviewDisplayLayout(layout);
    try {
        await setAppSetting(PREVIEW_DISPLAY_LAYOUT_SETTING, layout);
    } catch (e) {
        showToast('Preview Display layout not saved', { detail: String(e), type: 'warning', duration: 6000 });
    }
}

export async function persistPreviewDisplayOverlay(overlay = get(previewDisplayOverlay)) {
    setPreviewDisplayOverlay(overlay);
    try {
        await setAppSetting(PREVIEW_DISPLAY_OVERLAY_SETTING, JSON.stringify(overlay));
    } catch (e) {
        showToast('Preview Display settings not saved', { detail: String(e), type: 'warning', duration: 6000 });
    }
}

export function handlePreviewDisplayField(field: PreviewDisplayField) {
    const overlay = get(previewDisplayOverlay);
    persistPreviewDisplayOverlay(withPreviewDisplayField(overlay, field, !overlay[field]));
}

export function handlePreviewDisplayRailSide(side: PreviewRailSide) {
    persistPreviewDisplayOverlay(withPreviewDisplayRailSide(get(previewDisplayOverlay), side));
}

export function handlePreviewDisplayRailWidth(width: PreviewRailWidth) {
    persistPreviewDisplayOverlay(withPreviewDisplayRailWidth(get(previewDisplayOverlay), width));
}

export function handlePreviewDisplayRailTextSize(size: PreviewRailTextSize) {
    persistPreviewDisplayOverlay(withPreviewDisplayRailTextSize(get(previewDisplayOverlay), size));
}

export async function requestPreviewDisplayCapture(destination: 'clipboard' | 'png') {
    try {
        await openPreviewDisplay();
        await emit('preview-display:capture-request', { destination });
        showToast(destination === 'clipboard' ? 'Preview Display copy requested' : 'Preview Display export requested', {
            type: 'info',
            duration: 3000,
        });
    } catch (e) {
        showToast('Preview Display capture failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

function displayLabel(monitor: { name: string | null; width: number; height: number; primary: boolean }, index: number): string {
    const name = monitor.name || `Display ${index + 1}`;
    return `${name}${monitor.primary ? ' (Primary)' : ''} ${monitor.width}x${monitor.height}`;
}

export async function handlePreviewDisplayMoveMonitor() {
    try {
        const monitors = await listPreviewDisplayMonitors();
        if (monitors.length === 0) {
            showToast('No displays available', { type: 'warning' });
            return;
        }
        showToast('Move Preview Display', {
            detail: 'Choose display',
            duration: 12000,
            actions: monitors.slice(0, 4).map((monitor, index) => ({
                label: displayLabel(monitor, index),
                onclick: () => {
                    placePreviewDisplay(monitor.id, false).catch((e) => {
                        showToast('Preview Display move failed', { detail: String(e), type: 'error', duration: 8000 });
                    });
                },
            })),
        });
    } catch (e) {
        showToast('Display list unavailable', { detail: String(e), type: 'error', duration: 8000 });
    }
}

export async function handlePreviewDisplayFullscreen() {
    try {
        await placePreviewDisplay(null, true);
    } catch (e) {
        showToast('Preview Display fullscreen failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

export async function copyPreviewDisplayWebStreamUrl(status: PreviewWebStreamStatus = get(previewDisplayWebStreamStatus)) {
    if (!status.active || !status.url) {
        showToast('Preview Display web stream is not running', { type: 'warning', duration: 4000 });
        return;
    }
    try {
        await navigator.clipboard.writeText(status.url);
        showToast('Preview Display URL copied', { detail: status.url, type: 'success', duration: 8000 });
    } catch (e) {
        showToast('Preview Display URL ready', { detail: `${status.url} Copy failed: ${String(e)}`, type: 'warning', duration: 10000 });
    }
}

function showPreviewDisplayWebStreamToast(status: PreviewWebStreamStatus) {
    if (!status.url) return;
    showToast('Preview Display web stream live', {
        detail: status.url,
        type: 'success',
        duration: 12000,
        actions: [
            {
                label: 'Open',
                onclick: () => {
                    openUrl(status.url!).catch((e) => {
                        showToast('Could not open Preview Display URL', { detail: String(e), type: 'error', duration: 8000 });
                    });
                },
            },
            {
                label: 'Copy',
                onclick: () => {
                    copyPreviewDisplayWebStreamUrl(status);
                },
            },
            {
                label: 'Stop',
                onclick: () => {
                    handlePreviewDisplayStopWebStream();
                },
            },
        ],
    });
}

export async function handlePreviewDisplayStartWebStream(host: '127.0.0.1' | '0.0.0.0' = '127.0.0.1') {
    try {
        const status = await startPreviewDisplayWebStream(host, null);
        setPreviewDisplayWebStreamStatus(status);
        await copyPreviewDisplayWebStreamUrl(status);
        showPreviewDisplayWebStreamToast(status);
    } catch (e) {
        showToast('Preview Display web stream failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

export async function handlePreviewDisplayStopWebStream() {
    try {
        const status = await stopPreviewDisplayWebStream();
        setPreviewDisplayWebStreamStatus(status);
        showToast('Preview Display web stream stopped', { type: 'info', duration: 4000 });
    } catch (e) {
        showToast('Preview Display web stream stop failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}
