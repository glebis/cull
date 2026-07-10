<!-- Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author. -->
<!-- Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md. -->
<script lang="ts">
    import { onMount } from 'svelte';
    import { revealItemInDir } from '@tauri-apps/plugin-opener';
    import { getDataFlowStatus, getApiAuditLog, exportAuditLog, getMcpAuditLog } from '$lib/api';
    import { showToast } from '$lib/stores';
    import type { DataFlowEntry, AuditLogEntry, McpAuditEntry } from '$lib/api';

    let flowStatus = $state<DataFlowEntry[]>([]);
    let auditLog = $state<AuditLogEntry[]>([]);
    let mcpAudit = $state<McpAuditEntry[]>([]);
    let expandedEntry = $state<string | null>(null);
    let historyOpen = $state(true);
    let agentLogOpen = $state(true);
    let potentialOpen = $state(false);
    let loading = $state(true);

    // Count of failed-auth attempts surfaced from the agent access log.
    const authFailedCount = $derived(
        mcpAudit.filter(e => e.tool_name === '_auth_failed').length,
    );

    const STATUS_COLORS: Record<string, string> = {
        local: 'var(--green)',
        active: 'var(--red)',
        configured: 'var(--orange)',
        off: 'var(--text-secondary)',
    };

    const PROVIDER_TERMS_NOTICE = 'Verify current provider terms before regulated or sensitive use.';

    const PROVIDER_COMPLIANCE: Record<string, {
        company: string;
        certifications: string[];
        gdpr: string;
        training: string;
        tos: string;
        retention: string;
    }> = {
        gemini: {
            company: 'Google LLC, Mountain View CA',
            certifications: ['SOC 1/2/3', 'ISO 27001', 'ISO 27017', 'ISO 27018'],
            gdpr: 'DPA available via Google Cloud terms',
            training: 'Tier and account dependent; verify Google AI or Google Cloud terms before use',
            tos: 'https://ai.google.dev/gemini-api/terms',
            retention: 'Tier and account dependent; verify current provider terms',
        },
        openai: {
            company: 'OpenAI Inc, San Francisco CA',
            certifications: ['SOC 2 Type II'],
            gdpr: 'DPA and residency options depend on account, product, and current terms',
            training: 'API data controls are account and product dependent; verify current policy',
            tos: 'https://openai.com/policies/terms-of-use/',
            retention: 'Retention controls are account and product dependent; verify current policy',
        },
        cohere: {
            company: 'Cohere, Toronto CA / San Francisco CA',
            certifications: ['SOC 2 Type II'],
            gdpr: 'DPA available for business plans',
            training: 'No training on API data by default',
            tos: 'https://cohere.com/terms-of-use',
            retention: 'See Cohere data usage policy',
        },
        openrouter: {
            company: 'OpenRouter (US)',
            certifications: ['SOC 2 (Enterprise only)'],
            gdpr: 'Claims compliance. No public DPA.',
            training: 'Proxy — depends on downstream provider',
            tos: 'https://openrouter.ai/terms',
            retention: 'Routing and retention depend on downstream provider and selected route',
        },
        ollama: {
            company: 'Local inference (your machine)',
            certifications: [],
            gdpr: 'N/A — fully local',
            training: 'No data collection',
            tos: 'https://ollama.com/privacy',
            retention: 'Local only',
        },
    };

    onMount(async () => {
        try {
            const [status, log, mcp] = await Promise.all([
                getDataFlowStatus(),
                getApiAuditLog(20),
                getMcpAuditLog(30),
            ]);
            flowStatus = status;
            auditLog = log;
            mcpAudit = mcp;
        } catch (e) {
            console.error('Privacy dashboard load error:', e);
        }
        loading = false;
    });

    function formatBytes(bytes: number | null): string {
        if (bytes == null) return '—';
        if (bytes < 1024) return `${bytes} B`;
        if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
        return `${(bytes / 1048576).toFixed(1)} MB`;
    }

    function formatTime(ts: string): string {
        const d = new Date(ts);
        return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }

    function formatDate(ts: string): string {
        const d = new Date(ts);
        return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
    }

    function toolLabel(name: string): string {
        return name === '_auth_failed' ? 'auth failed' : name;
    }

    function actorLabel(entry: McpAuditEntry): string {
        if (entry.token_name) return entry.token_name;
        if (entry.token_id?.startsWith('plugin:')) return `Plugin ${entry.token_id.slice('plugin:'.length)}`;
        if (entry.token_id) return entry.token_id;
        return entry.tool_name === '_auth_failed' ? 'Unknown token' : 'Cull UI';
    }

    function actorRole(entry: McpAuditEntry): string {
        if (entry.token_role) return entry.token_role;
        if (entry.token_id?.startsWith('plugin:')) return 'plugin';
        if (entry.token_id) return 'token';
        return entry.tool_name === '_auth_failed' ? 'unauthorized' : 'local';
    }

    async function handleExport() {
        try {
            const path = await exportAuditLog();
            const fileName = path.split(/[\\/]/).pop() ?? 'audit log';
            const reveal = () => revealItemInDir(path);
            const revealAction = () => {
                void reveal();
            };
            try {
                await reveal();
                showToast('Audit log exported', {
                    detail: fileName,
                    type: 'success',
                    actions: [{ label: 'Reveal', onclick: revealAction }],
                });
            } catch (revealError) {
                showToast('Audit log exported', {
                    detail: `${fileName}. Reveal failed: ${String(revealError)}`,
                    type: 'warning',
                    duration: 10000,
                    actions: [{ label: 'Reveal', onclick: revealAction }],
                });
            }
        } catch (e) {
            console.error('Export error:', e);
            showToast('Audit log export failed', {
                detail: String(e),
                type: 'error',
                duration: 10000,
            });
        }
    }
