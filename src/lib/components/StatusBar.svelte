<script lang="ts">
    import { viewMode, totalCount, selectedCount, statusHint, gridPreset, GRID_PRESETS, activeCollection, collections, showDetectionBoxes, nsfwMode } from '$lib/stores';
    import { derived } from 'svelte/store';

    const collectionName = derived(
        [activeCollection, collections],
        ([$id, $cols]) => {
            if (!$id) return null;
            const found = $cols.find(c => c[0] === $id);
            return found ? found[1] : null;
        }
    );
</script>

<div class="statusbar">
    <div class="left">
        <span class="mode">{$viewMode}</span>
        {#if $collectionName}
            <span class="sep">|</span>
            <span class="collection-name">{$collectionName}</span>
        {/if}
        <span class="sep">|</span>
        <span>{$totalCount} images</span>
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
    </div>
    <div class="right">
        {#if $showDetectionBoxes}
            <span class="hint active-hint">D:boxes</span>
        {:else}
            <span class="hint">D:boxes</span>
        {/if}
        <span class="hint">B:nsfw:{$nsfwMode}</span>
        <span class="hint">hjkl:nav</span>
        <span class="hint">space:select</span>
        <span class="hint">s+1-5:rate</span>
        <span class="hint">0:clear</span>
        <span class="hint">a:accept</span>
        <span class="hint">x:reject</span>
        <span class="hint">u:undecide</span>
        <span class="hint">c:collect</span>
        <span class="hint">b:batch</span>
        <span class="hint">+/-:size</span>
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
    }
    .left {
        display: flex;
        align-items: center;
        gap: 8px;
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
        gap: 12px;
    }
    .hint {
        color: var(--text-secondary);
        font-size: 10px;
    }
    .active-hint {
        color: var(--green, #9ece6a);
    }
    .preset {
        color: var(--text-secondary);
        font-size: 10px;
    }
    .status-hint {
        color: var(--orange);
        font-weight: 700;
    }
    .collection-name {
        color: var(--blue);
        font-weight: 600;
    }
</style>
