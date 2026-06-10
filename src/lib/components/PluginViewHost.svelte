<script lang="ts">
    // Renders a plugin tab's view by invoking the tab registry entry's
    // mountView(container) callback. The plugin mounts its own Svelte view into
    // the host-provided container (see cull-publish/index.ts).
    import { get } from 'svelte/store';
    import { tabRegistry } from '$lib/plugins/tab-registry';

    let { pluginId, note = '' }: { pluginId: string; note?: string } = $props();
    let container = $state<HTMLElement | null>(null);
    let mounted = $state(false);

    $effect(() => {
        if (!container) return;
        const entry = get(tabRegistry).find(t => t.id === pluginId && t.source === 'plugin');
        if (entry?.mountView) {
            container.replaceChildren();
            entry.mountView(container);   // plugin mounts its Svelte view into the container
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
