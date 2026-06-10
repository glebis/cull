<script lang="ts">
    import { onMount } from 'svelte';
    import { fetchPluginRegistry, installPlugin, uninstallPlugin, listInstalledPluginInfo, getAppSetting, setAppSetting } from '$lib/api';
    import type { InstalledPluginInfo, RegistryPluginInfo } from '$lib/api';
    import { grantPromptModel } from '$lib/plugins/loader';
    import type { GrantPromptModel } from '$lib/plugins/host';
    import { BUNDLED_PLUGINS } from '$lib/plugins/bundled';
    import { filterPlugins } from '$lib/plugins/plugin-search';
    import { pluginsEnabled, showToast } from '$lib/stores';

    let modulePlugins = $state(false);

    async function toggleModulePlugins() {
        await setAppSetting('module_plugins', modulePlugins ? 'true' : 'false');
        pluginsEnabled.set(modulePlugins);
        showToast(
            modulePlugins ? 'Plugins enabled — restart to load installed plugins' : 'Plugins disabled',
            { type: modulePlugins ? 'success' : 'info', duration: 4000 },
        );
    }

    let registryPlugins = $state<RegistryPluginInfo[]>([]);
    let installedPlugins = $state<InstalledPluginInfo[]>([]);
    let registryError = $state<string | null>(null);
    let loadingRegistry = $state(true);
    let registryLoading = $state(false);
    let busy = $state(false);
    let query = $state('');

    // Install consent: set when Install is clicked, surfaced as a dialog
    // listing every manifest permission BEFORE anything is downloaded.
    let consent = $state<GrantPromptModel | null>(null);

    let installedIds = $derived(new Set(installedPlugins.map(p => p.manifest.id)));
    // Ids shown in the Core group; suppress duplicate registry entries for them.
    let bundledIds = new Set(BUNDLED_PLUGINS.map(p => p.manifest.id));

    let coreManifests = $derived(filterPlugins(BUNDLED_PLUGINS.map(p => p.manifest), query));
    let filteredInstalled = $derived(filterPlugins(installedPlugins.map(p => p.manifest), query));
    let filteredInstalledRows = $derived(
        installedPlugins.filter(p => filteredInstalled.some(m => m.id === p.manifest.id)),
    );
    let filteredRegistryRows = $derived(
        filterPlugins(
            registryPlugins.map(p => p.manifest).filter(m => !bundledIds.has(m.id)),
            query,
        ).map(m => registryPlugins.find(p => p.manifest.id === m.id)!),
    );

    async function refreshInstalled() {
        try {
            installedPlugins = await listInstalledPluginInfo();
        } catch (e) {
            console.error('Failed to list installed plugins:', e);
        }
    }

    async function refreshRegistry() {
        registryLoading = true;
        try {
            registryPlugins = await fetchPluginRegistry();
            registryError = null;
        } catch (e) {
            registryError = String(e);
        }
        registryLoading = false;
        loadingRegistry = false;
    }

    onMount(async () => {
        modulePlugins = (await getAppSetting('module_plugins')) === 'true';
        await refreshInstalled();
        await refreshRegistry();
    });

    function requestInstall(plugin: RegistryPluginInfo) {
        // No download happens here: only the consent dialog opens.
        consent = grantPromptModel(plugin.manifest);
    }

    function cancelInstall() {
        consent = null;
    }

    async function confirmInstall() {
        if (!consent) return;
        const pluginId = consent.pluginId;
        busy = true;
        try {
            await installPlugin(pluginId);
            await refreshInstalled();
            showToast(`Plugin '${pluginId}' installed`, { type: 'success', duration: 3000 });
            consent = null;
        } catch (e) {
            showToast('Plugin install failed', { detail: String(e), type: 'error', duration: 6000 });
        }
        busy = false;
    }

    async function handleUninstall(pluginId: string) {
        if (!window.confirm(`Uninstall plugin '${pluginId}'? This removes its files and revokes all granted permissions.`)) {
            return;
        }
        busy = true;
        try {
            await uninstallPlugin(pluginId);
            await refreshInstalled();
            showToast(`Plugin '${pluginId}' uninstalled`, { type: 'info', duration: 3000 });
        } catch (e) {
            showToast('Plugin uninstall failed', { detail: String(e), type: 'error', duration: 6000 });
        }
        busy = false;
    }
