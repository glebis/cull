<script lang="ts">
    import { onMount } from 'svelte';
    import { lineageLayout, images, focusedIndex, navigateTo, activeCollection, activeFolder, collections, showToast } from '$lib/stores';
    import { listLineageGroups, getLineageGroupImages, renameLineageGroup, dissolveLineageGroup, type LineageGroup, type ImageWithFile } from '$lib/api';
    import type { LineageLayout } from '$lib/stores';

    let groups = $state<LineageGroup[]>([]);
    let groupImages = $state<Map<string, ImageWithFile[]>>(new Map());
    let selectedGroupId = $state<string | null>(null);
    let loading = $state(true);

    // Current images from the active context (collection/folder/all)
    let contextImageIds = $derived(new Set($images.map(img => img.image.id)));

    // Display label for current scope
    let scopeLabel = $derived.by(() => {
        if ($activeFolder) return $activeFolder.split('/').pop() ?? $activeFolder;
        if ($activeCollection) {
            const col = $collections.find(([id, _name, _count]) => id === $activeCollection);
            return col ? col[1] : 'Collection';
        }
        return 'All Images';
    });

    onMount(async () => {
        await loadGroups();
    });

    // Reload when folder/collection changes
    $effect(() => {
        // Track these reactive values
        void $activeFolder;
        void $activeCollection;
        void $images.length;
        loadGroups();
    });

    async function loadGroups() {
        loading = true;
        try {
            const allGroups = await listLineageGroups();
            // Filter groups to only show those with images in current context
            const filtered: LineageGroup[] = [];
            const imgMap = new Map<string, ImageWithFile[]>();

            for (const group of allGroups) {
                const imgs = await getLineageGroupImages(group.id);
                const contextImgs = imgs.filter(img => contextImageIds.has(img.image.id));
                if (contextImgs.length > 0) {
                    filtered.push({ ...group, image_count: contextImgs.length });
                    imgMap.set(group.id, contextImgs);
                }
            }

            groups = filtered;
            groupImages = imgMap;
            if (filtered.length > 0 && !selectedGroupId) {
                selectedGroupId = filtered[0].id;
            }
        } catch (e) {
            console.error('Failed to load lineage groups:', e);
        }
        loading = false;
    }

    function toggleLayout() {
        lineageLayout.update(l => l === 'timeline' ? 'comparison' : 'timeline');
    }

    function openInLoupe(index: number) {
        focusedIndex.set(index);
        navigateTo('loupe');
    }

    function openInLoupeByKey(e: KeyboardEvent, index: number) {
        if (e.key !== 'Enter' && e.key !== ' ') return;
        e.preventDefault();
        openInLoupe(index);
    }

    function findGlobalIndex(imageId: string): number {
        return $images.findIndex(img => img.image.id === imageId);
    }

    function thumbnailUrl(img: ImageWithFile): string {
        return img.thumbnail_path
            ? `asset://localhost/${img.thumbnail_path}`
            : `asset://localhost/${img.path}`;
    }

    async function handleRename(groupId: string) {
        const group = groups.find(g => g.id === groupId);
        const name = window.prompt('Rename lineage group:', group?.name ?? '');
        if (!name || !name.trim()) return;
        try {
            await renameLineageGroup(groupId, name.trim());
            await loadGroups();
        } catch (e) {
            console.error('Failed to rename group:', e);
        }
    }

    async function handleDissolve(groupId: string) {
        if (!window.confirm('Dissolve this lineage group? Images will be ungrouped.')) return;
        try {
            await dissolveLineageGroup(groupId);
            await loadGroups();
            showToast('Lineage group dissolved', { type: 'info', duration: 4000 });
        } catch (e) {
            console.error('Failed to dissolve group:', e);
        }
    }
</script>

