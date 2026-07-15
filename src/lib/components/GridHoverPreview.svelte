<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import type { ImageWithFile } from '$lib/api';
    import type { GridHoverPreviewPlan } from '$lib/grid-hover-preview';
    import { safeAssetPreviewPath } from '$lib/view-utils';

    interface Props {
        plan: GridHoverPreviewPlan;
        items: ImageWithFile[];
        x: number;
        y: number;
    }

    let { plan, items, x, y }: Props = $props();
    let previewWidth = $derived(plan.mode === 'single' ? 320 : 284);
    let left = $derived(Math.max(12, Math.min(x + 18, window.innerWidth - previewWidth - 12)));
    let top = $derived(Math.max(12, Math.min(y + 18, window.innerHeight - (plan.mode === 'single' ? 286 : 332))));

    function previewSrc(item: ImageWithFile): string {
        const path = safeAssetPreviewPath(item, { displayPx: plan.mode === 'single' ? 320 : 88, dpr: window.devicePixelRatio || 1 });
        return path ? convertFileSrc(path) : '';
    }

    function filename(item: ImageWithFile): string {
        return item.path.split('/').filter(Boolean).pop() ?? item.image.id;
    }
</script>

<aside
    class="hover-preview"
    class:group={plan.mode === 'group'}
    style="left: {left}px; top: {top}px; width: {previewWidth}px;"
    aria-hidden="true"
>
    {#if plan.mode === 'single' && items[0]}
        {@const src = previewSrc(items[0])}
        <div class="single-image">
            {#if src}<img {src} alt="" />{/if}
        </div>
        <div class="caption">{filename(items[0])}</div>
    {:else}
        <div class="preview-grid">
            {#each items as item (item.image.id)}
                {@const src = previewSrc(item)}
                <div class="preview-cell">
                    {#if src}<img {src} alt="" />{/if}
                </div>
            {/each}
        </div>
        <div class="caption">{plan.groupCount} images around pointer</div>
    {/if}
</aside>

<style>
    .hover-preview {
        position: fixed;
        z-index: 320;
        pointer-events: none;
        padding: 8px;
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        background: color-mix(in srgb, var(--surface) 94%, transparent);
        box-shadow: 0 16px 48px color-mix(in srgb, var(--bg) 78%, transparent), 0 0 0 1px color-mix(in srgb, var(--blue) 12%, transparent);
        backdrop-filter: blur(14px);
        transform-origin: 18px 18px;
        transition: left 180ms cubic-bezier(0.22, 1, 0.36, 1), top 180ms cubic-bezier(0.22, 1, 0.36, 1);
        animation: preview-arrive 220ms cubic-bezier(0.16, 1, 0.3, 1);
        will-change: left, top, transform, opacity;
    }

    .single-image {
        width: 100%;
        height: 232px;
        display: flex;
        align-items: center;
        justify-content: center;
        overflow: hidden;
        border-radius: var(--radius);
        background: var(--bg);
    }

    img {
        width: 100%;
        height: 100%;
        object-fit: contain;
        display: block;
    }

    .preview-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 3px;
    }

    .preview-cell {
        aspect-ratio: 1;
        overflow: hidden;
        border-radius: var(--radius);
        background: var(--bg);
    }

    .preview-cell img {
        object-fit: cover;
    }

    .caption {
        margin-top: 7px;
        overflow: hidden;
        color: var(--text-secondary);
        font-size: 10px;
        line-height: 14px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    @keyframes preview-arrive {
        from { opacity: 0; transform: translateY(7px) scale(0.94); }
        to { opacity: 1; transform: translateY(0) scale(1); }
    }

    @media (prefers-reduced-motion: reduce) {
        .hover-preview { animation-duration: 1ms; }
    }
</style>