</script>

<div class="plugin-toggle-row">
    <label style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
        <input type="checkbox" bind:checked={modulePlugins} onchange={toggleModulePlugins} />
        Plugins (Beta)
    </label>
    <span class="plugin-muted">
        Install checksum-verified plugins from the Cull registry; each plugin's permissions are shown before install
    </span>
</div>

<input
    class="plugin-search"
    type="search"
    placeholder="Search plugins…"
    bind:value={query}
    aria-label="Search plugins"
/>

{#if coreManifests.length > 0}
    <div class="plugin-group-label">Core</div>
    {#each coreManifests as core (core.id)}
        <div class="plugin-row">
            <div class="plugin-info">
                <div class="plugin-title">
                    <span class="plugin-name">{core.name}</span>
                    <span class="plugin-version">v{core.version}</span>
                </div>
                <div class="plugin-description">{core.description}</div>
                <div class="plugin-permissions">
                    {#each core.permissions as capability}
                        <span class="permission-tag">{capability}</span>
                    {/each}
                </div>
            </div>
            <span class="core-badge" title="Built-in plugin — always available">⬡ Core</span>
        </div>
    {/each}
{/if}

{#if filteredInstalledRows.length > 0}
    <div class="plugin-group-label">Installed</div>
    {#each filteredInstalledRows as installed (installed.manifest.id)}
        <div class="plugin-row">
            <div class="plugin-info">
                <div class="plugin-title">
                    <span class="plugin-name">{installed.manifest.name}</span>
                    <span class="plugin-version">v{installed.manifest.version}</span>
                </div>
                <div class="plugin-permissions">
                    {#if installed.granted.length > 0}
                        {#each installed.granted as capability}
                            <span class="permission-tag">{capability}</span>
                        {/each}
                    {:else}
                        <span class="plugin-muted">no permissions granted</span>
                    {/if}
                </div>
            </div>
            <button class="action-btn danger" disabled={busy} onclick={() => handleUninstall(installed.manifest.id)}>
                Uninstall
            </button>
        </div>
    {/each}
{/if}

<div class="plugin-group-header">
    <div class="plugin-group-label">Registry</div>
    <button class="action-btn" onclick={refreshRegistry} disabled={registryLoading}>
        {registryLoading ? 'Refreshing…' : 'Refresh'}
    </button>
</div>
{#if loadingRegistry}
    <p class="plugin-muted">Loading registry…</p>
{:else if registryError}
    <p class="plugin-error">Could not fetch the plugin registry: {registryError}</p>
{:else if filteredRegistryRows.length === 0}
    <p class="plugin-muted">No plugins in the registry yet.</p>
{:else}
    {#each filteredRegistryRows as plugin (plugin.manifest.id)}
        <div class="plugin-row">
            <div class="plugin-info">
                <div class="plugin-title">
                    <span class="plugin-name">{plugin.manifest.name}</span>
                    <span class="plugin-version">v{plugin.manifest.version}</span>
                </div>
                <div class="plugin-description">{plugin.manifest.description}</div>
                <div class="plugin-permissions">
                    {#each plugin.manifest.permissions as capability}
                        <span class="permission-tag">{capability}</span>
                    {/each}
                </div>
            </div>
            {#if installedIds.has(plugin.manifest.id)}
                <span class="plugin-installed-badge">Installed</span>
            {:else}
                <button class="action-btn" disabled={busy} onclick={() => requestInstall(plugin)}>
                    Install
                </button>
            {/if}
        </div>
    {/each}
{/if}

{#if consent}
    <div class="consent-dialog" role="alertdialog" aria-label="Plugin permissions consent">
        <div class="consent-title">Install '{consent.name}'?</div>
        <p class="consent-note">This plugin will be able to:</p>
        <ul class="consent-permissions">
            {#each consent.permissions as permission}
                <li>
                    <span class="permission-tag">{permission.capability}</span>
                    <span class="consent-description">{permission.description}</span>
                </li>
            {/each}
        </ul>
        <div class="consent-actions">
            <button class="action-btn" disabled={busy} onclick={confirmInstall}>
                {busy ? 'Installing…' : 'Install'}
            </button>
            <button class="action-btn" disabled={busy} onclick={cancelInstall}>Cancel</button>
        </div>
    </div>
{/if}

<style>
    .plugin-toggle-row {
        display: flex;
        flex-direction: column;
        gap: 4px;
        padding-bottom: var(--spacing);
        border-bottom: 1px solid var(--border);
    }
    .plugin-search {
        width: 100%;
        box-sizing: border-box;
        margin-top: var(--spacing);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        padding: 6px 8px;
        font: inherit;
        font-size: 0.85em;
    }
    .plugin-search:focus { outline: none; border-color: var(--blue); }
    .plugin-group-label {
        color: var(--text-secondary);
        font-size: 0.8em;
        text-transform: uppercase;
        letter-spacing: 0.06em;
        margin: calc(var(--spacing) * 1.5) 0 var(--spacing);
    }
    .plugin-group-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--spacing);
    }
    .core-badge {
        color: var(--purple);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 1px 6px;
        font-size: 0.75em;
        white-space: nowrap;
    }
    .plugin-row {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: var(--spacing);
        padding: var(--spacing) 0;
        border-bottom: 1px solid var(--border);
    }
    .plugin-row:last-of-type { border-bottom: none; }
    .plugin-info {
        display: flex;
        flex-direction: column;
        gap: 4px;
        min-width: 0;
    }
    .plugin-title {
        display: flex;
        align-items: baseline;
        gap: var(--spacing);
    }
    .plugin-name { color: var(--text); }
    .plugin-version { color: var(--text-secondary); font-size: 0.85em; }
    .plugin-description { color: var(--text-secondary); font-size: 0.85em; }
    .plugin-permissions {
        display: flex;
        flex-wrap: wrap;
        gap: 4px;
    }
    .permission-tag {
        color: var(--purple);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 1px 6px;
        font-size: 0.75em;
    }
    .plugin-muted { color: var(--text-secondary); font-size: 0.85em; }
    .plugin-error { color: var(--red); font-size: 0.85em; }
    .plugin-installed-badge { color: var(--green); font-size: 0.85em; }
    .action-btn {
        background: none;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--blue);
        padding: 4px 10px;
        cursor: pointer;
        font: inherit;
        font-size: 0.85em;
    }
    .action-btn:hover:not(:disabled) { border-color: var(--blue); }
    .action-btn:disabled { opacity: 0.5; cursor: default; }
    .action-btn.danger { color: var(--red); }
    .action-btn.danger:hover:not(:disabled) { border-color: var(--red); }
    .consent-dialog {
        margin-top: var(--spacing);
        border: 1px solid var(--orange);
        border-radius: var(--radius);
        padding: calc(var(--spacing) * 1.5);
        background: var(--bg);
    }
    .consent-title { color: var(--text); margin-bottom: 4px; }
    .consent-note { color: var(--text-secondary); font-size: 0.85em; margin: 4px 0; }
    .consent-permissions {
        list-style: none;
        margin: var(--spacing) 0;
        padding: 0;
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    .consent-permissions li {
        display: flex;
        align-items: baseline;
        gap: var(--spacing);
    }
    .consent-description { color: var(--text-secondary); font-size: 0.85em; }
    .consent-actions {
        display: flex;
        gap: var(--spacing);
        margin-top: var(--spacing);
    }
</style>
