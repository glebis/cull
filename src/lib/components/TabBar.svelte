<script lang="ts">
    import { viewMode, thumbnailSize, windowName, navigateTo, showToast, staticPublishingEnabled } from '$lib/stores';
    import type { ViewMode } from '$lib/stores';
    import { maybeShowShortcutReminder, VIEW_CYCLE_SHORTCUT_REMINDER_ID } from '$lib/shortcut-reminders';
    import { visibleViewTabs } from '$lib/view-tabs';
    import ViewTabIcon from './ViewTabIcon.svelte';

    let tabs = $derived(visibleViewTabs($staticPublishingEnabled));

    let size = $state(160);
    thumbnailSize.subscribe(v => size = v);

    function setSize(e: Event) {
        const val = parseInt((e.target as HTMLInputElement).value);
        size = val;
        thumbnailSize.set(val);
    }

    function selectTab(mode: ViewMode) {
        const changed = $viewMode !== mode;
        navigateTo(mode);
        if (!changed) return;
        maybeShowShortcutReminder(VIEW_CYCLE_SHORTCUT_REMINDER_ID, () => {
            showToast('Shortcut available', {
                detail: 'Tab cycles views. Shift+Tab goes back.',
                type: 'info',
                duration: 5000,
            });
        });
    }
</script>

<div class="tabbar" data-tauri-drag-region="deep">
    {#if $windowName && $windowName !== 'Cull'}
        <span class="window-name">{$windowName}</span>
    {/if}
    <div class="tabs">
        {#each tabs as tab}
            <button
                class="tab"
                class:active={$viewMode === tab.id}
                onclick={() => selectTab(tab.id)}
            >
                <ViewTabIcon icon={tab.icon} />{tab.label}{#if tab.key}<span class="tab-key">{tab.key}</span>{/if}
            </button>
        {/each}
    </div>
    {#if $viewMode === 'grid'}
        <div class="slider-group">
            <span class="slider-icon">▪▪</span>
            <div class="slider-track">
                <input
                    type="range"
                    min="80"
                    max="400"
                    step="16"
                    value={size}
                    oninput={setSize}
                    aria-label="Thumbnail size"
                />
            </div>
            <span class="slider-icon">▪</span>
        </div>
    {/if}
</div>

<style>
    .tabbar {
        height: var(--macos-titlebar-safe-area);
        background: var(--surface);
        border-bottom: 1px solid var(--border);
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0 var(--spacing);
        padding-left: var(--macos-window-controls-width);
        grid-area: tabbar;
    }
    .window-name {
        font-size: 11px;
        color: var(--text-secondary);
        margin-right: 8px;
        padding-right: 8px;
        border-right: 1px solid var(--border);
        white-space: nowrap;
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
    }
    .tab:hover:not(:disabled) {
        color: var(--text);
    }
    .tab.active {
        color: var(--green);
        border-bottom-color: var(--green);
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
        width: 10px;
        height: 10px;
        border-radius: 50%;
        background: var(--blue);
        cursor: pointer;
    }
    input[type="range"]::-webkit-slider-thumb:hover {
        background: var(--green);
    }
</style>
