<script lang="ts">
    import { viewMode, thumbnailSize } from '$lib/stores';
    import type { ViewMode } from '$lib/stores';

    const tabs: { id: ViewMode; label: string; key: string }[] = [
        { id: 'grid', label: 'Grid', key: '1' },
        { id: 'compare', label: 'Compare', key: '2' },
        { id: 'loupe', label: 'Loupe', key: '3' },
        { id: 'canvas', label: 'Canvas', key: '4' },
        { id: 'lineage', label: 'Lineage', key: '5' },
        { id: 'embeddings', label: 'Embeddings', key: '6' },
        { id: 'export', label: 'Export', key: '7' },
    ];

    let size = $state(160);
    thumbnailSize.subscribe(v => size = v);

    function setSize(e: Event) {
        const val = parseInt((e.target as HTMLInputElement).value);
        size = val;
        thumbnailSize.set(val);
    }
</script>

<div class="tabbar">
    <div class="tabs">
        {#each tabs as tab}
            <button
                class="tab"
                class:active={$viewMode === tab.id}
                onclick={() => viewMode.set(tab.id)}
            >
                <span class="tab-key">{tab.key}</span>{tab.label}
            </button>
        {/each}
    </div>
    <div class="slider-group">
        <span class="slider-label">{size}px</span>
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
</div>

<style>
    .tabbar {
        height: 40px;
        background: var(--surface);
        border-bottom: 1px solid var(--border);
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0 var(--spacing);
        grid-area: tabbar;
    }
    .tabs {
        display: flex;
        gap: 2px;
    }
    .tab {
        background: none;
        border: none;
        color: var(--text-secondary);
        font-family: var(--font);
        font-size: 12px;
        padding: 4px 12px;
        cursor: pointer;
        border-radius: var(--radius);
        transition: all 0.15s;
    }
    .tab:hover:not(:disabled) {
        color: var(--text);
        background: var(--border);
    }
    .tab.active {
        color: var(--green);
        background: rgba(158, 206, 106, 0.1);
    }
    .tab-key {
        color: var(--text-secondary);
        font-size: 10px;
        margin-right: 4px;
        opacity: 0.5;
    }
    .tab.active .tab-key {
        color: var(--green);
    }
    .slider-group {
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .slider-label {
        color: var(--text-secondary);
        font-size: 11px;
        min-width: 42px;
        text-align: right;
    }
    input[type="range"] {
        width: 100px;
        accent-color: var(--blue);
        height: 4px;
    }
</style>
