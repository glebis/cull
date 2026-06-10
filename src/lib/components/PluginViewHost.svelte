<script lang="ts">
    // Renders the root element a plugin registered via host.mountView().
    // Used by the publish view when the cull-publish plugin owns the surface
    // (Track C3): core defers, the note says who manages the view.
    import { getRegisteredPluginViews } from '$lib/plugins/loader';

    let { pluginId, note = '' }: { pluginId: string; note?: string } = $props();

    let container = $state<HTMLElement | null>(null);
    let mounted = $state(false);

    $effect(() => {
        if (!container) return;
        const view = getRegisteredPluginViews().get(pluginId);
        if (view) {
            container.replaceChildren(view);
            mounted = true;
        } else {
            container.replaceChildren();
            mounted = false;
        }
    });
</script>

<div class="plugin-view-host">
    {#if note}
        <div class="plugin-view-note" role="note">{note}</div>
    {/if}
    <div class="plugin-view-root" bind:this={container}></div>
    {#if !mounted}
        <p class="plugin-view-missing">
            The '{pluginId}' plugin is installed but did not register a view.
        </p>
    {/if}
</div>

<style>
    .plugin-view-host {
        display: grid;
        align-content: start;
        min-height: 100%;
    }
    .plugin-view-note {
        color: var(--text-secondary);
        font-size: 11px;
        padding: 6px 20px;
        border-bottom: 1px solid var(--border);
    }
    .plugin-view-missing {
        color: var(--text-secondary);
        font-size: 12px;
        padding: 20px;
        text-align: center;
    }
</style>
