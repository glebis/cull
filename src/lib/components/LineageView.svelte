<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { onMount } from 'svelte';
    import { lineageLayout, images, focusedIndex, focusedImageOverride, navigateTo, activeCollection, activeFolder, collections, showToast, requestTextInput, requestConfirm } from '$lib/stores';
    import { listLineageGroups, getLineageGroupImages, renameLineageGroup, dissolveLineageGroup, type LineageGroup, type ImageWithFile } from '$lib/api';
    import { resolveLineageImageFocus } from '$lib/lineage-utils';
    import type { LineageLayout } from '$lib/stores';
    import { safeAssetPreviewPath } from '$lib/view-utils';
    import ContextMenu from './ContextMenu.svelte';
    import ActionMenu, { type ActionMenuItem } from './ActionMenu.svelte';

    let groups = $state<LineageGroup[]>([]);
    let groupImages = $state<Map<string, ImageWithFile[]>>(new Map());
    let selectedGroupId = $state<string | null>(null);
    let loading = $state(true);
    let contextMenu = $state<{ image: ImageWithFile; x: number; y: number } | null>(null);
    let groupContextMenu = $state<{ group: LineageGroup; x: number; y: number } | null>(null);

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
                if (contextImgs.length >= 2) {
                    filtered.push({ ...group, image_count: contextImgs.length });
                    imgMap.set(group.id, contextImgs);
                }
            }

            groups = filtered;
            groupImages = imgMap;
            if (filtered.length === 0) {
                selectedGroupId = null;
            } else if (!selectedGroupId || !imgMap.has(selectedGroupId)) {
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

    function focusLineageImage(img: ImageWithFile) {
        const target = resolveLineageImageFocus($images, img);
        if (target.focusedIndex !== null) {
            focusedIndex.set(target.focusedIndex);
        }
        focusedImageOverride.set(target.focusedImageOverride);
    }

    function openInLoupe(img: ImageWithFile) {
        focusLineageImage(img);
        navigateTo('loupe');
    }

    function openInLoupeByKey(e: KeyboardEvent, img: ImageWithFile) {
        if (e.key !== 'Enter' && e.key !== ' ') return;
        e.preventDefault();
        openInLoupe(img);
    }

    function handleImageContextMenu(e: MouseEvent, img: ImageWithFile) {
        e.preventDefault();
        e.stopPropagation();
        focusLineageImage(img);
        contextMenu = { image: img, x: e.clientX, y: e.clientY };
    }

    function thumbnailUrl(img: ImageWithFile): string {
        const previewPath = safeAssetPreviewPath(img);
        return previewPath ? convertFileSrc(previewPath) : '';
    }

    async function handleRename(groupId: string) {
        const group = groups.find(g => g.id === groupId);
        const name = await requestTextInput({
            title: 'Rename Lineage Group',
            label: 'Group name',
            initialValue: group?.name ?? '',
            confirmLabel: 'Rename',
        });
        if (!name || !name.trim()) return;
        try {
            await renameLineageGroup(groupId, name.trim());
            await loadGroups();
            showToast('Lineage group renamed', { type: 'success', duration: 4000 });
        } catch (e) {
            console.error('Failed to rename group:', e);
            showToast('Failed to rename lineage group', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleDissolve(groupId: string) {
        const confirmed = await requestConfirm({
            title: 'Dissolve Lineage Group',
            description: 'Dissolve this lineage group? Images will be ungrouped.',
            confirmLabel: 'Dissolve Group',
            danger: true,
        });
        if (!confirmed) return;
        try {
            await dissolveLineageGroup(groupId);
            await loadGroups();
            showToast('Lineage group dissolved', { type: 'info', duration: 4000 });
        } catch (e) {
            console.error('Failed to dissolve group:', e);
            showToast('Failed to dissolve lineage group', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    function contextPoint(event: MouseEvent | KeyboardEvent): { x: number; y: number } {
        if (event instanceof MouseEvent && event.type === 'contextmenu') {
            return { x: event.clientX, y: event.clientY };
        }
        const target = event.currentTarget as HTMLElement | null;
        const row = target?.closest<HTMLElement>('.strip-header, .group-tab-row') ?? target;
        const rect = row?.getBoundingClientRect();
        return rect
            ? { x: rect.left + Math.min(32, rect.width / 2), y: rect.top + Math.min(24, rect.height) }
            : { x: 16, y: 16 };
    }

    function isContextMenuKey(event: KeyboardEvent): boolean {
        return event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10');
    }

    function openGroupContextMenu(event: MouseEvent | KeyboardEvent, group: LineageGroup) {
        event.preventDefault();
        event.stopPropagation();
        groupContextMenu = { group, ...contextPoint(event) };
    }

    let groupContextItems = $derived.by((): ActionMenuItem[] => {
        const target = groupContextMenu;
        if (!target) return [];
        return [
            { id: 'lineage-rename', label: 'Rename…', action: () => handleRename(target.group.id) },
            {
                id: 'lineage-dissolve',
                label: 'Dissolve Group…',
                action: () => handleDissolve(target.group.id),
                danger: true,
                separatorBefore: true,
            },
        ];
    });
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
            <p class="hint">Lineage appears when at least two variants are visible in this scope.</p>
        </div>
    {:else if $lineageLayout === 'timeline'}
        <!-- TIMELINE LAYOUT -->
        <div class="timeline-container">
            {#each groups as group (group.id)}
                {@const imgs = groupImages.get(group.id) ?? []}
                    <div class="timeline-strip">
                    <div class="strip-header" role="group" aria-label={`Lineage group: ${group.name}`} oncontextmenu={(event) => openGroupContextMenu(event, group)}>
                        <button
                            class="group-name"
                            ondblclick={() => handleRename(group.id)}
                            onkeydown={(event) => { if (isContextMenuKey(event)) openGroupContextMenu(event, group); }}
                        >{group.name}</button>
                        <span class="group-meta">{group.image_count} variants</span>
                        {#if group.detection_method}
                            <span class="detection-badge">{group.detection_method}</span>
                        {/if}
                        <button
                            class="group-menu-button"
                            onclick={(event) => openGroupContextMenu(event, group)}
                            title="Group actions"
                            aria-label={`Group actions: ${group.name}`}
                            aria-haspopup="menu"
                        >…</button>
                    </div>
                    <div class="strip-images">
                        {#each imgs as img, i (img.image.id)}
                            {@const previewUrl = thumbnailUrl(img)}
                            <div
                                class="strip-thumb"
                                onclick={() => openInLoupe(img)}
                                onkeydown={(e) => openInLoupeByKey(e, img)}
                                oncontextmenu={(e) => handleImageContextMenu(e, img)}
                                role="button"
                                tabindex="0"
                            >
                                {#if previewUrl}
                                    <img
                                        src={previewUrl}
                                        alt=""
                                        loading="lazy"
                                    />
                                {:else}
                                    <div class="preview-unavailable">Preview unavailable</div>
                                {/if}
                                {#if img.selection?.decision === 'accept'}
                                    <div class="badge accept">Accept</div>
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
                    <div class="group-tab-row">
                        <button
                            class="group-tab"
                            class:active={selectedGroupId === group.id}
                            onclick={() => selectedGroupId = group.id}
                            oncontextmenu={(event) => openGroupContextMenu(event, group)}
                            onkeydown={(event) => { if (isContextMenuKey(event)) openGroupContextMenu(event, group); }}
                        >
                            {group.name}
                            <span class="tab-count">{group.image_count}</span>
                        </button>
                        <button
                            class="group-tab-menu-button"
                            class:active={selectedGroupId === group.id}
                            onclick={(event) => openGroupContextMenu(event, group)}
                            title="Group actions"
                            aria-label={`Group actions: ${group.name}`}
                            aria-haspopup="menu"
                        >…</button>
                    </div>
                {/each}
            </div>

            {#if selectedGroupId}
                {@const imgs = groupImages.get(selectedGroupId) ?? []}
                <div class="comparison-grid" style="--cols: {Math.min(imgs.length, Math.ceil(Math.sqrt(imgs.length)))}">
                    {#each imgs as img (img.image.id)}
                        {@const previewUrl = thumbnailUrl(img)}
                        <div
                            class="comparison-cell"
                            onclick={() => openInLoupe(img)}
                            onkeydown={(e) => openInLoupeByKey(e, img)}
                            oncontextmenu={(e) => handleImageContextMenu(e, img)}
                            role="button"
                            tabindex="0"
                        >
                            {#if previewUrl}
                                <img
                                    src={previewUrl}
                                    alt=""
                                    loading="lazy"
                                />
                            {:else}
                                <div class="preview-unavailable large">Preview unavailable</div>
                            {/if}
                            {#if img.selection?.decision === 'accept'}
                                <div class="badge accept">Accept</div>
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

{#if contextMenu}
    <ContextMenu
        image={contextMenu.image}
        x={contextMenu.x}
        y={contextMenu.y}
        onclose={() => contextMenu = null}
    />
{/if}

{#if groupContextMenu}
    <ActionMenu
        title={groupContextMenu.group.name}
        x={groupContextMenu.x}
        y={groupContextMenu.y}
        items={groupContextItems}
        onclose={() => groupContextMenu = null}
    />
{/if}

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
        color: var(--text);
    }
    .scope-label {
        background: var(--surface);
        color: var(--orange);
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
        color: var(--text-secondary);
        font-size: 13px;
    }
    .layout-toggle {
        margin-left: auto;
        background: var(--surface);
        border: 1px solid var(--border);
        color: var(--text-secondary);
        padding: 4px 10px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 12px;
    }
    .layout-toggle:hover { color: var(--text); }

    /* Timeline */
    .timeline-container {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(min(100%, 360px), 1fr));
        gap: calc(var(--spacing) * 2);
    }
    .timeline-strip {
        margin-bottom: 20px;
        padding: 12px;
        background: var(--surface);
        border-radius: 8px;
    }
    .strip-header {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 10px;
        min-width: 0;
    }
    .group-name {
        background: none;
        border: none;
        color: var(--orange);
        font-weight: 600;
        font-size: 13px;
        cursor: pointer;
        padding: 0;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .group-meta,
    .detection-badge,
    .group-menu-button {
        flex-shrink: 0;
    }
    .group-meta {
        color: var(--text-secondary);
        font-size: 11px;
    }
    .detection-badge {
        background: var(--bg);
        color: var(--text-secondary);
        padding: 1px 6px;
        border-radius: 3px;
        font-size: 10px;
    }
    .group-menu-button {
        margin-left: auto;
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        font: inherit;
        font-size: 14px;
        opacity: 0;
        padding: 2px 4px;
    }
    .timeline-strip:hover .group-menu-button,
    .timeline-strip:focus-within .group-menu-button {
        opacity: 1;
    }
    .group-menu-button:hover,
    .group-menu-button:focus-visible { color: var(--text); outline: none; }
    .strip-images {
        display: flex;
        align-items: center;
        gap: 6px;
        flex-wrap: wrap;
        overflow: hidden;
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
    .preview-unavailable {
        display: grid;
        place-items: center;
        width: 100px;
        height: 100px;
        color: var(--text-secondary);
        background: var(--surface);
        font-size: 10px;
        line-height: 1.2;
        text-align: center;
        padding: 4px;
        border-radius: 6px;
    }
    .preview-unavailable.large {
        width: 100%;
        aspect-ratio: 1;
    }
    .arrow {
        color: var(--text-secondary);
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
    .badge.accept { background: var(--green); color: var(--bg); }
    .badge.reject { background: var(--red); color: var(--bg); }
    .stars {
        position: absolute;
        bottom: 4px;
        left: 4px;
        color: var(--orange);
        font-size: 10px;
    }

    /* Comparison */
    .group-tabs {
        display: flex;
        gap: 4px;
        margin-bottom: 16px;
        overflow-x: auto;
    }
    .group-tab-row {
        display: flex;
    }
    .group-tab {
        background: var(--surface);
        border: 1px solid var(--border);
        color: var(--text-secondary);
        padding: 6px 14px;
        border-radius: 6px;
        cursor: pointer;
        font-size: 12px;
        white-space: nowrap;
    }
    .group-tab.active {
        background: var(--orange);
        color: var(--bg);
        border-color: var(--orange);
    }
    .group-tab-menu-button {
        background: var(--surface);
        border: 1px solid var(--border);
        border-left: none;
        border-radius: 0 6px 6px 0;
        color: var(--text-secondary);
        cursor: pointer;
        font: inherit;
        opacity: 0;
        padding: 6px 8px;
    }
    .group-tab-row:hover .group-tab-menu-button,
    .group-tab-row:focus-within .group-tab-menu-button {
        opacity: 1;
    }
    .group-tab-menu-button.active {
        background: var(--orange);
        border-color: var(--orange);
        color: var(--bg);
    }
    .group-tab-menu-button:hover,
    .group-tab-menu-button:focus-visible { color: var(--text); outline: none; }
    .group-tab-menu-button.active:hover,
    .group-tab-menu-button.active:focus-visible { color: var(--bg); }
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
        background: var(--surface);
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
        color: var(--text-secondary);
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
        color: var(--text-secondary);
        font-size: 14px;
    }
    .hint { font-size: 12px; color: var(--text-secondary); }
</style>
