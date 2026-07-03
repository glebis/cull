<script lang="ts">
    import { openPreviewDisplay } from '$lib/api';
    import { viewMode, thumbnailSize, canvasZoom, navigateTo, requestCanvasZoom, showToast } from '$lib/stores';
    import type { ViewMode } from '$lib/stores';
    import {
        canvasZoomFromPosition,
        canvasZoomPositionFromZoom,
    } from '$lib/canvas-interactions';
    import { maybeShowShortcutReminder, VIEW_CYCLE_SHORTCUT_REMINDER_ID } from '$lib/shortcut-reminders';
    import { visibleViewTabs } from '$lib/view-tabs';
    import { tabRegistry } from '$lib/plugins/tab-registry';
    import {
        thumbnailSizeFromZoomPosition,
        zoomPositionFromThumbnailSize,
    } from '$lib/thumbnail-zoom';
    import ViewTabIcon from './ViewTabIcon.svelte';

    let registeredTabIds = $derived(new Set($tabRegistry.map(t => t.id)));
    let tabs = $derived(visibleViewTabs(registeredTabIds));

    let zoomPosition = $state(zoomPositionFromThumbnailSize(160));
    thumbnailSize.subscribe(v => {
        zoomPosition = zoomPositionFromThumbnailSize(v);
    });
    let canvasZoomPosition = $state(canvasZoomPositionFromZoom(1));
    canvasZoom.subscribe(v => {
        canvasZoomPosition = canvasZoomPositionFromZoom(v);
    });

    function setSize(e: Event) {
        const position = parseFloat((e.target as HTMLInputElement).value);
        const val = thumbnailSizeFromZoomPosition(position);
        zoomPosition = position;
        thumbnailSize.set(val);
    }

    function setCanvasZoom(e: Event) {
        const position = parseFloat((e.target as HTMLInputElement).value);
        canvasZoomPosition = position;
        requestCanvasZoom(canvasZoomFromPosition(position));
    }

    function selectTab(mode: ViewMode) {
        const changed = $viewMode !== mode;
        navigateTo(mode);
        if (!changed) return;
        maybeShowShortcutReminder(VIEW_CYCLE_SHORTCUT_REMINDER_ID, () => {
            showToast('Shortcut available', {
                detail: 'Ctrl+Tab cycles views. Ctrl+Shift+Tab goes back.',
                type: 'info',
                duration: 5000,
            });
        });
    }

    function openPreviewDisplayWindow() {
        openPreviewDisplay().catch((e) => {
            showToast('Preview Display failed', {
                detail: String(e),
                type: 'error',
                duration: 8000,
            });
        });
    }
</script>

