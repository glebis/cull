<script lang="ts">
    import type { ViewTabIconId } from '$lib/view-tabs';

    interface Props {
        icon: ViewTabIconId;
    }

    let { icon }: Props = $props();
</script>

<span class="view-tab-icon" data-icon={icon} aria-hidden="true">
    {#if icon === 'grid-contact-sheet'}
        <svg viewBox="0 0 24 24" focusable="false">
            <rect class="soft-fill" x="4" y="5" width="5.5" height="5" rx="1" />
            <rect x="4" y="5" width="5.5" height="5" rx="1" />
            <rect x="12.5" y="5" width="7.5" height="5" rx="1" />
            <rect x="4" y="14" width="7.5" height="5" rx="1" />
            <rect x="14.5" y="14" width="5.5" height="5" rx="1" />
            <path class="detail" d="M12.5 10h7.5M4 19h7.5M14.5 19H20" />
        </svg>
    {:else if icon === 'loupe-focus'}
        <svg viewBox="0 0 24 24" focusable="false">
            <circle cx="10.5" cy="10.5" r="5.5" />
            <path d="M14.6 14.6 20 20" />
            <path class="detail" d="M8.1 10.5h4.8M10.5 8.1v4.8" />
            <path class="soft-fill" d="M7.3 6.9a5.5 5.5 0 0 1 6.3 0" />
        </svg>
    {:else if icon === 'compare-split'}
        <svg viewBox="0 0 24 24" focusable="false">
            <rect x="4" y="5" width="7" height="14" rx="1.5" />
            <rect x="13" y="5" width="7" height="14" rx="1.5" />
            <path class="detail" d="M8 9v6M16 9v6" />
        </svg>
    {:else if icon === 'canvas-board'}
        <svg viewBox="0 0 24 24" focusable="false">
            <rect x="4" y="5" width="16" height="14" rx="1.5" />
            <path class="detail" d="M8 9h5M8 12h8M8 15h4" />
            <circle class="soft-fill" cx="17" cy="15" r="1.6" />
        </svg>
    {:else if icon === 'lineage-branch'}
        <svg viewBox="0 0 24 24" focusable="false">
            <circle cx="6" cy="12" r="2" />
            <circle cx="17.5" cy="7" r="2" />
            <circle cx="17.5" cy="17" r="2" />
            <path d="M8 12h3.2c1.5 0 2.1-5 4.3-5" />
            <path d="M8 12h3.2c1.5 0 2.1 5 4.3 5" />
        </svg>
    {:else if icon === 'embedding-map'}
        <svg viewBox="0 0 24 24" focusable="false">
            <circle cx="7" cy="8" r="1.7" />
            <circle cx="15.5" cy="6.5" r="1.7" />
            <circle cx="17.5" cy="15.5" r="1.7" />
            <circle cx="9.5" cy="17" r="1.7" />
            <path class="detail" d="M8.7 7.7 13.8 7M16 8.2l1 5.5M15.8 16l-4.6.7" />
        </svg>
    {:else if icon === 'publish-launch'}
        <svg viewBox="0 0 24 24" focusable="false">
            <path d="M6 18h12" />
            <path d="M12 17V6" />
            <path d="M8.5 9.5 12 6l3.5 3.5" />
            <path class="detail" d="M6.5 14.5v3.5h11v-3.5" />
        </svg>
    {:else if icon === 'export-package'}
        <svg viewBox="0 0 24 24" focusable="false">
            <path d="M6 15.5v3h12v-3" />
            <path d="M12 5v10" />
            <path d="M8.5 11.5 12 15l3.5-3.5" />
            <path class="detail" d="M7.5 8.5h9" />
        </svg>
    {/if}
</span>

<style>
    .view-tab-icon {
        width: 16px;
        height: 16px;
        display: inline-grid;
        place-items: center;
        flex: 0 0 16px;
        color: currentColor;
        opacity: 0.68;
    }

    :global(.tab.active) .view-tab-icon {
        opacity: 1;
    }

    svg {
        width: 16px;
        height: 16px;
        display: block;
        fill: none;
        stroke: currentColor;
        stroke-width: 1.65;
        stroke-linecap: round;
        stroke-linejoin: round;
        shape-rendering: geometricPrecision;
    }

    .soft-fill {
        fill: currentColor;
        stroke: none;
        opacity: 0.18;
    }

    .detail {
        opacity: 0.55;
    }
</style>
