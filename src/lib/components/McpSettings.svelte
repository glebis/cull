<script lang="ts">
    import { onMount } from 'svelte';
    import { listMcpTokens, createMcpToken, revokeMcpToken, rotateMcpToken, getAppSetting, setAppSetting } from '$lib/api';
    import type { McpToken } from '$lib/api';

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

    const ROLES = [
        { value: 'viewer', label: 'Viewer', desc: 'Read-only library access' },
        { value: 'curator', label: 'Curator', desc: 'Read + rate/curate/export' },
        { value: 'operator', label: 'Operator', desc: 'Curator + import + AI' },
        { value: 'admin', label: 'Admin', desc: 'Full access' },
    ];

    onMount(async () => {
        try {
            const [toks, ctSetting, trashSetting, httpSetting, portSetting] = await Promise.all([
                listMcpTokens(),
                getAppSetting('close_to_tray'),
                getAppSetting('skip_trash_confirm'),
                getAppSetting('mcp_http_enabled'),
                getAppSetting('mcp_http_port'),
            ]);
            tokens = toks;
            closeToTray = ctSetting !== 'false';
            confirmTrash = trashSetting !== 'true';
            httpEnabled = httpSetting === 'true';
            if (portSetting) httpPort = portSetting;
        } catch (e) {
            console.error('Failed to load MCP settings:', e);
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
                imageview: {
                    command: "/Applications/ImageView.app/Contents/MacOS/imageview",
                    args: ["--mcp-stdio"]
                }
            }
        }, null, 2);
        navigator.clipboard.writeText(config);
    }
</script>

<div class="overlay" onclick={onclose} onkeydown={(e) => e.key === 'Escape' && onclose()} role="dialog" tabindex="-1">
    <div class="panel" onclick={(e) => e.stopPropagation()} role="document">
        <div class="panel-header">
            <h2>MCP Server</h2>
            <button class="close-btn" onclick={onclose}>&times;</button>
        </div>

        {#if loading}
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
    "imageview": {
      "command": "imageview",
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
</style>