<div class="tabbar" data-tauri-drag-region="deep">
    <div class="tabbar-left">
        <button
            class="preview-display-launch"
            type="button"
            aria-label="Open Preview Display"
            title="Open Preview Display"
            onclick={openPreviewDisplayWindow}
        >
            <span class="preview-display-icon" aria-hidden="true">
                <span class="preview-display-screen"></span>
                <span class="preview-display-stand"></span>
            </span>
        </button>
    </div>
    <div class="tabbar-center">
        <div class="tabs">
            {#each tabs as tab}
                <button
                    class="tab"
                    class:active={$viewMode === tab.id}
                    class:compact={tab.compact}
                    class:show-label={$viewMode === tab.id}
                    aria-label={tab.label}
                    title={tab.compact ? tab.label : undefined}
                    onclick={() => selectTab(tab.id)}
                >
                    <ViewTabIcon icon={tab.icon} />
                    <span class="tab-label">{tab.label}</span>
                    {#if tab.compact}
                        <span class="tab-label-popover" aria-hidden="true">{tab.label}</span>
                    {/if}
                    {#if tab.key}<span class="tab-key">{tab.key}</span>{/if}
                </button>
            {/each}
        </div>
    </div>
    <div class="tabbar-right">
        {#if $viewMode === 'grid'}
            <div class="slider-group">
                <span class="slider-icon">▪▪</span>
                <div class="slider-track">
                    <input
                        type="range"
                        min="0"
                        max="100"
                        step="1"
                        value={zoomPosition}
                        oninput={setSize}
                        aria-label="Thumbnail size"
                    />
                </div>
                <span class="slider-icon">▪</span>
            </div>
        {:else if $viewMode === 'canvas'}
            <div class="slider-group">
                <span class="slider-icon">-</span>
                <div class="slider-track">
                    <input
                        type="range"
                        min="0"
                        max="100"
                        step="1"
                        value={canvasZoomPosition}
                        oninput={setCanvasZoom}
                        aria-label="Canvas zoom"
                    />
                </div>
                <span class="slider-icon">+</span>
            </div>
        {/if}
    </div>
</div>

<style>
    .tabbar {
        height: var(--macos-titlebar-safe-area);
        background: var(--surface);
        border-bottom: 1px solid var(--border);
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
        align-items: center;
        padding: 0 var(--spacing) 0 0;
        grid-area: tabbar;
    }

    .tabbar-left,
    .tabbar-right {
        display: flex;
        align-items: center;
        min-width: 0;
    }

    .tabbar-left {
        justify-self: start;
        padding-left: var(--macos-window-controls-width);
    }

    .tabbar-center {
        justify-self: center;
        min-width: 0;
    }

    .tabbar-right {
        justify-self: end;
    }

    .preview-display-launch {
        width: 28px;
        height: 28px;
        border: 1px solid transparent;
        border-radius: var(--radius);
        background: transparent;
        color: var(--text-secondary);
        display: grid;
        place-items: center;
        cursor: pointer;
        flex: 0 0 auto;
    }

    .preview-display-launch:hover {
        color: var(--blue);
        border-color: var(--border);
        background: color-mix(in srgb, var(--blue) 10%, transparent);
    }

    .preview-display-icon {
        width: 17px;
        height: 17px;
        display: grid;
        grid-template-rows: 12px 5px;
        justify-items: center;
        align-items: start;
    }

    .preview-display-screen {
        width: 17px;
        height: 11px;
        border: 1.5px solid currentColor;
        border-radius: 2px;
        box-shadow: inset 0 0 0 1px color-mix(in srgb, currentColor 16%, transparent);
    }

    .preview-display-stand {
        width: 9px;
        height: 5px;
        border-bottom: 1.5px solid currentColor;
        position: relative;
    }

    .preview-display-stand::before {
        content: '';
        position: absolute;
        left: 50%;
        top: -1px;
        width: 1.5px;
        height: 5px;
        background: currentColor;
        transform: translateX(-50%);
    }

    .tabs {
        display: flex;
        gap: 2px;
    }
    .tab {
        background: none;
        border: none;
        border-bottom: 2px solid transparent;
        color: var(--text-secondary);
        display: inline-flex;
        align-items: center;
        gap: 5px;
        font-family: var(--font);
        font-size: 12px;
        line-height: 1;
        padding: 8px 12px 6px;
        cursor: pointer;
        transition: all 0.15s;
        white-space: nowrap;
        position: relative;
    }
    .tab:hover:not(:disabled) {
        color: var(--text);
    }
    .tab.active {
        color: var(--green);
        border-bottom-color: var(--green);
    }
    .tab-label {
        display: inline-block;
        transition: opacity 0.16s ease, transform 0.16s ease, max-width 0.16s ease;
    }
    .tab.compact:not(.show-label) {
        width: 34px;
        justify-content: center;
        gap: 0;
        padding-left: 8px;
        padding-right: 8px;
    }
    .tab.compact:not(.show-label) .tab-label,
    .tab.compact:not(.show-label) .tab-key {
        max-width: 0;
        opacity: 0;
        overflow: hidden;
        transform: translateX(-2px);
    }
    .tab-label-popover {
        position: absolute;
        left: calc(100% - 2px);
        top: 50%;
        padding: 4px 7px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--text);
        opacity: 0;
        pointer-events: none;
        transform: translate3d(-4px, -50%, 0);
        transition: opacity 0.14s ease, transform 0.14s ease;
        z-index: 3;
    }
    .tab.compact:hover .tab-label-popover,
    .tab.compact:focus-visible .tab-label-popover {
        opacity: 1;
        transform: translate3d(0, -50%, 0);
    }
    .tab.compact.show-label .tab-label-popover {
        display: none;
    }
    .tab-key {
        color: var(--text-secondary);
        font-size: 9px;
        opacity: 0.25;
    }
    .tab:hover .tab-key {
        opacity: 0.5;
    }
    .tab.active .tab-key {
        color: var(--green);
        opacity: 0.35;
    }
    .slider-group {
        display: flex;
        align-items: center;
        gap: 6px;
    }
    .slider-icon {
        color: var(--text-secondary);
        font-size: 8px;
        opacity: 0.5;
        letter-spacing: 1px;
    }
    .slider-track {
        width: 80px;
        display: flex;
        align-items: center;
    }
    input[type="range"] {
        -webkit-appearance: none;
        appearance: none;
        width: 100%;
        height: 2px;
        background: var(--border);
        border-radius: 1px;
        outline: none;
        cursor: pointer;
    }
    input[type="range"]::-webkit-slider-thumb {
        -webkit-appearance: none;
        appearance: none;
        width: 16px;
        height: 16px;
        border-radius: 50%;
        background: var(--blue);
        cursor: pointer;
    }
    input[type="range"]::-webkit-slider-thumb:hover {
        background: var(--green);
    }
    @media (prefers-reduced-motion: reduce) {
        .tab,
        .tab-label,
        .tab-label-popover {
            transition: none;
        }
    }
</style>
