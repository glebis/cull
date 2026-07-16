<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { cubicOut } from 'svelte/easing';
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

    function organicZoom(_node: Element, params: { start: number }) {
        return {
            duration: reduceMotion ? 0 : 340,
            easing: cubicOut,
            css: (t: number) => {
                const scale = params.start + (1 - params.start) * t;
                const opacity = 0.3 + 0.7 * t;
                const saturation = 0.84 + 0.16 * t;
                const brightness = 0.88 + 0.12 * t;
                return `transform: scale(${scale}); opacity: ${opacity}; filter: saturate(${saturation}) brightness(${brightness});`;
            },
        };
    }

    let previewColumns = $derived.by(() => {
        if (plan.mode !== 'group') return 1;
        if (items.length === plan.groupCount) return Math.max(1, sourceShape.cols);
        const aspect = Math.max(1, sourceShape.cols) / Math.max(1, sourceShape.rows);
        return Math.max(1, Math.ceil(Math.sqrt(items.length * aspect)));
    });
    let previewSize = $derived.by(() => {
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
    });
    let left = $derived(Math.max(
        viewportInset,
        Math.min(
            anchor.left + anchor.width / 2 - previewSize.width / 2,
            window.innerWidth - previewSize.width - viewportInset,
        ),
    ));
    let top = $derived(Math.max(
        viewportInset,
        Math.min(
            anchor.top + anchor.height / 2 - previewSize.height / 2,
            window.innerHeight - previewSize.height - viewportInset,
        ),
    ));
    let originX = $derived(anchor.left + anchor.width / 2 - left);
    let originY = $derived(anchor.top + anchor.height / 2 - top);
    let sourceScale = $derived.by(() => {
        if (plan.mode === 'group') {
            return Math.max(0.015, Math.min(
                1,
                anchor.width / previewSize.width,
                anchor.height / previewSize.height,
            ));
        }
        const item = items[0];
        if (!item) return 0.1;
        const aspect = Math.max(1, item.image.width) / Math.max(1, item.image.height);
        const containedWidth = Math.min(anchor.width, anchor.height * aspect);
        return Math.max(0.015, Math.min(1, containedWidth / previewSize.width));
    });

    function previewSrc(item: ImageWithFile): string {
        const path = safeAssetPreviewPath(item, { displayPx: plan.mode === 'single' ? 320 : 88, dpr: window.devicePixelRatio || 1 });
        return path ? convertFileSrc(path) : '';
    }

</script>

<aside
    class="hover-preview"
    class:group={plan.mode === 'group'}
    style:--preview-origin-x={`${originX}px`}
    style:--preview-origin-y={`${originY}px`}
    style="left: {left}px; top: {top}px; width: {previewSize.width}px; height: {previewSize.height}px;"
    transition:organicZoom={{ start: sourceScale }}
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
                style:grid-template-columns={`repeat(${previewColumns}, 1fr)`}
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
        transform-origin: var(--preview-origin-x) var(--preview-origin-y);
        transition: left 220ms cubic-bezier(0.22, 1, 0.36, 1),
            top 220ms cubic-bezier(0.22, 1, 0.36, 1),
            width 220ms cubic-bezier(0.22, 1, 0.36, 1),
            height 220ms cubic-bezier(0.22, 1, 0.36, 1);
        will-change: left, top, transform, opacity;
    }

    .single-image {
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

    .preview-grid {
        display: grid;
        width: 100%;
        height: 100%;
        gap: 2px;
    }

    .preview-cell {
        aspect-ratio: 1;
        overflow: hidden;
        background: var(--bg);
    }

    .preview-cell img {
        object-fit: cover;
    }

</style>
