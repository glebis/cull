<script lang="ts">
    import { onMount, tick } from 'svelte';
    import { applyAppIconVariant, getAppSetting, setAppSetting } from '$lib/api';
    import { APP_ICON_VARIANTS, DEFAULT_APP_ICON_VARIANT, normalizeAppIconVariant, type AppIconVariantId } from '$lib/app-icons';
    import { showToast } from '$lib/stores';
    import { settingsTab, type SettingsTab } from '$lib/settings-navigation';
    import AgentAccessSettings from './AgentAccessSettings.svelte';
    import AiSettings from './AiSettings.svelte';
    import GeneralSettings from './GeneralSettings.svelte';
    import PluginsSettings from './PluginsSettings.svelte';
    import PrivacyDashboard from './PrivacyDashboard.svelte';

    const TABS: { id: SettingsTab; label: string }[] = [
        { id: 'general', label: 'General' },
        { id: 'appearance', label: 'Appearance' },
        { id: 'ai', label: 'AI' },
        { id: 'agent-access', label: 'Agent Access' },
        { id: 'privacy', label: 'Privacy' },
        { id: 'plugins', label: 'Plugins' },
    ];
    let { onclose }: { onclose: () => void } = $props();
    let panelElement = $state<HTMLDivElement | null>(null);
    let appIconVariant = $state<AppIconVariantId>(DEFAULT_APP_ICON_VARIANT);

    onMount(async () => {
        void tick().then(() => panelElement?.focus());
        appIconVariant = normalizeAppIconVariant(await getAppSetting('app_icon_variant'));
    });

    async function selectAppIconVariant(variant: AppIconVariantId) {
        if (appIconVariant === variant) return;
        const previous = appIconVariant;
        appIconVariant = variant;
        try {
            await applyAppIconVariant(variant);
            await setAppSetting('app_icon_variant', variant);
            showToast(`${APP_ICON_VARIANTS.find(icon => icon.id === variant)?.label ?? 'App'} icon selected`, { type: 'success', duration: 2500 });
        } catch (error) {
            appIconVariant = previous;
            showToast('Could not apply app icon', { detail: String(error), type: 'error', duration: 5000 });
        }
    }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose} onkeydown={(event) => event.key === 'Escape' && onclose()} tabindex="-1">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="panel" bind:this={panelElement} onclick={(event) => event.stopPropagation()} role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1">
        <header class="panel-header"><h2 id="settings-title">Settings</h2><button class="close" onclick={onclose} aria-label="Close settings">&times;</button></header>
        <div class="settings-tabs" role="tablist" aria-label="Settings sections">
            {#each TABS as tab}
                <button id={`settings-tab-${tab.id}`} class="settings-tab" class:active={$settingsTab === tab.id} role="tab" aria-selected={$settingsTab === tab.id} aria-controls={`settings-panel-${tab.id}`} tabindex={$settingsTab === tab.id ? 0 : -1} onclick={() => settingsTab.set(tab.id)}>{tab.label}</button>
            {/each}
        </div>
        <div class="content" id={`settings-panel-${$settingsTab}`} role="tabpanel" aria-labelledby={`settings-tab-${$settingsTab}`}>
            {#if $settingsTab === 'general'}
                <GeneralSettings />
            {:else if $settingsTab === 'appearance'}
                <section class="appearance"><h3>Icon Color</h3><div class="icon-grid">{#each APP_ICON_VARIANTS as variant}<button class="icon-option" class:active={appIconVariant === variant.id} aria-pressed={appIconVariant === variant.id} aria-label={`Use ${variant.label} app icon`} onclick={() => selectAppIconVariant(variant.id)}><span class="icon-preview"><img src={variant.asset} alt="" /></span><span><strong>{variant.label}</strong><small>{variant.description}</small></span></button>{/each}</div></section>
            {:else if $settingsTab === 'ai'}
                <AiSettings />
            {:else if $settingsTab === 'agent-access'}
                <AgentAccessSettings />
            {:else if $settingsTab === 'privacy'}
                <section class="wrapped"><PrivacyDashboard /></section>
            {:else}
                <section class="wrapped"><PluginsSettings /></section>
            {/if}
        </div>
    </div>
</div>

<style>
    .overlay { position: fixed; inset: 0; z-index: var(--z-modal); display: flex; align-items: center; justify-content: center; background: color-mix(in srgb, var(--bg) 78%, transparent); }
    .panel { width: min(720px, calc(100vw - 32px)); max-height: min(84vh, 820px); display: grid; grid-template-rows: auto auto minmax(0, 1fr); overflow: hidden; background: var(--surface); border: 1px solid var(--border); border-radius: 8px; }
    .panel-header { display: flex; align-items: center; justify-content: space-between; padding: 16px 20px; border-bottom: 1px solid var(--border); }
    h2 { margin: 0; color: var(--text); font-size: 14px; }
    .close { padding: 0 4px; background: none; border: 0; color: var(--text-secondary); font: 18px var(--font); cursor: pointer; }
    .settings-tabs { display: flex; gap: 3px; padding: 8px 20px; overflow-x: auto; border-bottom: 1px solid var(--border); }
    .settings-tab { flex: 0 0 auto; padding: 6px 10px; background: none; border: 0; border-radius: var(--radius); color: var(--text-secondary); font: 11px var(--font); cursor: pointer; }
    .settings-tab:hover { color: var(--text); }
    .settings-tab.active { background: var(--bg); color: var(--text); }
    .content { min-height: 0; overflow-y: auto; }
    .appearance, .wrapped { padding: 16px 20px; }
    h3 { margin: 0 0 12px; color: var(--text-secondary); font-size: 11px; letter-spacing: .08em; text-transform: uppercase; }
    .icon-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
    .icon-option { display: flex; align-items: center; gap: 10px; padding: 10px; text-align: left; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); color: var(--text); font: 11px var(--font); cursor: pointer; }
    .icon-option.active { border-color: var(--blue); }
    .icon-preview { width: 38px; height: 38px; flex: 0 0 auto; }
    .icon-preview img { width: 100%; height: 100%; }
    .icon-option > span:last-child { display: grid; gap: 4px; }
    .icon-option strong { font-weight: 500; }
    .icon-option small { color: var(--text-secondary); font-size: 9px; }
    @media (max-width: 620px) { .icon-grid { grid-template-columns: 1fr; } }
</style>
