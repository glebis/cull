<script lang="ts">
    import { viewMode, totalCount, selectedCount, statusHint } from '$lib/stores';
</script>

<div class="statusbar">
    <div class="left">
        <span class="mode">{$viewMode}</span>
        <span class="sep">|</span>
        <span>{$totalCount} images</span>
        {#if $selectedCount > 0}
            <span class="sep">|</span>
            <span class="selected">{$selectedCount} selected</span>
        {/if}
        {#if $statusHint}
            <span class="sep">|</span>
            <span class="status-hint">{$statusHint}</span>
        {/if}
    </div>
    <div class="right">
        <span class="hint">hjkl:nav</span>
        <span class="hint">space:select</span>
        <span class="hint">s+1-5:rate</span>
        <span class="hint">0:clear</span>
        <span class="hint">a:accept</span>
        <span class="hint">x:reject</span>
        <span class="hint">u:undecide</span>
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
    .status-hint {
        color: var(--orange);
        font-weight: 700;
    }
</style>
