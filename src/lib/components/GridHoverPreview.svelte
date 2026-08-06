<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { untrack } from 'svelte';
    import { fade } from 'svelte/transition';
    import type { ImageWithFile } from '$lib/api';
    import type { GridHoverPreviewPlan } from '$lib/grid-hover-preview';
    import { safeAssetPreviewPath } from '$lib/view-utils';

    interface Props {
        plan: GridHoverPreviewPlan;
        items: ImageWithFile[];
        anchor: { left: number; top: number; width: number; height: number };
        sourceShape: { rows: number; cols: number };
    }

    let { plan, items, anchor, sourceShape }: Props = $props();
    const viewportInset = 12;
    const singleMaxWidth = 420;
    const singleMaxHeight = 360;
    const groupExtent = 300;
    const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    function previewColumnsForCurrentSource(): number {
        if (plan.mode !== 'group') return 1;
        if (items.length === plan.groupCount) return Math.max(1, sourceShape.cols);
        const aspect = Math.max(1, sourceShape.cols) / Math.max(1, sourceShape.rows);
        return Math.max(1, Math.ceil(Math.sqrt(items.length * aspect)));
    }

    function previewSizeForCurrentSource(): { width: number; height: number } {
        if (plan.mode === 'group' || !items[0]) {
            const aspect = Math.max(1, sourceShape.cols) / Math.max(1, sourceShape.rows);
            const width = aspect >= 1 ? groupExtent : groupExtent * aspect;
            const height = aspect >= 1 ? groupExtent / aspect : groupExtent;
            return {
                width: Math.max(1, Math.round(width)),
                height: Math.max(1, Math.round(height)),
            };
        }
        const imageWidth = Math.max(1, items[0].image.width);
        const imageHeight = Math.max(1, items[0].image.height);
        const scale = Math.min(singleMaxWidth / imageWidth, singleMaxHeight / imageHeight);
        return {
            width: Math.max(1, Math.round(imageWidth * scale)),
            height: Math.max(1, Math.round(imageHeight * scale)),
        };
    }

    function sourceRectForCurrentSource(
        sourceAnchor: { left: number; top: number; width: number; height: number },
    ): { left: number; top: number; width: number; height: number } {
        if (plan.mode === 'group') return sourceAnchor;
        const item = items[0];
        if (!item) return sourceAnchor;
        const aspect = Math.max(1, item.image.width) / Math.max(1, item.image.height);
        const width = Math.min(sourceAnchor.width, sourceAnchor.height * aspect);
        const height = Math.min(sourceAnchor.height, sourceAnchor.width / aspect);
        return {
            left: sourceAnchor.left + (sourceAnchor.width - width) / 2,
            top: sourceAnchor.top + (sourceAnchor.height - height) / 2,
            width,
            height,
        };
    }

    // A hover session gets one stable lens. Content changes inside it, but its
    // geometry does not chase every thumbnail under the pointer.
    let previewColumns = $derived(previewColumnsForCurrentSource());
    let previewRows = $derived(Math.max(1, Math.ceil(items.length / previewColumns)));
    const lensSize = previewSizeForCurrentSource();
    const initialAnchor = untrack(() => ({ ...anchor }));
    const lensLeft = Math.max(
        viewportInset,
        Math.min(
            initialAnchor.left + initialAnchor.width / 2 - lensSize.width / 2,
            window.innerWidth - lensSize.width - viewportInset,
        ),
    );
    const lensTop = Math.max(
        viewportInset,
        Math.min(
            initialAnchor.top + initialAnchor.height / 2 - lensSize.height / 2,
            window.innerHeight - lensSize.height - viewportInset,
        ),
    );
    const sourceRect = sourceRectForCurrentSource(initialAnchor);
    const lensSourceX = sourceRect.left - lensLeft;
    const lensSourceY = sourceRect.top - lensTop;
    const lensSourceScaleX = sourceRect.width / lensSize.width;
    const lensSourceScaleY = sourceRect.height / lensSize.height;

    function previewSrc(item: ImageWithFile): string {
        const path = safeAssetPreviewPath(item, { displayPx: plan.mode === 'single' ? 320 : 88, dpr: window.devicePixelRatio || 1 });
        return path ? convertFileSrc(path) : '';
    }

</script>

<aside
    class="hover-preview"
    class:group={plan.mode === 'group'}
    style:--preview-source-x={`${lensSourceX}px`}
    style:--preview-source-y={`${lensSourceY}px`}
    style:--preview-source-scale-x={lensSourceScaleX}
    style:--preview-source-scale-y={lensSourceScaleY}
    style="left: {lensLeft}px; top: {lensTop}px; width: {lensSize.width}px; height: {lensSize.height}px;"
    aria-hidden="true"
>
    {#if plan.mode === 'single' && items[0]}
        {@const src = previewSrc(items[0])}
        <div class="single-image">
            {#key items[0].image.id}
                {#if src}<img {src} alt="" transition:fade={{ duration: reduceMotion ? 0 : 120 }} />{/if}
            {/key}
        </div>
    {:else}
        {#key plan.previewKey}
            <div
                class="preview-grid"
                style:grid-template-columns={`repeat(${previewColumns}, minmax(0, 1fr))`}
                style:grid-template-rows={`repeat(${previewRows}, minmax(0, 1fr))`}
                transition:fade={{ duration: reduceMotion ? 0 : 120 }}
            >
                {#each items as item (item.image.id)}
                    {@const src = previewSrc(item)}
                    <div class="preview-cell">
                        {#if src}<img {src} alt="" />{/if}
                    </div>
                {/each}
            </div>
        {/key}
    {/if}
</aside>

<style>
    .hover-preview {
        position: fixed;
        z-index: 320;
        pointer-events: none;
        overflow: hidden;
        border-radius: calc(var(--radius) * 1.5);
        background: var(--surface);
        box-shadow: 0 22px 70px color-mix(in srgb, var(--bg) 72%, transparent);
        transform-origin: top left;
        animation: preview-enter 380ms cubic-bezier(0.16, 1, 0.3, 1) both;
        will-change: transform, opacity, filter;
    }

    @keyframes preview-enter {
        0% {
            transform: translate(var(--preview-source-x), var(--preview-source-y)) scale(var(--preview-source-scale-x), var(--preview-source-scale-y));
            opacity: 0.2;
            filter: saturate(0.82) brightness(0.86);
            box-shadow: 0 2px 8px color-mix(in srgb, var(--bg) 42%, transparent);
        }
        62% {
            opacity: 0.96;
        }
        100% {
            transform: translate(0, 0) scale(1, 1);
            opacity: 1;
            filter: saturate(1) brightness(1);
            box-shadow: 0 22px 70px color-mix(in srgb, var(--bg) 72%, transparent);
        }
    }

    @media (prefers-reduced-motion: reduce) {
        .hover-preview {
            animation: none;
        }
    }

    .single-image {
        position: relative;
        width: 100%;
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        overflow: hidden;
        background: var(--bg);
    }

    img {
        width: 100%;
        height: 100%;
        object-fit: contain;
        display: block;
    }

    .single-image img {
        position: absolute;
        inset: 0;
    }

    .preview-grid {
        position: absolute;
        inset: 0;
        display: grid;
        width: 100%;
        height: 100%;
        gap: 2px;
    }

    .preview-cell {
        overflow: hidden;
        background: var(--bg);
    }

    .preview-cell img {
        object-fit: cover;
    }

</style>
