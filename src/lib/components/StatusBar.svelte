<script lang="ts">
    import { viewMode, totalCount, images, selectedCount, statusHint, gridPreset, GRID_PRESETS, activeCollection, collections, activeFolder, folders, activeSmartCollection, activeDetectedClass, imageLoadState, showDetectionBoxes, nsfwMode, shortcutsOpen } from '$lib/stores';
    import { previewDisplayBlanked, previewDisplayFrozen, previewDisplayWebStreamStatus } from '$lib/preview-display-store';
    import { previewDisplayStatusLabel } from '$lib/preview-display';
    import { openCommandPalette } from '$lib/command-palette';
    import { derived } from 'svelte/store';

    const displayCount = derived(
        [images, totalCount, activeCollection, collections, activeFolder, folders, activeSmartCollection, activeDetectedClass, imageLoadState],
        ([$imgs, $total, $activeCollection, $collections, $activeFolder, $folders, $activeSmartCollection, $activeDetectedClass, $imageLoadState]) => {
            const showing = $imgs.length;
            let scopeTotal: number | null = $total;
            if ($activeCollection) {
                scopeTotal = $collections.find(c => c[0] === $activeCollection)?.[2] ?? null;
            } else if ($activeFolder) {
                scopeTotal = $folders.find(f => f[0] === $activeFolder)?.[1] ?? null;
            } else if ($activeSmartCollection) {
                scopeTotal = $activeSmartCollection.image_count;
            } else if ($activeDetectedClass) {
                scopeTotal = null;
            }
            if (scopeTotal !== null) {
                if (showing < scopeTotal && showing > 0) {
                    return `${showing} / ${scopeTotal} images`;
                }
                return `${scopeTotal} images`;
            }
            return $imageLoadState.hasMore ? `${showing}+ images` : `${showing} images`;
        }
    );

    const collectionName = derived(
        [activeCollection, collections],
        ([$id, $cols]) => {
            if (!$id) return null;
            const found = $cols.find(c => c[0] === $id);
            return found ? found[1] : null;
        }
    );

    const modeLabel = derived(viewMode, ($mode) => $mode === 'tinder' ? 'Speed Review' : $mode);
    const previewStatus = derived(
        [previewDisplayFrozen, previewDisplayBlanked],
        ([$frozen, $blanked]) => previewDisplayStatusLabel($frozen, $blanked)
    );

    function openShortcuts() {
        shortcutsOpen.set(true);
    }

    function openCommands() {
        openCommandPalette('commands');
    }
</script>

<div class="statusbar">
    <div class="left">
        <span class="mode">{$modeLabel}</span>
        {#if $collectionName}
            <span class="sep">|</span>
            <span class="collection-name">{$collectionName}</span>
        {/if}
        <span class="sep">|</span>
        <span>{$displayCount}</span>
        {#if $selectedCount > 0}
            <span class="sep">|</span>
            <span class="selected">{$selectedCount} selected</span>
        {/if}
        {#if $viewMode === 'grid'}
            <span class="sep">|</span>
            <span class="preset">{GRID_PRESETS[$gridPreset].name}</span>
        {/if}
        {#if $statusHint}
            <span class="sep">|</span>
            <span class="status-hint">{$statusHint}</span>
        {/if}
        {#if $previewStatus}
            <span class="sep">|</span>
            <span class="preview-status">{$previewStatus}</span>
        {/if}
        {#if $previewDisplayWebStreamStatus.active}
            <span class="sep">|</span>
            <span class="preview-status" title={$previewDisplayWebStreamStatus.url ?? ''}>Preview web live</span>
        {/if}
    </div>
    <div class="right">
        {#if $showDetectionBoxes}
            <span class="state-chip active-hint">D:boxes</span>
        {/if}
        <span class="state-chip">B:nsfw:{$nsfwMode}</span>
        <button class="shortcut-button" type="button" onclick={openShortcuts} title="?:help" aria-label="Open keyboard shortcuts">
            <kbd>?</kbd>
            <span>Shortcuts</span>
        </button>
        <button class="shortcut-button" type="button" onclick={openCommands} title="Cmd+P:commands" aria-label="Open command palette">
            <kbd>Cmd+P</kbd>
            <span>Commands</span>
        </button>
    </div>
</div>

<style>
    .statusbar {
        height: 32px;
        background: var(--surface);
        border-top: 1px solid var(--border);
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0 12px;
        font-size: 11px;
        grid-area: statusbar;
        gap: 12px;
        min-width: 0;
        overflow: hidden;
        box-sizing: border-box;
    }
    .left {
        display: flex;
        align-items: center;
        gap: 8px;
        flex: 1 1 auto;
        min-width: 0;
        overflow: hidden;
    }
    .mode {
        color: var(--green);
        font-weight: 700;
        text-transform: uppercase;
    }
    .sep {
        color: var(--border);
    }
    .selected {
        color: var(--blue);
    }
    .right {
        display: flex;
        align-items: center;
        gap: 8px;
        overflow: hidden;
        flex: 0 1 auto;
        min-width: 0;
    }
    .state-chip,
    .shortcut-button {
        color: var(--text-secondary);
        font-size: 10px;
        white-space: nowrap;
    }
    .state-chip {
        display: inline-flex;
        align-items: center;
        min-height: 18px;
    }
    .shortcut-button {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        height: 22px;
        padding: 0 6px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        font: inherit;
        line-height: 1;
        cursor: default;
    }
    .shortcut-button:hover,
    .shortcut-button:focus-visible {
        color: var(--text);
        border-color: var(--blue);
        outline: none;
    }
    .shortcut-button kbd {
        color: var(--blue);
        font: inherit;
    }
    .active-hint {
        color: var(--green);
    }
    .preset {
        color: var(--text-secondary);
        font-size: 10px;
    }
    .status-hint {
        color: var(--orange);
        font-weight: 700;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .preview-status {
        color: var(--orange);
        font-weight: 700;
        white-space: nowrap;
    }
    .collection-name {
        color: var(--blue);
        font-weight: 600;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
</style>