</script>

<div class="privacy-dashboard">
    <h3 class="section-header">Privacy & Data</h3>
    <p class="privacy-note">{PROVIDER_TERMS_NOTICE}</p>

    {#if loading}
        <p class="loading">Loading...</p>
    {:else}
        <div class="flow-section">
            <h4 class="subsection-header">Data Flow Status</h4>
            <table class="flow-table">
                <thead>
                    <tr>
                        <th>Feature</th>
                        <th>Status</th>
                        <th>Server</th>
                        <th>Data Sent</th>
                    </tr>
                </thead>
                <tbody>
                    {#each flowStatus as entry}
                        <tr>
                            <td>{entry.feature}</td>
                            <td>
                                <span class="status-dot" style="background: {STATUS_COLORS[entry.status] || 'var(--text-secondary)'}"></span>
                                <span class="status-label">{entry.status}</span>
                            </td>
                            <td class="server-cell">{entry.server}</td>
                            <td class="data-cell">{entry.data_sent}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>

            {#if flowStatus.some(e => e.feature === 'Gemini embeddings' && e.status === 'active')}
                <div class="gemini-warning">
                    Gemini free tier: Google may use your images for model training. Use paid tier for privacy.
                </div>
            {/if}
        </div>

        <div class="history-section">
            <button class="collapse-toggle" onclick={() => historyOpen = !historyOpen}>
                <span class="arrow">{historyOpen ? '▼' : '▶'}</span>
                API Call History ({auditLog.length})
            </button>

            {#if historyOpen}
                <div class="audit-list">
                    {#if auditLog.length === 0}
                        <p class="empty-state">No API calls recorded yet.</p>
                    {:else}
                        {#each auditLog as entry}
                            <div class="audit-entry" class:expanded={expandedEntry === entry.id}>
                                <button class="audit-row" onclick={() => expandedEntry = expandedEntry === entry.id ? null : entry.id}>
                                    <span class="audit-time">{formatDate(entry.timestamp)} {formatTime(entry.timestamp)}</span>
                                    <span class="audit-provider">{entry.provider}</span>
                                    <span class="audit-size">{formatBytes(entry.data_size_bytes)}</span>
                                    <span class="audit-status" class:success={entry.response_status === 200}>
                                        {entry.response_status === 200 ? '✓' : entry.response_status ?? '?'}
                                    </span>
                                    <span class="arrow">{expandedEntry === entry.id ? '▼' : '▶'}</span>
                                </button>

                                {#if expandedEntry === entry.id}
                                    <div class="audit-details">
                                        <div class="detail-row">
                                            <span class="detail-label">Endpoint</span>
                                            <span class="detail-value endpoint">{entry.endpoint}</span>
                                        </div>
                                        <div class="detail-row">
                                            <span class="detail-label">Data sent</span>
                                            <span class="detail-value">
                                                {#if entry.data_type === 'image'}1 image ({formatBytes(entry.data_size_bytes)}{entry.image_dimensions ? `, ${entry.image_dimensions}` : ''}){/if}
                                                {#if entry.data_type === 'prompt'}Prompt ({formatBytes(entry.data_size_bytes)}){/if}
                                                {#if entry.data_type === 'prompt+image'}Prompt + source image{/if}
                                            </span>
                                        </div>
                                        {#if entry.prompt_preview}
                                            <div class="detail-row">
                                                <span class="detail-label">Prompt</span>
                                                <span class="detail-value prompt-preview">"{entry.prompt_preview}"</span>
                                            </div>
                                        {/if}
                                        {#if entry.model}
                                            <div class="detail-row">
                                                <span class="detail-label">Model</span>
                                                <span class="detail-value">{entry.model}</span>
                                            </div>
                                        {/if}
                                        <div class="detail-row">
                                            <span class="detail-label">Jurisdiction</span>
                                            <span class="detail-value">{entry.jurisdiction}</span>
                                        </div>
                                        {#if PROVIDER_COMPLIANCE[entry.provider]}
                                            {@const comp = PROVIDER_COMPLIANCE[entry.provider]}
                                            <div class="detail-row">
                                                <span class="detail-label">Company</span>
                                                <span class="detail-value">{comp.company}</span>
                                            </div>
                                            {#if comp.certifications.length > 0}
                                                <div class="detail-row">
                                                    <span class="detail-label">Certifications</span>
                                                    <span class="detail-value certs">{comp.certifications.join(', ')}</span>
                                                </div>
                                            {/if}
                                            <div class="detail-row">
                                                <span class="detail-label">GDPR</span>
                                                <span class="detail-value">{comp.gdpr}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span class="detail-label">Training</span>
                                                <span class="detail-value" class:training-warning={comp.training.includes('Yes')}>{comp.training}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span class="detail-label">Retention</span>
                                                <span class="detail-value">{comp.retention}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span class="detail-label">ToS</span>
                                                <a class="detail-value tos-link" href={comp.tos} target="_blank">{comp.tos}</a>
                                            </div>
                                        {/if}
                                    </div>
                                {/if}
                            </div>
                        {/each}
                    {/if}
                </div>
            {/if}
        </div>

        <div class="agent-log-section">
            <button class="collapse-toggle" onclick={() => agentLogOpen = !agentLogOpen}>
                <span class="arrow">{agentLogOpen ? '▼' : '▶'}</span>
                Agent Access Log ({mcpAudit.length})
                {#if authFailedCount > 0}
                    <span class="auth-failed-badge">{authFailedCount} failed auth</span>
                {/if}
            </button>

            {#if agentLogOpen}
                <div class="audit-list">
                    {#if mcpAudit.length === 0}
                        <p class="empty-state">No agent or token activity recorded yet.</p>
                    {:else}
                        {#each mcpAudit as entry}
                            <div class="agent-row" class:failed={entry.result_status !== 'ok'}>
                                <span class="audit-time">{formatDate(entry.timestamp)} {formatTime(entry.timestamp)}</span>
                                <span class="agent-actor">
                                    <strong>{actorLabel(entry)}</strong>
                                    <em>{actorRole(entry)}</em>
                                </span>
                                <span class="agent-tool" class:auth-failed={entry.tool_name === '_auth_failed'}>{toolLabel(entry.tool_name)}</span>
                                <span class="agent-status" class:ok={entry.result_status === 'ok'}>{entry.result_status}</span>
                            </div>
                        {/each}
                    {/if}
                </div>
            {/if}
        </div>

        <div class="footer-sections">
            <button class="collapse-toggle" onclick={() => potentialOpen = !potentialOpen}>
                <span class="arrow">{potentialOpen ? '▼' : '▶'}</span>
                What could leave this machine
            </button>
            {#if potentialOpen}
                <div class="potential-list">
                    {#each flowStatus.filter(e => e.status !== 'local' && e.status !== 'off') as entry}
                        <div class="potential-item">
                            <strong>{entry.feature}</strong>: {entry.data_sent} → {entry.server}
                        </div>
                    {/each}
                    {#if flowStatus.filter(e => e.status !== 'local' && e.status !== 'off').length === 0}
                        <p class="safe-state">No cloud APIs configured. All data stays on your machine.</p>
                    {/if}
                </div>
            {/if}

            <button class="collapse-toggle" onclick={handleExport}>
                <span class="arrow">↓</span>
                Export audit log (JSON)
            </button>
        </div>
    {/if}
</div>

<style>
    .privacy-dashboard { padding: 0; }
    .section-header { font-size: 14px; font-weight: 600; margin-bottom: 16px; text-transform: uppercase; letter-spacing: 0.5px; }
    .subsection-header { font-size: 12px; color: var(--text-secondary); margin-bottom: 8px; text-transform: uppercase; letter-spacing: 0.5px; }

    .flow-table { width: 100%; border-collapse: collapse; font-size: 12px; margin-bottom: 12px; }
    .flow-table th { text-align: left; color: var(--text-secondary); font-weight: 500; padding: 6px 8px; border-bottom: 1px solid var(--border); }
    .flow-table td { padding: 6px 8px; border-bottom: 1px solid var(--border); }
    .status-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 6px; vertical-align: middle; }
    .status-label { text-transform: capitalize; font-size: 11px; }
    .server-cell, .data-cell { color: var(--text-secondary); }

    .gemini-warning {
        background: rgba(247, 118, 142, 0.1); border: 1px solid rgba(247, 118, 142, 0.3);
        border-radius: var(--radius); padding: 8px 12px; font-size: 11px; color: var(--red); margin-bottom: 16px;
    }

    .collapse-toggle {
        display: flex; align-items: center; gap: 8px; width: 100%; background: none; border: none;
        border-bottom: 1px solid var(--border); color: var(--text); font-family: var(--font);
        font-size: 12px; padding: 10px 0; cursor: pointer; text-align: left;
    }
    .collapse-toggle:hover { color: var(--blue); }
    .arrow { font-size: 10px; color: var(--text-secondary); width: 12px; }

    .audit-list { margin-bottom: 8px; }
    .empty-state, .safe-state { color: var(--text-secondary); font-size: 12px; padding: 12px 0; }
    .safe-state { color: var(--green); }
    .audit-entry { border-bottom: 1px solid var(--border); }
    .audit-row {
        display: flex; align-items: center; gap: 12px; width: 100%; background: none; border: none;
        color: var(--text); font-family: var(--font); font-size: 12px; padding: 8px 0; cursor: pointer; text-align: left;
    }
    .audit-row:hover { background: var(--surface); }
    .audit-time { color: var(--text-secondary); min-width: 100px; font-size: 11px; }
    .audit-provider { min-width: 80px; font-weight: 500; }
    .audit-size { color: var(--text-secondary); min-width: 70px; font-size: 11px; }
    .audit-status { font-size: 11px; color: var(--text-secondary); }
    .audit-status.success { color: var(--green); }

    .audit-details { padding: 8px 0 12px 24px; background: var(--surface); border-radius: var(--radius); margin-bottom: 4px; }
    .detail-row { display: flex; gap: 12px; padding: 3px 8px; font-size: 11px; }
    .detail-label { color: var(--text-secondary); min-width: 90px; flex-shrink: 0; }
    .detail-value { color: var(--text); word-break: break-all; }
    .endpoint { font-size: 10px; color: var(--text-secondary); }
    .prompt-preview { color: var(--orange); font-style: italic; }
    .certs { color: var(--green); }
    .training-warning { color: var(--red); }
    .tos-link { color: var(--blue); text-decoration: none; font-size: 10px; }
    .tos-link:hover { text-decoration: underline; }

    .potential-list { padding: 8px 0; }
    .potential-item { font-size: 12px; padding: 4px 0; color: var(--text-secondary); }
    .potential-item strong { color: var(--text); }
    .loading { color: var(--text-secondary); font-size: 12px; }
    .footer-sections { margin-top: 8px; }

    .agent-log-section { margin-top: 4px; }
    .auth-failed-badge {
        margin-left: auto;
        font-size: 10px;
        padding: 1px 6px;
        border-radius: 3px;
        color: var(--red);
        background: color-mix(in srgb, var(--red) 15%, transparent);
    }
    .agent-row {
        display: flex; align-items: center; gap: 12px;
        font-size: 12px; padding: 6px 0; border-bottom: 1px solid var(--border);
        color: var(--text);
    }
    .agent-row.failed { color: var(--red); }
    .agent-actor {
        display: flex;
        flex-direction: column;
        min-width: 120px;
    }
    .agent-actor strong {
        color: inherit;
        font-size: 12px;
        font-weight: 600;
    }
    .agent-actor em {
        color: var(--text-secondary);
        font-size: 10px;
        font-style: normal;
        text-transform: capitalize;
    }
    .agent-tool { min-width: 140px; font-weight: 500; }
    .agent-tool.auth-failed { color: var(--red); }
    .agent-status { color: var(--text-secondary); font-size: 11px; text-transform: capitalize; }
    .agent-status.ok { color: var(--green); }
    .agent-row.failed .agent-status { color: var(--red); }
</style>
