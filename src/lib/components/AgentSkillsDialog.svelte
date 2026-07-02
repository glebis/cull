<script lang="ts">
    import { tick } from 'svelte';
    import { openUrl } from '@tauri-apps/plugin-opener';
    import { showToast } from '$lib/stores';
    import { MCP_CONFIG_SNIPPET } from '$lib/mcp-config';

    let { onclose }: { onclose: () => void } = $props();

    let closeButton: HTMLButtonElement | undefined = $state();

    const agentDocsUrl = 'https://github.com/glebis/cull/blob/main/docs/agents.md';
    const mcpConfig = MCP_CONFIG_SNIPPET;

    const agentInstructions = `Use Cull through MCP when you need the live app, selections, snapshots, tokens, or audit logs.
Use cull --json for headless import, export, library stats, embeddings, and quality analysis.
Use the same MCP tool names and JSON fields in CLI calls where possible.
Do not rely on Cull tools as the confirmation layer for destructive operations. Get confirmation in the app, MCP client, shell wrapper, or operator workflow before file removal, token revocation, audit pruning, or broad batch changes.`;

    tick().then(() => closeButton?.focus());

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') {
            e.preventDefault();
            onclose();
        }
    }

    async function copyText(label: string, text: string) {
        try {
            await navigator.clipboard.writeText(text);
            showToast(`${label} copied`, { type: 'success', duration: 3000 });
        } catch (e) {
            showToast(`${label} copy failed`, { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function openDocs() {
        try {
            await openUrl(agentDocsUrl);
        } catch {
            window.open(agentDocsUrl, '_blank', 'noopener,noreferrer');
        }
    }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="agent-skills-overlay" onclick={onclose}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
        class="agent-skills-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="agent-skills-title"
        tabindex="-1"
        onclick={(e: MouseEvent) => e.stopPropagation()}
    >
        <header class="dialog-header">
            <div class="title-group">
                <p class="eyebrow">Agent integration</p>
                <h2 id="agent-skills-title">Install Agent Skills</h2>
                <p class="summary">
                    Add Cull to an agent as an MCP server, then give the agent the integration rules
                    for choosing MCP or the headless CLI.
                </p>
            </div>
            <button
                bind:this={closeButton}
                class="close-btn"
                type="button"
                aria-label="Close"
                onclick={onclose}
            >X</button>
        </header>

        <div class="dialog-body">
            <section class="section" aria-labelledby="agent-skills-install-title">
                <div class="section-header">
                    <h3 id="agent-skills-install-title">MCP server config</h3>
                    <button
                        class="copy-btn"
                        type="button"
                        onclick={() => copyText('MCP config', mcpConfig)}
                    >Copy</button>
                </div>
                <p class="section-copy">
                    Put this in the agent runtime's MCP config. The `cull` command opens the app in
                    tray mode if it is not already running.
                </p>
                <pre><code>{mcpConfig}</code></pre>
            </section>

            <section class="section" aria-labelledby="agent-skills-rules-title">
                <div class="section-header">
                    <h3 id="agent-skills-rules-title">Agent instructions</h3>
                    <button
                        class="copy-btn"
                        type="button"
                        onclick={() => copyText('Agent instructions', agentInstructions)}
                    >Copy</button>
                </div>
                <pre><code>{agentInstructions}</code></pre>
            </section>

            <div class="workflow-grid" aria-label="Cull agent workflow choices">
                <div>
                    <span class="meta-label">Live app</span>
                    <span class="meta-value">Use MCP for selections, snapshots, tokens, audit logs, and UI state.</span>
                </div>
                <div>
                    <span class="meta-label">Headless work</span>
                    <span class="meta-value">Use `cull --json` for import, export, stats, embeddings, and quality jobs.</span>
                </div>
                <div>
                    <span class="meta-label">Reference</span>
                    <button class="link-btn" type="button" onclick={openDocs}>Open docs/agents.md</button>
                </div>
            </div>
        </div>
    </div>
</div>

<style>
    .agent-skills-overlay {
        position: fixed;
        inset: 0;
        z-index: 12000;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: calc(var(--spacing) * 2);
        background: color-mix(in srgb, var(--bg) 76%, transparent);
    }

    .agent-skills-dialog {
        width: min(720px, 100%);
        max-height: calc(100vh - 32px);
        overflow: hidden;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        box-shadow: 0 24px 80px color-mix(in srgb, var(--bg) 86%, transparent);
    }

    .dialog-header {
        display: grid;
        grid-template-columns: minmax(0, 1fr) 32px;
        gap: calc(var(--spacing) * 2);
        align-items: start;
        padding: calc(var(--spacing) * 3);
        border-bottom: 1px solid var(--border);
    }

    .title-group {
        min-width: 0;
    }

    .eyebrow {
        margin: 0 0 4px;
        color: var(--purple);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0;
        text-transform: uppercase;
    }

    h2,
    h3,
    p {
        margin: 0;
    }

    h2 {
        color: var(--text);
        font-size: 24px;
        line-height: 1.15;
        letter-spacing: 0;
    }

    .summary,
    .section-copy,
    .meta-value {
        color: var(--text-secondary);
        font-size: 12px;
        line-height: 1.55;
    }

    .summary {
        margin-top: var(--spacing);
        color: var(--text);
        max-width: 62ch;
    }

    .close-btn,
    .copy-btn,
    .link-btn {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text-secondary);
        cursor: pointer;
        font-family: var(--font);
        font-size: 12px;
    }

    .close-btn {
        width: 32px;
        height: 32px;
        font-weight: 700;
    }

    .copy-btn,
    .link-btn {
        min-height: 30px;
        padding: 0 calc(var(--spacing) * 1.5);
    }

    .close-btn:hover,
    .close-btn:focus-visible,
    .copy-btn:hover,
    .copy-btn:focus-visible,
    .link-btn:hover,
    .link-btn:focus-visible {
        border-color: var(--blue);
        color: var(--text);
    }

    .dialog-body {
        display: flex;
        max-height: calc(100vh - 156px);
        flex-direction: column;
        gap: calc(var(--spacing) * 2);
        overflow: auto;
        padding: calc(var(--spacing) * 3);
    }

    .section {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
    }

    .section-header {
        display: flex;
        gap: var(--spacing);
        align-items: center;
        justify-content: space-between;
    }

    h3,
    .meta-label {
        color: var(--orange);
        font-size: 11px;
        font-weight: 700;
        letter-spacing: 0;
        text-transform: uppercase;
    }

    pre {
        max-width: 100%;
        overflow: auto;
        padding: calc(var(--spacing) * 1.5);
        border: 1px solid var(--border-subtle);
        border-radius: var(--radius);
        background: var(--bg);
        user-select: text;
    }

    code {
        color: var(--green);
        font-family: var(--font);
        font-size: 11px;
        line-height: 1.55;
        white-space: pre-wrap;
        overflow-wrap: anywhere;
        user-select: text;
    }

    .workflow-grid {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: var(--spacing);
    }

    .workflow-grid > div {
        min-width: 0;
        padding: calc(var(--spacing) * 1.5);
        border: 1px solid var(--border-subtle);
        border-radius: var(--radius);
        background: var(--bg);
    }

    .meta-label,
    .meta-value {
        display: block;
        min-width: 0;
        overflow-wrap: anywhere;
    }

    .meta-label {
        margin-bottom: 4px;
        color: var(--blue);
    }

    .link-btn {
        width: 100%;
        margin-top: 2px;
        text-align: left;
    }

    @media (max-width: 720px) {
        .dialog-header,
        .dialog-body {
            padding: calc(var(--spacing) * 2);
        }

        .workflow-grid {
            grid-template-columns: 1fr;
        }

        h2 {
            font-size: 22px;
        }
    }
</style>
