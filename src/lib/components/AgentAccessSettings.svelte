<script lang="ts">
    import { onMount } from 'svelte';
    import { createMcpToken, getAppSetting, listMcpTokens, revokeMcpToken, rotateMcpToken, setAppSetting, type McpToken } from '$lib/api';
    import { MCP_CONFIG_SNIPPET } from '$lib/mcp-config';
    import { showToast } from '$lib/stores';
    import { expiryState, relativeExpiry } from '$lib/token-expiry';

    const SKILL_SOURCE = 'https://github.com/glebis/claude-skills/blob/main/cull/SKILL.md';
    const INSTALL_METHODS = [
        { id: 'npx', label: 'npx', copy: 'npx skills add glebis/claude-skills --skill cull' },
        { id: 'claude', label: 'Claude', copy: 'claude plugin marketplace add glebis/claude-skills\nclaude plugin install cull@glebis-skills' },
        { id: 'codex', label: 'Codex', copy: 'Use $skill-installer to install the Cull skill from https://github.com/glebis/claude-skills/tree/main/cull' },
        { id: 'prompt', label: 'Any agent', copy: 'Install the Cull skill from https://github.com/glebis/claude-skills/tree/main/cull. Read its SKILL.md and place the skill in your agent’s skills directory.' },
    ] as const;
    const ROLES = [
        { value: 'viewer', label: 'Viewer', desc: 'Read-only library access' },
        { value: 'curator', label: 'Curator', desc: 'Read + rate/curate/export' },
        { value: 'operator', label: 'Operator', desc: 'Curator + import + AI' },
        { value: 'admin', label: 'Admin', desc: 'Full access' },
    ];
    let installMethod = $state<(typeof INSTALL_METHODS)[number]['id']>('npx');
    let tokens = $state<McpToken[]>([]);
    let httpEnabled = $state(false);
    let httpPort = $state('9847');
    let showCreate = $state(false);
    let newName = $state('');
    let newRole = $state('admin');
    let newExpiryDays = $state('90');
    let revealedSecret = $state<string | null>(null);
    let revealedTokenName = $state('');

    let selectedMethod = $derived(INSTALL_METHODS.find(method => method.id === installMethod) ?? INSTALL_METHODS[0]);

    onMount(async () => {
        const [loadedTokens, enabled, port] = await Promise.all([
            listMcpTokens(), getAppSetting('mcp_http_enabled'), getAppSetting('mcp_http_port'),
        ]);
        tokens = loadedTokens;
        httpEnabled = enabled === 'true';
        httpPort = port || httpPort;
    });

    async function copy(value: string, label: string) {
        try { await navigator.clipboard.writeText(value); showToast(`${label} copied`, { type: 'success', duration: 2500 }); }
        catch (error) { showToast('Could not copy', { detail: String(error), type: 'error', duration: 5000 }); }
    }
    async function toggleHttp() { httpEnabled = !httpEnabled; await setAppSetting('mcp_http_enabled', httpEnabled ? 'true' : 'false'); }
    async function savePort() {
        const port = Number.parseInt(httpPort, 10);
        if (port > 0 && port < 65536) await setAppSetting('mcp_http_port', httpPort);
    }
    async function createToken() {
        if (!newName.trim()) return;
        try {
            const days = Number.parseInt(newExpiryDays, 10);
            const expiresAt = Number.isFinite(days) && days > 0 ? new Date(Date.now() + days * 86_400_000).toISOString() : undefined;
            const [token, secret] = await createMcpToken(newName.trim(), newRole, undefined, expiresAt);
            tokens = [...tokens, token];
            revealedSecret = secret;
            revealedTokenName = token.name;
            showCreate = false;
            newName = '';
        } catch (error) { showToast('Could not create token', { detail: String(error), type: 'error', duration: 8000 }); }
    }
    async function revoke(id: string) {
        try { await revokeMcpToken(id); tokens = tokens.filter(token => token.id !== id); }
        catch (error) { showToast('Could not revoke token', { detail: `The token is still active. ${String(error)}`, type: 'error', duration: 10000 }); }
    }
    async function rotate(id: string) {
        try {
            revealedSecret = await rotateMcpToken(id);
            revealedTokenName = tokens.find(token => token.id === id)?.name ?? id;
            tokens = await listMcpTokens();
        } catch (error) { showToast('Could not rotate token', { detail: `The old secret is still valid. ${String(error)}`, type: 'error', duration: 10000 }); }
    }
    function formatAge(iso: string | null) {
        if (!iso) return 'never';
        const minutes = Math.floor((Date.now() - new Date(iso).getTime()) / 60000);
        if (minutes < 1) return 'just now';
        if (minutes < 60) return `${minutes}m ago`;
        if (minutes < 1440) return `${Math.floor(minutes / 60)}h ago`;
        return `${Math.floor(minutes / 1440)}d ago`;
    }
</script>

