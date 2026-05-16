<script lang="ts">
    import { onMount } from 'svelte';
    import { listMcpTokens, createMcpToken, revokeMcpToken, rotateMcpToken, getAppSetting, setAppSetting, hasApiKey, setApiKey, deleteApiKey, validateApiKey, backfillRawPreviews } from '$lib/api';
    import type { McpToken } from '$lib/api';
    import { showToast } from '$lib/stores';
    import PrivacyDashboard from './PrivacyDashboard.svelte';
    import StaticPublishingSettings from './StaticPublishingSettings.svelte';

    let activeSettingsTab = $state<'general' | 'privacy' | 'static-publishing'>('general');

    let { onclose }: { onclose: () => void } = $props();

    let tokens = $state<McpToken[]>([]);
    let closeToTray = $state(true);
    let confirmTrash = $state(true);
    let httpEnabled = $state(false);
    let httpPort = $state('9847');
    let loading = $state(true);

    let showCreateForm = $state(false);
    let newName = $state('');
    let newRole = $state('admin');

    let revealedSecret = $state<string | null>(null);
    let revealedTokenName = $state('');
    let copied = $state(false);

    let autoPurge = $state(true);
    let moduleRaw = $state(false);
    let moduleStaticPublishing = $state(false);

    interface ApiKeyState {
        exists: boolean;
        inputValue: string;
        status: 'none' | 'connected' | 'invalid' | 'validating' | 'error';
    }
    const PROVIDERS = ['openai', 'google', 'openrouter'] as const;
    const PROVIDER_LABELS: Record<string, string> = { openai: 'OpenAI', google: 'Google', openrouter: 'OpenRouter' };
    const PROVIDER_PLACEHOLDERS: Record<string, string> = { openai: 'sk-...', google: 'AIza...', openrouter: 'sk-or-...' };
    let apiKeys = $state<Record<string, ApiKeyState>>({
        openai: { exists: false, inputValue: '', status: 'none' },
        google: { exists: false, inputValue: '', status: 'none' },
        openrouter: { exists: false, inputValue: '', status: 'none' },
    });

    const ROLES = [
        { value: 'viewer', label: 'Viewer', desc: 'Read-only library access' },
        { value: 'curator', label: 'Curator', desc: 'Read + rate/curate/export' },
        { value: 'operator', label: 'Operator', desc: 'Curator + import + AI' },
        { value: 'admin', label: 'Admin', desc: 'Full access' },
    ];

    onMount(async () => {
        try {
            const [toks, ctSetting, trashSetting, httpSetting, portSetting, purgeSetting, rawSetting, staticPublishingSetting] = await Promise.all([
                listMcpTokens(),
                getAppSetting('close_to_tray'),
                getAppSetting('skip_trash_confirm'),
                getAppSetting('mcp_http_enabled'),
                getAppSetting('mcp_http_port'),
                getAppSetting('auto_purge_missing'),
                getAppSetting('module_raw'),
                getAppSetting('module_static_publishing'),
            ]);
            tokens = toks;
            closeToTray = ctSetting !== 'false';
            confirmTrash = trashSetting !== 'true';
            httpEnabled = httpSetting === 'true';
            if (portSetting) httpPort = portSetting;
            autoPurge = purgeSetting === 'true';
            moduleRaw = rawSetting === 'true';
            moduleStaticPublishing = staticPublishingSetting === 'true';

            const [hasOpenai, hasGoogle, hasOpenrouter] = await Promise.all([
                hasApiKey('openai'),
                hasApiKey('google'),
                hasApiKey('openrouter'),
            ]);
            apiKeys.openai.exists = hasOpenai;
            apiKeys.openai.status = hasOpenai ? 'connected' : 'none';
            apiKeys.google.exists = hasGoogle;
            apiKeys.google.status = hasGoogle ? 'connected' : 'none';
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
        if (!moduleStaticPublishing && activeSettingsTab === 'static-publishing') {
            activeSettingsTab = 'general';
        }
        showToast(
            moduleStaticPublishing ? 'Static Publishing enabled' : 'Static Publishing disabled',
            { type: moduleStaticPublishing ? 'success' : 'info', duration: 3000 },
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

    async function handleCreate() {
        if (!newName.trim()) return;
        try {
            const [token, secret] = await createMcpToken(newName.trim(), newRole);
            tokens = [...tokens, token];
            revealedSecret = secret;
            revealedTokenName = token.name;
            showCreateForm = false;
            newName = '';
            newRole = 'admin';
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

<div class="overlay" onclick={onclose} onkeydown={(e) => e.key === 'Escape' && onclose()} role="dialog" tabindex="-1">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="panel" onclick={(e) => e.stopPropagation()} role="document">
        <div class="panel-header">
            <h2>Settings</h2>
            <button class="close-btn" onclick={onclose}>&times;</button>
        </div>

        <div class="settings-tabs">
            <button class="settings-tab" class:active={activeSettingsTab === 'general'} onclick={() => activeSettingsTab = 'general'}>General</button>
            <button class="settings-tab" class:active={activeSettingsTab === 'privacy'} onclick={() => activeSettingsTab = 'privacy'}>Privacy & Data</button>
            {#if moduleStaticPublishing}
                <button class="settings-tab" class:active={activeSettingsTab === 'static-publishing'} onclick={() => activeSettingsTab = 'static-publishing'}>Static Publishing</button>
            {/if}
        </div>

        {#if activeSettingsTab === 'privacy'}
            <div class="section">
                <PrivacyDashboard />
            </div>
        {:else if activeSettingsTab === 'static-publishing'}
            <StaticPublishingSettings />
        {:else if loading}
            <p class="loading">Loading...</p>
        {:else}
            <div class="section">
                <div class="section-header">General</div>
                <div class="setting-row">
                    <span>Close to tray</span>
                    <button class="toggle" class:on={closeToTray} onclick={toggleCloseToTray}>
                        {closeToTray ? 'ON' : 'OFF'}
                    </button>
                </div>
                <div class="setting-row">
                    <span>Confirm before Trash</span>
                    <button class="toggle" class:on={confirmTrash} onclick={toggleConfirmTrash}>
                        {confirmTrash ? 'ON' : 'OFF'}
                    </button>
                </div>
                <div class="setting-row">
                    <span>HTTP server</span>
                    <div class="row-right">
                        <button class="toggle" class:on={httpEnabled} onclick={toggleHttp}>
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
                <div class="setting-row">
                    <span>Auto-purge missing files</span>
                    <button class="toggle" class:on={autoPurge} onclick={toggleAutoPurge}>
                        {autoPurge ? 'ON' : 'OFF'}
                    </button>
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
            </div>

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
                                </div>
                                <div class="token-actions">
                                    <button class="action-btn" onclick={() => handleRotate(token.id)}>Rotate</button>
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
        background: rgba(0, 0, 0, 0.6);
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
    .role-viewer { background: rgba(122, 162, 247, 0.2); color: var(--blue); }
    .role-curator { background: rgba(224, 175, 104, 0.2); color: var(--orange); }
    .role-operator { background: rgba(187, 154, 247, 0.2); color: var(--purple); }
    .role-admin { background: rgba(247, 118, 142, 0.2); color: var(--red); }
    .token-used {
        font-size: 11px;
        color: var(--text-secondary);
    }
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
        background: rgba(158, 206, 106, 0.05);
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
