<script lang="ts">
    import { viewMode, thumbnailSize, windowName, navigateTo } from '$lib/stores';
    import type { ViewMode } from '$lib/stores';

    const tabs: { id: ViewMode; label: string; key: string; icon: string }[] = [
        { id: 'grid', label: 'Grid', key: '⌘1', icon: '⊞' },
        { id: 'compare', label: 'Compare', key: '⌘2', icon: '⇔' },
        { id: 'loupe', label: 'Loupe', key: '⌘3', icon: '◎' },
        { id: 'canvas', label: 'Canvas', key: '⌘4', icon: '▦' },
        { id: 'lineage', label: 'Lineage', key: '⌘5', icon: '⎇' },
        { id: 'embeddings', label: 'Embeddings', key: '⌘6', icon: '⁘' },
        { id: 'export', label: 'Export', key: '⌘7', icon: '⤓' },
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
    {#if $windowName && $windowName !== 'ImageView'}
        <span class="window-name">{$windowName}</span>
    {/if}
    <div class="tabs">
        {#each tabs as tab}
            <button
                class="tab"
                class:active={$viewMode === tab.id}
                onclick={() => navigateTo(tab.id)}
            >
                <span class="tab-icon">{tab.icon}</span>{tab.label}<span class="tab-key">{tab.key}</span>
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
        height: 40px;
        background: var(--surface);
        border-bottom: 1px solid var(--border);
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0 var(--spacing);
        padding-left: 78px;
        grid-area: tabbar;
        -webkit-app-region: drag;
    }
    .window-name {
        font-size: 11px;
        color: var(--text-secondary);
        margin-right: 8px;
        padding-right: 8px;
        border-right: 1px solid var(--border);
        white-space: nowrap;
        -webkit-app-region: no-drag;
    }
    .tabs {
        display: flex;
        gap: 2px;
        -webkit-app-region: no-drag;
    }
    .tab {
        background: none;
        border: none;
        border-bottom: 2px solid transparent;
        color: var(--text-secondary);
        font-family: var(--font);
        font-size: 12px;
        padding: 8px 12px 6px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .tab:hover:not(:disabled) {
        color: var(--text);
    }
    .tab.active {
        color: var(--green);
        border-bottom-color: var(--green);
    }
    .tab-icon {
        margin-right: 4px;
        font-size: 13px;
        opacity: 0.6;
    }
    .tab.active .tab-icon {
        opacity: 1;
    }
    .tab-key {
        color: var(--text-secondary);
        font-size: 9px;
        margin-left: 4px;
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
        -webkit-app-region: no-drag;
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