<div class="lineage-view">
    <div class="lineage-header">
        <h2>Lineage</h2>
        <span class="scope-label" title={$activeFolder ?? $activeCollection ?? 'All images'}>{scopeLabel}</span>
        <span class="group-count">{groups.length} {groups.length === 1 ? 'group' : 'groups'}</span>
        <button class="layout-toggle" onclick={toggleLayout} title="Switch layout">
            {$lineageLayout === 'timeline' ? '⊞' : '☰'}
            {$lineageLayout === 'timeline' ? 'Comparison' : 'Timeline'}
        </button>
    </div>

    {#if loading}
        <div class="loading">Loading lineage groups...</div>
    {:else if groups.length === 0}
        <div class="empty">
            <p>No lineage groups in <strong>{scopeLabel}</strong></p>
            <p class="hint">Import multiple variants of the same image to see them grouped here.</p>
        </div>
    {:else if $lineageLayout === 'timeline'}
        <!-- TIMELINE LAYOUT -->
        <div class="timeline-container">
            {#each groups as group (group.id)}
                {@const imgs = groupImages.get(group.id) ?? []}
                <div class="timeline-strip">
                    <div class="strip-header">
                        <button class="group-name" ondblclick={() => handleRename(group.id)}>{group.name}</button>
                        <span class="group-meta">{group.image_count} variants</span>
                        {#if group.detection_method}
                            <span class="detection-badge">{group.detection_method}</span>
                        {/if}
                        <button class="strip-action" onclick={() => handleDissolve(group.id)} title="Dissolve group">{'✕'}</button>
                    </div>
                    <div class="strip-images">
                        {#each imgs as img, i (img.image.id)}
                            <div
                                class="strip-thumb"
                                onclick={() => openInLoupe(findGlobalIndex(img.image.id))}
                                onkeydown={(e) => openInLoupeByKey(e, findGlobalIndex(img.image.id))}
                                role="button"
                                tabindex="0"
                            >
                                <img
                                    src={thumbnailUrl(img)}
                                    alt=""
                                    loading="lazy"
                                />
                                {#if img.selection?.decision === 'pick'}
                                    <div class="badge pick">Pick</div>
                                {:else if img.selection?.decision === 'reject'}
                                    <div class="badge reject">Reject</div>
                                {/if}
                                {#if img.selection?.star_rating}
                                    <div class="stars">{'★'.repeat(img.selection.star_rating)}</div>
                                {/if}
                            </div>
                            {#if i < imgs.length - 1}
                                <span class="arrow">{'→'}</span>
                            {/if}
                        {/each}
                    </div>
                </div>
            {/each}
        </div>
    {:else}
        <!-- COMPARISON LAYOUT -->
        <div class="comparison-container">
            <div class="group-tabs">
                {#each groups as group (group.id)}
                    <button
                        class="group-tab"
                        class:active={selectedGroupId === group.id}
                        onclick={() => selectedGroupId = group.id}
                    >
                        {group.name}
                        <span class="tab-count">{group.image_count}</span>
                    </button>
                {/each}
            </div>

            {#if selectedGroupId}
                {@const imgs = groupImages.get(selectedGroupId) ?? []}
                <div class="comparison-grid" style="--cols: {Math.min(imgs.length, Math.ceil(Math.sqrt(imgs.length)))}">
                    {#each imgs as img (img.image.id)}
                        <div
                            class="comparison-cell"
                            onclick={() => openInLoupe(findGlobalIndex(img.image.id))}
                            onkeydown={(e) => openInLoupeByKey(e, findGlobalIndex(img.image.id))}
                            role="button"
                            tabindex="0"
                        >
                            <img
                                src={thumbnailUrl(img)}
                                alt=""
                                loading="lazy"
                            />
                            {#if img.selection?.decision === 'pick'}
                                <div class="badge pick">Pick</div>
                            {:else if img.selection?.decision === 'reject'}
                                <div class="badge reject">Reject</div>
                            {/if}
                            {#if img.selection?.star_rating}
                                <div class="stars">{'★'.repeat(img.selection.star_rating)}</div>
                            {/if}
                            <div class="cell-name">
                                {img.path.split('/').pop()}
                            </div>
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    {/if}
</div>

<style>
    .lineage-view {
        height: 100%;
        overflow-y: auto;
        padding: 16px;
    }
    .lineage-header {
        display: flex;
        align-items: center;
        gap: 12px;
        margin-bottom: 16px;
    }
    .lineage-header h2 {
        margin: 0;
        font-size: 16px;
        color: var(--text-primary, #eee);
    }
    .scope-label {
        background: var(--bg-elevated, #2a2a3e);
        color: var(--accent-warm, #e0a060);
        padding: 2px 8px;
        border-radius: 4px;
        font-size: 12px;
        font-weight: 500;
        max-width: 160px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .group-count {
        color: var(--text-secondary, #888);
        font-size: 13px;
    }
    .layout-toggle {
        margin-left: auto;
        background: var(--bg-elevated, #2a2a3e);
        border: 1px solid var(--border, #444);
        color: var(--text-secondary, #aaa);
        padding: 4px 10px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 12px;
    }
    .layout-toggle:hover { color: var(--text-primary, #eee); }

    /* Timeline */
    .timeline-strip {
        margin-bottom: 20px;
        padding: 12px;
        background: var(--bg-elevated, #1e1e2e);
        border-radius: 8px;
    }
    .strip-header {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 10px;
    }
    .group-name {
        background: none;
        border: none;
        color: var(--accent-warm, #e0a060);
        font-weight: 600;
        font-size: 13px;
        cursor: pointer;
        padding: 0;
    }
    .group-meta {
        color: var(--text-secondary, #666);
        font-size: 11px;
    }
    .detection-badge {
        background: var(--bg-hover, #333);
        color: var(--text-secondary, #888);
        padding: 1px 6px;
        border-radius: 3px;
        font-size: 10px;
    }
    .strip-action {
        margin-left: auto;
        background: none;
        border: none;
        color: var(--text-secondary, #555);
        cursor: pointer;
        font-size: 14px;
    }
    .strip-action:hover { color: var(--text-primary, #eee); }
    .strip-images {
        display: flex;
        align-items: center;
        gap: 6px;
        overflow-x: auto;
        padding-bottom: 4px;
    }
    .strip-thumb {
        position: relative;
        flex-shrink: 0;
        cursor: pointer;
        border-radius: 6px;
        overflow: hidden;
    }
    .strip-thumb img {
        display: block;
        width: 100px;
        height: 100px;
        object-fit: cover;
        border-radius: 6px;
    }
    .strip-thumb:hover img {
        opacity: 0.8;
    }
    .arrow {
        color: var(--text-secondary, #444);
        font-size: 16px;
        flex-shrink: 0;
    }
    .badge {
        position: absolute;
        top: 4px;
        left: 4px;
        padding: 1px 5px;
        border-radius: 3px;
        font-size: 9px;
        font-weight: 600;
    }
    .badge.pick { background: var(--accent, #8cc63f); color: #1a1a2e; }
    .badge.reject { background: #e04040; color: #fff; }
    .stars {
        position: absolute;
        bottom: 4px;
        left: 4px;
        color: var(--accent, #8cc63f);
        font-size: 10px;
    }

    /* Comparison */
    .group-tabs {
        display: flex;
        gap: 4px;
        margin-bottom: 16px;
        overflow-x: auto;
    }
    .group-tab {
        background: var(--bg-elevated, #2a2a3e);
        border: 1px solid var(--border, #333);
        color: var(--text-secondary, #888);
        padding: 6px 14px;
        border-radius: 6px;
        cursor: pointer;
        font-size: 12px;
        white-space: nowrap;
    }
    .group-tab.active {
        background: var(--accent-warm, #e0a060);
        color: #1a1a2e;
        border-color: var(--accent-warm, #e0a060);
    }
    .tab-count {
        margin-left: 4px;
        opacity: 0.6;
    }
    .comparison-grid {
        display: grid;
        grid-template-columns: repeat(var(--cols, 2), 1fr);
        gap: 8px;
    }
    .comparison-cell {
        position: relative;
        cursor: pointer;
        border-radius: 8px;
        overflow: hidden;
        background: var(--bg-elevated, #1e1e2e);
    }
    .comparison-cell img {
        display: block;
        width: 100%;
        aspect-ratio: 1;
        object-fit: cover;
    }
    .comparison-cell:hover img { opacity: 0.85; }
    .cell-name {
        padding: 4px 8px;
        font-size: 11px;
        color: var(--text-secondary, #888);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .loading, .empty {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        min-height: 200px;
        color: var(--text-secondary, #888);
        font-size: 14px;
    }
    .hint { font-size: 12px; color: var(--text-secondary, #666); }
</style>