<section class="settings-section">
    <h3>Install the Cull Skill</h3>
    <p class="intro">Teach your coding agent how to search, curate, rate, and organise your Cull library. Choose an installation method; Cull only copies the command or prompt.</p>
    <div class="method-tabs" role="tablist" aria-label="Skill installation method">
        {#each INSTALL_METHODS as method}
            <button role="tab" aria-selected={installMethod === method.id} class:active={installMethod === method.id} onclick={() => installMethod = method.id}>{method.label}</button>
        {/each}
    </div>
    <div class="copy-box"><code>{selectedMethod.copy}</code><button onclick={() => copy(selectedMethod.copy, 'Installation instructions')}>Copy</button></div>
    <a href={SKILL_SOURCE} target="_blank" rel="noreferrer">View SKILL.md source ↗</a>
</section>

<section class="settings-section">
    <h3>MCP Connection <span>optional</span></h3>
    <p class="intro">Enable Cull's local HTTP endpoint when an agent cannot connect through the desktop transport.</p>
    <div class="setting-row"><span>Local HTTP endpoint</span><div class="controls"><button class:on={httpEnabled} aria-pressed={httpEnabled} onclick={toggleHttp}>{httpEnabled ? 'ON' : 'OFF'}</button>{#if httpEnabled}<input class="port" bind:value={httpPort} onblur={savePort} aria-label="MCP HTTP port" />{/if}</div></div>
    {#if httpEnabled}<p class="note">Loopback only by default. Remote access requires explicit opt-in and least-privilege tokens.</p>{/if}
</section>

{#if revealedSecret}
    <section class="settings-section secret">
        <h3>Token Secret · {revealedTokenName}</h3>
        <p class="warning">Copy this now. It will not be shown again.</p>
        <div class="copy-box"><code>{revealedSecret}</code><button onclick={() => copy(revealedSecret!, 'Token')}>Copy</button></div>
        <button class="plain" onclick={() => revealedSecret = null}>Done</button>
    </section>
{/if}

<section class="settings-section">
    <h3>Access Tokens <button class="plain" onclick={() => showCreate = !showCreate}>{showCreate ? 'Cancel' : '+ Create'}</button></h3>
    {#if showCreate}
        <div class="create-form">
            <input bind:value={newName} placeholder="Token name (e.g. Claude Code)" />
            <select bind:value={newRole}>{#each ROLES as role}<option value={role.value}>{role.label} — {role.desc}</option>{/each}</select>
            <select bind:value={newExpiryDays} aria-label="Token expiry"><option value="7">7 days</option><option value="30">30 days</option><option value="90">90 days (recommended)</option><option value="365">1 year</option><option value="">No expiry</option></select>
            <button class="primary" disabled={!newName.trim()} onclick={createToken}>Create Token</button>
        </div>
    {/if}
    {#if tokens.length === 0}<p class="note">No tokens created yet.</p>{/if}
    {#each tokens as token}
        <div class="token-row"><div><strong>{token.name}</strong><span>{token.role}</span><span>{formatAge(token.last_used_at)}</span><span class={expiryState(token.expires_at)}>{relativeExpiry(token.expires_at)}</span></div><div class="controls"><button onclick={() => rotate(token.id)}>{expiryState(token.expires_at) === 'ok' ? 'Rotate' : 'Rotate to renew'}</button><button class="danger" onclick={() => revoke(token.id)}>Revoke</button></div></div>
    {/each}
</section>

<section class="settings-section">
    <h3>Claude Code MCP Config <button class="plain" onclick={() => copy(MCP_CONFIG_SNIPPET, 'Config')}>Copy</button></h3>
    <pre>{MCP_CONFIG_SNIPPET}</pre>
</section>

<style>
    .settings-section { padding: 16px 20px; border-bottom: 1px solid var(--border); }
    h3 { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin: 0 0 12px; color: var(--text-secondary); font-size: 11px; letter-spacing: .08em; text-transform: uppercase; }
    h3 span { font-size: 9px; font-weight: 400; letter-spacing: 0; text-transform: none; }
    .intro, .note { margin: 0 0 12px; color: var(--text-secondary); font-size: 10px; line-height: 1.6; }
    .method-tabs { display: flex; gap: 4px; margin-bottom: 8px; }
    button { padding: 5px 8px; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); color: var(--text-secondary); font: 10px var(--font); cursor: pointer; }
    button.active, button.on { color: var(--green); border-color: var(--green); }
    button.plain { margin-left: auto; background: none; border: 0; color: var(--blue); }
    button.primary { color: var(--blue); }
    button.danger { color: var(--red); }
    .copy-box { display: flex; align-items: stretch; gap: 8px; margin-bottom: 8px; }
    code, pre { flex: 1; min-width: 0; margin: 0; padding: 10px; overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); color: var(--text); font: 10px/1.5 var(--font); }
    a { color: var(--blue); font-size: 10px; }
    .setting-row, .token-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; color: var(--text); font-size: 12px; }
    .controls { display: flex; align-items: center; gap: 6px; }
    input, select { box-sizing: border-box; padding: 6px 8px; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); color: var(--text); font: 10px var(--font); }
    input.port { width: 70px; }
    .create-form { display: grid; gap: 8px; margin-bottom: 12px; }
    .token-row { padding: 9px 0; border-top: 1px solid var(--border); }
    .token-row > div:first-child { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
    .token-row strong { font-size: 11px; }
    .token-row span { color: var(--text-secondary); font-size: 9px; }
    .token-row span.warn { color: var(--orange); }
    .token-row span.expired { color: var(--red); }
    .warning { color: var(--orange); font-size: 10px; }
</style>
