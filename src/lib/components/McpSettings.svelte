<script lang="ts">
    import { onMount, tick } from 'svelte';
    import { listMcpTokens, createMcpToken, revokeMcpToken, rotateMcpToken, getAppSetting, setAppSetting, applyAppIconVariant, hasApiKey, setApiKey, deleteApiKey, validateApiKey, backfillRawPreviews } from '$lib/api';
    import type { McpToken } from '$lib/api';
    import { APP_ICON_VARIANTS, normalizeAppIconVariant, type AppIconVariantId } from '$lib/app-icons';
    import { clientToolsEnabled, navigateTo, pluginsEnabled, showToast, staticPublishingEnabled, viewMode, voiceDictationEnabled } from '$lib/stores';
    import { CLIPBOARD_PASTE_DATE_FORMAT_SETTING, DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT } from '$lib/clipboard-actions';
    import { relativeExpiry, expiryState } from '$lib/token-expiry';
    import PrivacyDashboard from './PrivacyDashboard.svelte';
    import PluginsSettings from './PluginsSettings.svelte';

    let activeSettingsTab = $state<'general' | 'appearance' | 'privacy'>('general');

    let { onclose }: { onclose: () => void } = $props();

    let tokens = $state<McpToken[]>([]);
    let closeToTray = $state(true);
    let confirmTrash = $state(true);
    let autoUpdate = $state(true);
    let httpEnabled = $state(false);
    let httpPort = $state('9847');
    let loading = $state(true);

    let showCreateForm = $state(false);
    let newName = $state('');
    let newRole = $state('admin');
    // Expiry window in days; '90' is the SEC-004 default, '' = no expiry.
    let newExpiryDays = $state('90');

    let revealedSecret = $state<string | null>(null);
    let revealedTokenName = $state('');
    let copied = $state(false);

    let autoPurge = $state(true);
    let moduleRaw = $state(false);
    let moduleStaticPublishing = $state(false);
    let moduleClientTools = $state(false);
    let moduleVoiceDictation = $state(false);
    let modulePlugins = $state(false);
    let appIconVariant = $state<AppIconVariantId>('primary');
    let clipboardPasteDateFormat = $state(DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT);
    let cohereEmbeddingModel = $state('embed-v4.0');
    let openaiEmbeddingModel = $state('text-embedding-3-large');
    let ollamaEmbeddingUrl = $state('http://localhost:11434/api/embed');
    let ollamaEmbeddingModel = $state('embeddinggemma');
    let panelElement = $state<HTMLDivElement | null>(null);

    interface ApiKeyState {
        exists: boolean;
        inputValue: string;
        status: 'none' | 'connected' | 'invalid' | 'validating' | 'error';
    }
    const PROVIDERS = ['openai', 'google', 'cohere', 'openrouter'] as const;
    const PROVIDER_LABELS: Record<string, string> = { openai: 'OpenAI', google: 'Google', cohere: 'Cohere', openrouter: 'OpenRouter' };
    const PROVIDER_PLACEHOLDERS: Record<string, string> = { openai: 'sk-...', google: 'AIza...', cohere: 'co-...', openrouter: 'sk-or-...' };
    let apiKeys = $state<Record<string, ApiKeyState>>({
        openai: { exists: false, inputValue: '', status: 'none' },
        google: { exists: false, inputValue: '', status: 'none' },
        cohere: { exists: false, inputValue: '', status: 'none' },
        openrouter: { exists: false, inputValue: '', status: 'none' },
    });

    const ROLES = [
        { value: 'viewer', label: 'Viewer', desc: 'Read-only library access' },
        { value: 'curator', label: 'Curator', desc: 'Read + rate/curate/export' },
        { value: 'operator', label: 'Operator', desc: 'Curator + import + AI' },
        { value: 'admin', label: 'Admin', desc: 'Full access' },
    ];

    onMount(async () => {
        void tick().then(() => panelElement?.focus());
        try {
            const [toks, ctSetting, trashSetting, autoUpdateSetting, httpSetting, portSetting, purgeSetting, rawSetting, staticPublishingSetting, clientToolsSetting, voiceDictationSetting, pluginsSetting, iconSetting, clipboardPasteDateFormatSetting, cohereEmbeddingSetting, openaiEmbeddingSetting, ollamaEmbeddingUrlSetting, ollamaEmbeddingModelSetting] = await Promise.all([
                listMcpTokens(),
                getAppSetting('close_to_tray'),
                getAppSetting('skip_trash_confirm'),
                getAppSetting('auto_update_enabled'),
                getAppSetting('mcp_http_enabled'),
                getAppSetting('mcp_http_port'),
                getAppSetting('auto_purge_missing'),
                getAppSetting('module_raw'),
                getAppSetting('module_static_publishing'),
                getAppSetting('module_client_tools'),
                getAppSetting('module_voice_dictation'),
                getAppSetting('module_plugins'),
                getAppSetting('app_icon_variant'),
                getAppSetting(CLIPBOARD_PASTE_DATE_FORMAT_SETTING),
                getAppSetting('cohere_embedding_model'),
                getAppSetting('openai_embedding_model'),
                getAppSetting('ollama_embedding_url'),
                getAppSetting('ollama_embedding_model'),
            ]);
            tokens = toks;
            closeToTray = ctSetting !== 'false';
            confirmTrash = trashSetting !== 'true';
            autoUpdate = autoUpdateSetting !== 'false';
            httpEnabled = httpSetting === 'true';
            if (portSetting) httpPort = portSetting;
            autoPurge = purgeSetting === 'true';
            moduleRaw = rawSetting !== 'false';
            moduleStaticPublishing = staticPublishingSetting === 'true';
            staticPublishingEnabled.set(moduleStaticPublishing);
            moduleClientTools = clientToolsSetting === 'true';
            clientToolsEnabled.set(moduleClientTools);
            moduleVoiceDictation = voiceDictationSetting === 'true';
            voiceDictationEnabled.set(moduleVoiceDictation);
            modulePlugins = pluginsSetting === 'true';
            pluginsEnabled.set(modulePlugins);
            appIconVariant = normalizeAppIconVariant(iconSetting);
            if (clipboardPasteDateFormatSetting) clipboardPasteDateFormat = clipboardPasteDateFormatSetting;
            if (cohereEmbeddingSetting) cohereEmbeddingModel = cohereEmbeddingSetting;
            if (openaiEmbeddingSetting) openaiEmbeddingModel = openaiEmbeddingSetting;
            if (ollamaEmbeddingUrlSetting) ollamaEmbeddingUrl = ollamaEmbeddingUrlSetting;
            if (ollamaEmbeddingModelSetting) ollamaEmbeddingModel = ollamaEmbeddingModelSetting;

            const [hasOpenai, hasGoogle, hasCohere, hasOpenrouter] = await Promise.all([
                hasApiKey('openai'),
                hasApiKey('google'),
                hasApiKey('cohere'),
                hasApiKey('openrouter'),
            ]);
            apiKeys.openai.exists = hasOpenai;
            apiKeys.openai.status = hasOpenai ? 'connected' : 'none';
            apiKeys.google.exists = hasGoogle;
            apiKeys.google.status = hasGoogle ? 'connected' : 'none';
            apiKeys.cohere.exists = hasCohere;
            apiKeys.cohere.status = hasCohere ? 'connected' : 'none';
            apiKeys.openrouter.exists = hasOpenrouter;
            apiKeys.openrouter.status = hasOpenrouter ? 'connected' : 'none';
        } catch (e) {
            console.error('Failed to load settings:', e);
        }
        loading = false;
    });

    async function toggleCloseToTray() {
        closeToTray = !closeToTray;
        await setAppSetting('close_to_tray', closeToTray ? 'true' : 'false');
    }

    async function toggleConfirmTrash() {
        confirmTrash = !confirmTrash;
        await setAppSetting('skip_trash_confirm', confirmTrash ? 'false' : 'true');
    }

    async function toggleAutoUpdate() {
        autoUpdate = !autoUpdate;
        await setAppSetting('auto_update_enabled', autoUpdate ? 'true' : 'false');
        window.dispatchEvent(new CustomEvent('auto-update-setting-changed'));
    }

    async function toggleHttp() {
        httpEnabled = !httpEnabled;
        await setAppSetting('mcp_http_enabled', httpEnabled ? 'true' : 'false');
    }

    async function savePort() {
        const port = parseInt(httpPort);
        if (port > 0 && port < 65536) {
            await setAppSetting('mcp_http_port', httpPort);
        }
    }

    async function toggleAutoPurge() {
        autoPurge = !autoPurge;
        await setAppSetting('auto_purge_missing', autoPurge ? 'true' : 'false');
    }

    async function saveClipboardPasteDateFormat() {
        const format = clipboardPasteDateFormat.trim() || DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT;
        clipboardPasteDateFormat = format;
        await setAppSetting(CLIPBOARD_PASTE_DATE_FORMAT_SETTING, format);
    }

    async function selectAppIconVariant(variant: AppIconVariantId) {
        if (appIconVariant === variant) return;
        const previous = appIconVariant;
        appIconVariant = variant;
        try {
            await applyAppIconVariant(variant);
            await setAppSetting('app_icon_variant', variant);
            const selected = APP_ICON_VARIANTS.find(icon => icon.id === variant);
            showToast(`${selected?.label ?? 'App'} icon selected`, { type: 'success', duration: 2500 });
        } catch (e) {
            appIconVariant = previous;
            console.error('Failed to apply app icon:', e);
            showToast('Could not apply app icon', { detail: String(e), type: 'error', duration: 5000 });
        }
    }

    async function toggleModuleRaw() {
        await setAppSetting('module_raw', moduleRaw ? 'true' : 'false');
        if (moduleRaw) {
            showToast('RAW support enabled.', {
                type: 'success',
                duration: 10000,
                actions: [{ label: 'Rescan library', onclick: () => backfillRawPreviews() }],
            });
        }
    }

    async function toggleModuleStaticPublishing() {
        await setAppSetting('module_static_publishing', moduleStaticPublishing ? 'true' : 'false');
        staticPublishingEnabled.set(moduleStaticPublishing);
        if (!moduleStaticPublishing && $viewMode === 'publish') navigateTo('export');
        showToast(
            moduleStaticPublishing ? 'Static Publishing enabled' : 'Static Publishing disabled',
            { type: moduleStaticPublishing ? 'success' : 'info', duration: 3000 },
        );
    }

    async function toggleModuleClientTools() {
        await setAppSetting('module_client_tools', moduleClientTools ? 'true' : 'false');
        clientToolsEnabled.set(moduleClientTools);
        showToast(
            moduleClientTools ? 'Client Tools enabled' : 'Client Tools disabled',
            { type: moduleClientTools ? 'success' : 'info', duration: 3000 },
        );
    }

    async function toggleModuleVoiceDictation() {
        await setAppSetting('module_voice_dictation', moduleVoiceDictation ? 'true' : 'false');
        voiceDictationEnabled.set(moduleVoiceDictation);
        showToast(
            moduleVoiceDictation ? 'Voice Dictation enabled' : 'Voice Dictation disabled',
            { type: moduleVoiceDictation ? 'success' : 'info', duration: 3000 },
        );
    }

    async function toggleModulePlugins() {
        await setAppSetting('module_plugins', modulePlugins ? 'true' : 'false');
        pluginsEnabled.set(modulePlugins);
        showToast(
            modulePlugins ? 'Plugins enabled — restart to load installed plugins' : 'Plugins disabled',
            { type: modulePlugins ? 'success' : 'info', duration: 4000 },
        );
    }

    async function handleApiKeyBlur(provider: string) {
        const key = apiKeys[provider].inputValue.trim();
        if (!key) return;
        apiKeys[provider].status = 'validating';
        try {
            const valid = await validateApiKey(provider, key);
            if (valid) {
                await setApiKey(provider, key);
                apiKeys[provider].exists = true;
                apiKeys[provider].status = 'connected';
                apiKeys[provider].inputValue = '';
            } else {
                apiKeys[provider].status = 'invalid';
            }
        } catch {
            apiKeys[provider].status = 'error';
        }
    }

    async function handleRemoveApiKey(provider: string) {
        await deleteApiKey(provider);
        apiKeys[provider].exists = false;
        apiKeys[provider].status = 'none';
        apiKeys[provider].inputValue = '';
    }

    async function saveCohereEmbeddingModel() {
        const model = cohereEmbeddingModel.trim() || 'embed-v4.0';
        cohereEmbeddingModel = model;
        await setAppSetting('cohere_embedding_model', model);
    }

    async function saveOpenAiEmbeddingModel() {
        const model = openaiEmbeddingModel.trim() || 'text-embedding-3-large';
        openaiEmbeddingModel = model;
        await setAppSetting('openai_embedding_model', model);
    }

    async function saveOllamaEmbeddingConfig() {
        const url = ollamaEmbeddingUrl.trim() || 'http://localhost:11434/api/embed';
        const model = ollamaEmbeddingModel.trim() || 'embeddinggemma';
        ollamaEmbeddingUrl = url;
        ollamaEmbeddingModel = model;
        await Promise.all([
            setAppSetting('ollama_embedding_url', url),
            setAppSetting('ollama_embedding_model', model),
        ]);
    }

    async function handleCreate() {
        if (!newName.trim()) return;
        try {
            const days = parseInt(newExpiryDays, 10);
            const expiresAt = Number.isFinite(days) && days > 0
                ? new Date(Date.now() + days * 86_400_000).toISOString()
                : undefined;
            const [token, secret] = await createMcpToken(newName.trim(), newRole, undefined, expiresAt);
            tokens = [...tokens, token];
            revealedSecret = secret;
            revealedTokenName = token.name;
            showCreateForm = false;
            newName = '';
            newRole = 'admin';
            newExpiryDays = '90';
        } catch (e) {
            console.error('Failed to create token:', e);
        }
    }

    async function handleRevoke(id: string) {
        try {
            await revokeMcpToken(id);
            tokens = tokens.filter(t => t.id !== id);
        } catch (e) {
            console.error('Failed to revoke token:', e);
        }
    }

    async function handleRotate(id: string) {
        try {
            const newSecret = await rotateMcpToken(id);
            const token = tokens.find(t => t.id === id);
            revealedSecret = newSecret;
            revealedTokenName = token?.name ?? id;
            // Rotation grants a fresh expiry window; refresh the displayed list.
            tokens = await listMcpTokens();
        } catch (e) {
            console.error('Failed to rotate token:', e);
        }
    }

    function copySecret() {
        if (revealedSecret) {
            navigator.clipboard.writeText(revealedSecret);
            copied = true;
            setTimeout(() => { copied = false; }, 2000);
        }
    }

    function dismissSecret() {
        revealedSecret = null;
        revealedTokenName = '';
    }

    function formatAge(iso: string | null): string {
        if (!iso) return 'never';
        const diff = Date.now() - new Date(iso).getTime();
        const mins = Math.floor(diff / 60000);
        if (mins < 1) return 'just now';
        if (mins < 60) return `${mins}m ago`;
        const hours = Math.floor(mins / 60);
        if (hours < 24) return `${hours}h ago`;
        const days = Math.floor(hours / 24);
        return `${days}d ago`;
    }

    function copyConfig() {
        const config = JSON.stringify({
            mcpServers: {
                cull: {
                    command: "/Applications/Cull.app/Contents/MacOS/cull",
                    args: ["--mcp-stdio"]
                }
            }
        }, null, 2);
        navigator.clipboard.writeText(config);
    }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose} onkeydown={(e) => e.key === 'Escape' && onclose()} tabindex="-1">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div
        class="panel"
        bind:this={panelElement}
        onclick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        tabindex="-1"
    >
        <div class="panel-header">
            <h2 id="settings-title">Settings</h2>
            <button class="close-btn" onclick={onclose} aria-label="Close settings">&times;</button>
        </div>

        <div class="settings-tabs">
            <button class="settings-tab" class:active={activeSettingsTab === 'general'} onclick={() => activeSettingsTab = 'general'}>General</button>
            <button class="settings-tab" class:active={activeSettingsTab === 'appearance'} onclick={() => activeSettingsTab = 'appearance'}>Appearance</button>
            <button class="settings-tab" class:active={activeSettingsTab === 'privacy'} onclick={() => activeSettingsTab = 'privacy'}>Privacy & Data</button>
        </div>

        {#if activeSettingsTab === 'privacy'}
            <div class="section">
                <PrivacyDashboard />
            </div>
        {:else if activeSettingsTab === 'appearance'}
            <div class="section">
                <div class="section-header">Icon Color</div>
                <div class="icon-grid">
                    {#each APP_ICON_VARIANTS as variant}
                        <button
                            type="button"
                            class="icon-option"
                            class:active={appIconVariant === variant.id}
                            aria-pressed={appIconVariant === variant.id}
                            aria-label={`Use ${variant.label} app icon`}
                            onclick={() => selectAppIconVariant(variant.id)}
                        >
                            <span class="icon-preview">
                                <img src={variant.asset} alt="" />
                            </span>
                            <span class="icon-copy">
                                <span class="icon-label">{variant.label}</span>
                                <span class="icon-description">{variant.description}</span>
                            </span>
                        </button>
                    {/each}
                </div>
            </div>
        {:else if loading}
            <p class="loading">Loading...</p>
        {:else}
            <div class="section">
                <div class="section-header">General</div>
                <div class="setting-row">
                    <span>Close to tray</span>
                    <button class="toggle" class:on={closeToTray} onclick={toggleCloseToTray} aria-pressed={closeToTray}>
                        {closeToTray ? 'ON' : 'OFF'}
                    </button>
                </div>
                <div class="setting-row">
                    <span>Confirm before Trash</span>
                    <button class="toggle" class:on={confirmTrash} onclick={toggleConfirmTrash} aria-pressed={confirmTrash}>
                        {confirmTrash ? 'ON' : 'OFF'}
                    </button>
                </div>
                <div class="setting-row">
                    <span>Auto update</span>
                    <button class="toggle" class:on={autoUpdate} onclick={toggleAutoUpdate} aria-pressed={autoUpdate}>
                        {autoUpdate ? 'ON' : 'OFF'}
                    </button>
                </div>
                <div class="setting-row">
                    <span>Remote access settings</span>
                    <div class="row-right">
                        <button class="toggle" class:on={httpEnabled} onclick={toggleHttp} aria-pressed={httpEnabled}>
                            {httpEnabled ? 'ON' : 'OFF'}
                        </button>
                        {#if httpEnabled}
                            <input
                                type="text"
                                class="port-input"
                                bind:value={httpPort}
                                onblur={savePort}
                                placeholder="9847"
                            />
                        {/if}
                    </div>
                </div>
                {#if httpEnabled}
                    <div class="setting-note">
                        HTTP uses loopback by default. Non-loopback MCP requires explicit remote opt-in and scoped tokens with least privilege.
                    </div>
                {/if}
                <div class="setting-row">
                    <span>Auto-purge missing files</span>
                    <button class="toggle" class:on={autoPurge} onclick={toggleAutoPurge} aria-pressed={autoPurge}>
                        {autoPurge ? 'ON' : 'OFF'}
                    </button>
                </div>
                <div class="setting-row">
                    <span>Paste filename date</span>
                    <input
                        type="text"
                        class="date-format-input"
                        bind:value={clipboardPasteDateFormat}
                        onblur={saveClipboardPasteDateFormat}
                        placeholder={DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT}
                    />
                </div>
                <div class="setting-note">
                    Used when the destination folder has no numeric filename sequence.
                </div>
            </div>

            <div class="section">
                <div class="section-header">Modules</div>
                <div class="section-item">
                    <label style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" bind:checked={moduleRaw} onchange={toggleModuleRaw} />
                        RAW File Support
                    </label>
                    <span class="text-secondary" style="font-size: 0.85em; margin-top: 4px;">
                        Import and preview RAW camera files (RAF, CR2, NEF, ARW, DNG, etc.)
                    </span>
                </div>
                <div class="section-item">
                    <label style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" bind:checked={moduleStaticPublishing} onchange={toggleModuleStaticPublishing} />
                        Static Publishing
                    </label>
                    <span class="text-secondary" style="font-size: 0.85em; margin-top: 4px;">
                        Canvas packages, static gallery assets, Claude Code handoffs, and scheduled publish settings
                    </span>
                </div>
                <div class="section-item">
                    <label style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" bind:checked={moduleClientTools} onchange={toggleModuleClientTools} />
                        Client Tools
                    </label>
                    <span class="text-secondary" style="font-size: 0.85em; margin-top: 4px;">
                        Client delivery list export (CSV) in the command palette
                    </span>
                </div>
                <div class="section-item">
                    <label style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" bind:checked={moduleVoiceDictation} onchange={toggleModuleVoiceDictation} />
                        Voice Dictation
                    </label>
                    <span class="text-secondary" style="font-size: 0.85em; margin-top: 4px;">
                        Microphone dictation controls in the search bar
                    </span>
                </div>
                <div class="section-item">
                    <label style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" bind:checked={modulePlugins} onchange={toggleModulePlugins} />
                        Plugins (Beta)
                    </label>
                    <span class="text-secondary" style="font-size: 0.85em; margin-top: 4px;">
                        Install checksum-verified plugins from the Cull registry; each plugin's permissions are shown before install
                    </span>
                </div>
            </div>

            {#if modulePlugins}
                <div class="section">
                    <div class="section-header">Plugins</div>
                    <PluginsSettings />
                </div>
            {/if}

            <div class="section">
                <div class="section-header">API Keys</div>
                {#each PROVIDERS as provider}
                    <div class="setting-row api-key-row">
                        <span class="provider-label">{PROVIDER_LABELS[provider]}</span>
                        <div class="api-key-controls">
                            {#if apiKeys[provider].exists && !apiKeys[provider].inputValue}
                                <span class="key-status connected">&#9679; Connected</span>
                                <button class="action-btn danger" onclick={() => handleRemoveApiKey(provider)}>Remove</button>
                            {:else}
                                <input
                                    type="password"
                                    placeholder={PROVIDER_PLACEHOLDERS[provider]}
                                    bind:value={apiKeys[provider].inputValue}
                                    class="api-input"
                                    onblur={() => handleApiKeyBlur(provider)}
                                />
                                {#if apiKeys[provider].status === 'validating'}
                                    <span class="key-status validating">Validating...</span>
                                {:else if apiKeys[provider].status === 'invalid'}
                                    <span class="key-status invalid">Invalid key</span>
                                {:else if apiKeys[provider].status === 'error'}
                                    <span class="key-status invalid">Could not validate</span>
                                {/if}
                            {/if}
                        </div>
                    </div>
                {/each}
                <div class="keychain-hint">&#128274; Stored securely in system keychain</div>
            </div>

            <div class="section">
                <div class="section-header">Embedding Models</div>
                <div class="setting-row api-key-row">
                    <span class="provider-label">Cohere</span>
                    <div class="api-key-controls">
                        <input
                            type="text"
                            class="api-input"
                            bind:value={cohereEmbeddingModel}
                            onblur={saveCohereEmbeddingModel}
                            placeholder="embed-v4.0"
                        />
                    </div>
                </div>
                <div class="setting-row api-key-row">
                    <span class="provider-label">OpenAI</span>
                    <div class="api-key-controls">
                        <input
                            type="text"
                            class="api-input"
                            bind:value={openaiEmbeddingModel}
                            onblur={saveOpenAiEmbeddingModel}
                            placeholder="text-embedding-3-large"
                        />
                    </div>
                </div>
                <div class="setting-row api-key-row">
                    <span class="provider-label">Ollama URL</span>
                    <div class="api-key-controls">
                        <input
                            type="text"
                            class="api-input"
                            bind:value={ollamaEmbeddingUrl}
                            onblur={saveOllamaEmbeddingConfig}
                            placeholder="http://localhost:11434/api/embed"
                        />
                    </div>
                </div>
                <div class="setting-row api-key-row">
                    <span class="provider-label">Ollama model</span>
                    <div class="api-key-controls">
                        <input
                            type="text"
                            class="api-input"
                            bind:value={ollamaEmbeddingModel}
                            onblur={saveOllamaEmbeddingConfig}
                            placeholder="embeddinggemma"
                        />
                    </div>
                </div>
            </div>

            {#if revealedSecret}
                <div class="section secret-reveal">
                    <div class="section-header">Token Secret — {revealedTokenName}</div>
                    <p class="secret-warning">Copy this now. It will not be shown again.</p>
                    <div class="secret-box">
                        <code>{revealedSecret}</code>
                        <button onclick={copySecret}>{copied ? 'Copied' : 'Copy'}</button>
                    </div>
                    <button class="dismiss-btn" onclick={dismissSecret}>Done</button>
                </div>
            {/if}

            <div class="section">
                <div class="section-header">
                    Access Tokens
                    <button class="add-btn" onclick={() => showCreateForm = !showCreateForm}>
                        {showCreateForm ? 'Cancel' : '+ Create'}
                    </button>
                </div>

                {#if showCreateForm}
                    <div class="create-form">
                        <input
                            type="text"
                            class="name-input"
                            bind:value={newName}
                            placeholder="Token name (e.g. Claude Code)"
                        />
                        <select class="role-select" bind:value={newRole}>
                            {#each ROLES as r}
                                <option value={r.value}>{r.label} — {r.desc}</option>
                            {/each}
                        </select>
                        <label class="expiry-field">
                            <span class="expiry-label">Expires after</span>
                            <select class="expiry-select" bind:value={newExpiryDays}>
                                <option value="7">7 days</option>
                                <option value="30">30 days</option>
                                <option value="90">90 days (recommended)</option>
                                <option value="365">1 year</option>
                                <option value="">No expiry</option>
                            </select>
                        </label>
                        <button class="create-btn" onclick={handleCreate} disabled={!newName.trim()}>
                            Create Token
                        </button>
                    </div>
                {/if}

                {#if tokens.length === 0}
                    <p class="empty">No tokens created yet.</p>
                {:else}
                    <div class="token-list">
                        {#each tokens as token}
                            <div class="token-row">
                                <div class="token-info">
                                    <span class="token-name">{token.name}</span>
                                    <span class="token-role role-{token.role}">{token.role}</span>
                                    <span class="token-used">{formatAge(token.last_used_at)}</span>
                                    {#key token.expires_at}
                                        <span
                                            class="token-expiry {expiryState(token.expires_at)}"
                                            title="Rotate to renew the expiry window"
                                        >{relativeExpiry(token.expires_at)}</span>
                                    {/key}
                                </div>
                                <div class="token-actions">
                                    {#if expiryState(token.expires_at) !== 'ok'}
                                        <button class="action-btn rotate-renew" onclick={() => handleRotate(token.id)}>Rotate to renew</button>
                                    {:else}
                                        <button class="action-btn" onclick={() => handleRotate(token.id)}>Rotate</button>
                                    {/if}
                                    <button class="action-btn danger" onclick={() => handleRevoke(token.id)}>Revoke</button>
                                </div>
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>

            <div class="section">
                <div class="section-header">
                    Claude Code Config
                    <button class="add-btn" onclick={copyConfig}>Copy</button>
                </div>
                <pre class="config-snippet">{`{
  "mcpServers": {
    "cull": {
      "command": "cull",
      "args": ["--mcp-stdio"]
    }
  }
}`}</pre>
            </div>
        {/if}
    </div>
</div>

<style>
    .overlay {
        position: fixed;
        inset: 0;
        background: color-mix(in srgb, var(--bg) 78%, transparent);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
    }
    .panel {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 8px;
        width: 520px;
        max-height: 80vh;
        overflow-y: auto;
        padding: 0;
    }
    .panel-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 16px 20px;
        border-bottom: 1px solid var(--border);
    }
    .settings-tabs {
        display: flex;
        gap: 4px;
        padding: 8px 20px;
        border-bottom: 1px solid var(--border);
    }
    .settings-tab {
        padding: 6px 12px;
        background: none;
        border: none;
        border-radius: var(--radius);
        color: var(--text-secondary);
        font-family: var(--font);
        font-size: 12px;
        cursor: pointer;
    }
    .settings-tab:hover { color: var(--text); }
    .settings-tab.active { background: var(--bg); color: var(--text); }
    .panel-header h2 {
        margin: 0;
        font-size: 14px;
        font-weight: 600;
        color: var(--text);
    }
    .close-btn {
        background: none;
        border: none;
        color: var(--text-secondary);
        font-size: 18px;
        cursor: pointer;
        padding: 0 4px;
    }
    .close-btn:hover { color: var(--text); }
    .loading {
        color: var(--text-secondary);
        padding: 20px;
        text-align: center;
    }
    .section {
        padding: 16px 20px;
        border-bottom: 1px solid var(--border);
    }
    .section:last-child { border-bottom: none; }
    .section-header {
        font-size: 11px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        color: var(--text-secondary);
        margin-bottom: 12px;
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .setting-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 6px 0;
        font-size: 13px;
        color: var(--text);
    }
    .setting-note {
        margin: 2px 0 8px;
        color: var(--text-secondary);
        font-size: 11px;
        line-height: 1.4;
    }
    .setting-column {
        display: flex;
        flex-direction: column;
        gap: 6px;
        padding: 8px 0 4px;
        font-size: 13px;
        color: var(--text);
    }
    .setting-column-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
    }
    .pattern-textarea {
        width: 100%;
        min-height: 132px;
        resize: vertical;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font-family: var(--font);
        font-size: 11px;
        line-height: 1.5;
        padding: 8px 10px;
        box-sizing: border-box;
    }
    .pattern-textarea:focus {
        outline: none;
        border-color: var(--blue);
    }
    .setting-saved {
        font-size: 11px;
        color: var(--green);
    }
    .icon-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 8px;
    }
    .icon-option {
        display: flex;
        align-items: center;
        gap: 10px;
        min-width: 0;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        cursor: pointer;
        font-family: inherit;
        padding: 8px;
        text-align: left;
    }
    .icon-option:hover,
    .icon-option.active {
        border-color: var(--blue);
    }
    .icon-option.active {
        box-shadow: inset 0 0 0 1px var(--blue);
    }
    .icon-preview {
        width: 48px;
        height: 48px;
        flex: 0 0 auto;
        overflow: hidden;
        border-radius: 12px;
        background: var(--surface);
    }
    .icon-preview img {
        width: 100%;
        height: 100%;
        display: block;
    }
    .icon-copy {
        display: flex;
        flex-direction: column;
        gap: 3px;
        min-width: 0;
    }
    .icon-label {
        font-size: 12px;
        color: var(--text);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .icon-description {
        font-size: 10px;
        color: var(--text-secondary);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .row-right {
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .toggle {
        background: var(--border);
        border: none;
        border-radius: var(--radius);
        padding: 4px 12px;
        font-size: 11px;
        font-family: inherit;
        color: var(--text-secondary);
        cursor: pointer;
    }
    .toggle.on {
        background: var(--green);
        color: var(--bg);
    }
    .port-input {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 4px 8px;
        width: 60px;
        font-size: 12px;
        font-family: inherit;
        color: var(--text);
    }
    .date-format-input {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 4px 8px;
        width: 132px;
        font-size: 12px;
        font-family: inherit;
        color: var(--text);
    }
    .date-format-input:focus {
        border-color: var(--blue);
        outline: none;
    }
    .add-btn {
        background: none;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 2px 10px;
        font-size: 11px;
        font-family: inherit;
        color: var(--blue);
        cursor: pointer;
    }
    .add-btn:hover { border-color: var(--blue); }
    .create-form {
        display: flex;
        flex-direction: column;
        gap: 8px;
        margin-bottom: 12px;
    }
    .name-input, .role-select {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 8px 10px;
        font-size: 13px;
        font-family: inherit;
        color: var(--text);
    }
    .role-select option { background: var(--bg); }
    .create-btn {
        background: var(--blue);
        border: none;
        border-radius: var(--radius);
        padding: 8px 16px;
        font-size: 13px;
        font-family: inherit;
        color: var(--bg);
        cursor: pointer;
        align-self: flex-start;
    }
    .create-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .empty {
        color: var(--text-secondary);
        font-size: 13px;
        text-align: center;
        padding: 12px 0;
    }
    .token-list {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .token-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 8px;
        background: var(--bg);
        border-radius: var(--radius);
    }
    .token-info {
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .token-name {
        font-size: 13px;
        color: var(--text);
    }
    .token-role {
        font-size: 10px;
        padding: 2px 6px;
        border-radius: 3px;
        text-transform: uppercase;
        font-weight: 600;
    }
    .role-viewer { background: color-mix(in srgb, var(--blue) 20%, transparent); color: var(--blue); }
    .role-curator { background: color-mix(in srgb, var(--orange) 20%, transparent); color: var(--orange); }
    .role-operator { background: color-mix(in srgb, var(--purple) 20%, transparent); color: var(--purple); }
    .role-admin { background: color-mix(in srgb, var(--red) 20%, transparent); color: var(--red); }
    .token-used {
        font-size: 11px;
        color: var(--text-secondary);
    }
    .token-expiry {
        font-size: 11px;
        color: var(--text-secondary);
    }
    .token-expiry.warn {
        color: var(--orange);
    }
    .token-expiry.expired {
        color: var(--red);
    }
    .action-btn.rotate-renew {
        border-color: var(--orange);
        color: var(--orange);
    }
    .action-btn.rotate-renew:hover {
        border-color: var(--orange);
        background: color-mix(in srgb, var(--orange) 12%, transparent);
        color: var(--orange);
    }
    .expiry-field {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        font-size: 12px;
        color: var(--text-secondary);
    }
    .expiry-label {
        white-space: nowrap;
    }
    .expiry-select {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 6px 10px;
        font-size: 13px;
        font-family: inherit;
        color: var(--text);
        flex: 1;
    }
    .expiry-select option { background: var(--bg); }
    .token-actions {
        display: flex;
        gap: 4px;
    }
    .action-btn {
        background: none;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 3px 10px;
        font-size: 11px;
        font-family: inherit;
        color: var(--text-secondary);
        cursor: pointer;
    }
    .action-btn:hover { border-color: var(--text-secondary); color: var(--text); }
    .action-btn.danger:hover { border-color: var(--red); color: var(--red); }
    .secret-reveal {
        background: color-mix(in srgb, var(--green) 5%, transparent);
        border: 1px solid var(--green);
        border-radius: var(--radius);
        margin: 0 20px 0 20px;
    }
    .secret-warning {
        font-size: 12px;
        color: var(--orange);
        margin: 0 0 8px 0;
    }
    .secret-box {
        display: flex;
        align-items: center;
        gap: 8px;
        background: var(--bg);
        border-radius: var(--radius);
        padding: 8px 10px;
        margin-bottom: 8px;
    }
    .secret-box code {
        flex: 1;
        font-size: 11px;
        color: var(--green);
        word-break: break-all;
    }
    .secret-box button {
        background: var(--green);
        border: none;
        border-radius: var(--radius);
        padding: 4px 12px;
        font-size: 11px;
        font-family: inherit;
        color: var(--bg);
        cursor: pointer;
        white-space: nowrap;
    }
    .dismiss-btn {
        background: none;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 4px 12px;
        font-size: 11px;
        font-family: inherit;
        color: var(--text-secondary);
        cursor: pointer;
    }
    .config-snippet {
        background: var(--bg);
        border-radius: var(--radius);
        padding: 12px;
        font-size: 11px;
        color: var(--text-secondary);
        overflow-x: auto;
        margin: 0;
    }
    .api-key-row {
        flex-wrap: wrap;
    }
    .provider-label {
        min-width: 90px;
    }
    .api-key-controls {
        display: flex;
        align-items: center;
        gap: 8px;
        flex: 1;
        justify-content: flex-end;
    }
    .api-input {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 4px 8px;
        width: 180px;
        font-size: 12px;
        font-family: inherit;
        color: var(--text);
    }
    .api-input:focus {
        border-color: var(--blue);
        outline: none;
    }
    .key-status {
        font-size: 11px;
        white-space: nowrap;
    }
    .key-status.connected {
        color: var(--green);
    }
    .key-status.invalid {
        color: var(--red);
    }
    .key-status.validating {
        color: var(--orange);
    }
    .keychain-hint {
        font-size: 11px;
        color: var(--text-secondary);
        margin-top: 8px;
    }
</style>
