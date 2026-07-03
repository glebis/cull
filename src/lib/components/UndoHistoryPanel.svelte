<script lang="ts">
    import { onMount } from 'svelte';
    import { undo, redo, listUndoHistory, type UndoRecord } from '$lib/api';
    import { showToast, undoHistoryOpen } from '$lib/stores';

    let loading = $state(false);
    let loadingAction: 'undo' | 'redo' | null = $state(null);
    let error = $state<string | null>(null);
    let history = $state<UndoRecord[]>([]);
    let expanded = $state<string | null>(null);

    function closeHistory() {
        undoHistoryOpen.set(false);
    }

    function formatTime(ts: string): string {
        const d = new Date(ts);
        if (Number.isNaN(d.getTime())) return 'Unknown time';
        return `${d.toLocaleDateString('en-GB', { day: '2-digit', month: 'short', year: 'numeric' })} ${d.toLocaleTimeString('en-GB', {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
            hour12: false,
        })}`;
    }

    function affectedCount(raw: string | null): number {
        if (!raw) return 0;
        const ids = raw.split(',').map(v => v.trim()).filter(Boolean);
        return ids.length;
    }

    function actionLabel(actionType: string): string {
        if (!actionType) return 'Unknown action';
        if (actionType === 'set_rating') return 'Set rating';
        if (actionType === 'set_decision') return 'Set decision';
        if (actionType === 'trash_image') return 'Move to Trash';
        return actionType.replace(/_/g, ' ');
    }

    function displayLabel(entry: UndoRecord): string {
        return entry.label?.trim() || actionLabel(entry.action_type || 'unknown_action');
    }

    function formatJson(payload: string): string {
        try {
            return JSON.stringify(JSON.parse(payload), null, 2);
        } catch {
            return payload;
        }
    }

    function toggleExpanded(id: string) {
        expanded = expanded === id ? null : id;
    }

    async function loadHistory() {
        loading = true;
        error = null;
        try {
            history = await listUndoHistory(40);
        } catch (e) {
            error = String(e);
        } finally {
            loading = false;
        }
    }

    async function performUndo() {
        if (loadingAction) return;
        loadingAction = 'undo';
        try {
            const label = await undo();
            if (label) {
                showToast(`Undone: ${label}`, { type: 'info', duration: 3500 });
                loadHistory();
                window.dispatchEvent(new CustomEvent('reload-images'));
            } else {
                showToast('Nothing to undo', { type: 'warning', duration: 2500 });
            }
        } catch (e) {
            showToast('Undo failed', { detail: String(e), type: 'error', duration: 6000 });
        } finally {
            loadingAction = null;
        }
    }

    async function performRedo() {
        if (loadingAction) return;
        loadingAction = 'redo';
        try {
            const label = await redo();
            if (label) {
                showToast(`Redone: ${label}`, { type: 'info', duration: 3500 });
                loadHistory();
                window.dispatchEvent(new CustomEvent('reload-images'));
            } else {
                showToast('Nothing to redo', { type: 'warning', duration: 2500 });
            }
        } catch (e) {
            showToast('Redo failed', { detail: String(e), type: 'error', duration: 6000 });
        } finally {
            loadingAction = null;
        }
    }

    function handleBackdropKeydown(event: KeyboardEvent) {
        if (event.key === 'Escape') closeHistory();
    }

    $effect(() => {
        if ($undoHistoryOpen) {
            loadHistory();
        }
    });

    onMount(() => {
        const onReload = () => {
            if ($undoHistoryOpen) {
                loadHistory();
            }
        };
        window.addEventListener('reload-images', onReload);
        return () => {
            window.removeEventListener('reload-images', onReload);
        };
    });
</script>

{#if $undoHistoryOpen}
    <div
        class="history-backdrop"
        role="dialog"
        aria-modal="true"
        aria-label="Action history"
        tabindex="-1"
        onclick={closeHistory}
        onkeydown={handleBackdropKeydown}
    >
        <section class="history-panel" role="presentation" onclick={(e) => e.stopPropagation()}>
            <header class="history-head">
                <div class="history-title-wrap">
                    <h2>Action History</h2>
                    <p class="history-subtitle">Recent library actions and undo state</p>
                </div>
                <div class="history-head-actions">
                    <button class="history-head-btn" type="button" onclick={() => loadHistory()} disabled={loading}>
                        Refresh
                    </button>
                    <button class="history-close" type="button" onclick={closeHistory} aria-label="Close history">
                        ×
                    </button>
                </div>
            </header>

            <div class="history-toolbar">
                <button class="history-btn undo" type="button" onclick={performUndo} disabled={loadingAction !== null}>
                    Undo
                </button>
                <button class="history-btn redo" type="button" onclick={performRedo} disabled={loadingAction !== null}>
                    Redo
                </button>
            </div>

            {#if loading}
                <p class="history-state">Loading history…</p>
            {:else if error}
                <p class="history-state error">Failed to load history: {error}</p>
            {:else if history.length === 0}
                <div class="history-empty" role="status">
                    <svg
                        class="history-empty-image"
                        viewBox="0 0 160 112"
                        role="img"
                        aria-label="Empty action timeline"
                    >
                        <rect class="empty-frame" x="18" y="18" width="124" height="76" rx="4" />
                        <path class="empty-line" d="M46 36h68M46 56h48M46 76h58" />
                        <circle class="empty-node primary" cx="34" cy="36" r="5" />
                        <circle class="empty-node" cx="34" cy="56" r="5" />
                        <circle class="empty-node" cx="34" cy="76" r="5" />
                        <path class="empty-spark" d="M118 36l8-10 8 10-8 10z" />
                    </svg>
                    <div class="history-empty-copy">
                        <h3>No undoable actions yet</h3>
                        <p>Recorded actions will appear here.</p>
                    </div>
                </div>
            {:else}
                <div class="history-list" role="list">
                    {#each history as entry (entry.id)}
                        <article class="history-item" role="listitem">
                            <button class="history-row" type="button" onclick={() => toggleExpanded(entry.id)}>
                                <span class="history-type">{actionLabel(entry.action_type)}</span>
                                <span class="history-label" title={displayLabel(entry)}>{displayLabel(entry)}</span>
                                <span class="history-meta">{formatTime(entry.created_at)}</span>
                                <span class="history-count">{affectedCount(entry.affected_image_ids)} image{affectedCount(entry.affected_image_ids) === 1 ? '' : 's'}</span>
                                <span class="history-chevron">{expanded === entry.id ? '▼' : '▶'}</span>
                            </button>
                            {#if expanded === entry.id}
                                <div class="history-details">
                                    <div class="detail-grid">
                                        <div class="detail-item">
                                            <span class="detail-key">Seq</span>
                                            <span class="detail-value">#{entry.seq}</span>
                                        </div>
                                        <div class="detail-item">
                                            <span class="detail-key">Action ID</span>
                                            <span class="detail-value">{entry.id}</span>
                                        </div>
                                        <div class="detail-item">
                                            <span class="detail-key">File backup</span>
                                            <span class="detail-value">{entry.has_file_backup ? 'Yes' : 'No'}</span>
                                        </div>
                                        <div class="detail-item">
                                            <span class="detail-key">Affected image IDs</span>
                                            <span class="detail-value">{entry.affected_image_ids ?? 'None'}</span>
                                        </div>
                                    </div>
                                    <div class="history-json-block">
                                        <div class="history-json-col">
                                            <h4>Before</h4>
                                            <pre>{formatJson(entry.before_json)}</pre>
                                        </div>
                                        <div class="history-json-col">
                                            <h4>After</h4>
                                            <pre>{formatJson(entry.after_json)}</pre>
                                        </div>
                                    </div>
                                </div>
                            {/if}
                        </article>
                    {/each}
                </div>
            {/if}
        </section>
    </div>
{/if}

<style>
    .history-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.58);
        display: flex;
        align-items: flex-start;
        justify-content: center;
        padding: 10vh 16px 24px;
        z-index: var(--z-modal);
        color: var(--text);
    }

    .history-panel {
        width: min(980px, 100%);
        max-height: min(82vh, 100%);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 12px;
        display: flex;
        flex-direction: column;
        gap: 10px;
        box-sizing: border-box;
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.55);
        overflow: hidden;
    }

    .history-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
        min-width: 0;
    }

    .history-title-wrap h2 {
        margin: 0;
        font-size: 15px;
        color: var(--text);
    }

    .history-subtitle {
        margin: 2px 0 0;
        font-size: 12px;
        color: var(--text-secondary);
    }

    .history-head-actions {
        display: flex;
        align-items: center;
        gap: 8px;
        flex-shrink: 0;
    }

    .history-head-btn,
    .history-close,
    .history-btn {
        border: 1px solid var(--border);
        background: var(--surface);
        color: var(--text);
        font: inherit;
        font-size: 11px;
        cursor: pointer;
        height: 24px;
        padding: 0 8px;
        border-radius: var(--radius);
    }

    .history-close {
        width: 24px;
        padding: 0;
        color: var(--text-secondary);
    }

    .history-close:hover,
    .history-head-btn:hover,
    .history-btn:hover {
        border-color: var(--blue);
    }

    .history-toolbar {
        display: flex;
        gap: 8px;
    }

    .history-btn.undo {
        color: var(--green);
        border-color: rgba(158, 206, 106, 0.45);
    }

    .history-btn.redo {
        color: var(--blue);
        border-color: rgba(122, 162, 247, 0.45);
    }

    .history-toolbar .history-btn:disabled {
        color: var(--text-secondary);
        cursor: not-allowed;
        border-color: var(--border);
    }

    .history-state {
        margin: 0;
        color: var(--text-secondary);
        font-size: 12px;
    }

    .history-state.error {
        color: var(--red);
    }

    .history-empty {
        min-height: 280px;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 14px;
        text-align: center;
        color: var(--text-secondary);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
    }

    .history-empty-image {
        width: min(180px, 48vw);
        height: auto;
        color: var(--blue);
    }

    .empty-frame {
        fill: var(--surface);
        stroke: var(--border);
        stroke-width: 2;
    }

    .empty-line {
        fill: none;
        stroke: var(--text-secondary);
        stroke-width: 3;
        stroke-linecap: round;
        opacity: 0.8;
    }

    .empty-node {
        fill: var(--surface);
        stroke: var(--purple);
        stroke-width: 2;
    }

    .empty-node.primary {
        stroke: var(--green);
    }

    .empty-spark {
        fill: var(--surface);
        stroke: currentColor;
        stroke-width: 2;
        stroke-linejoin: round;
    }

    .history-empty-copy {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }

    .history-empty-copy h3 {
        margin: 0;
        color: var(--text);
        font-size: 13px;
        font-weight: 500;
    }

    .history-empty-copy p {
        margin: 0;
        color: var(--text-secondary);
        font-size: 11px;
    }

    .history-list {
        overflow: auto;
        display: flex;
        flex-direction: column;
        gap: 8px;
        min-height: 0;
    }

    .history-item {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        overflow: hidden;
    }

    .history-row {
        width: 100%;
        border: none;
        background: transparent;
        color: inherit;
        font: inherit;
        display: grid;
        grid-template-columns: 110px minmax(240px, 1fr) 190px 120px 24px;
        gap: 10px;
        align-items: center;
        padding: 8px 10px;
        text-align: left;
        cursor: pointer;
    }

    .history-type {
        color: var(--blue);
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.2px;
    }

    .history-label {
        color: var(--text);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .history-meta,
    .history-count {
        color: var(--text-secondary);
        font-size: 11px;
        text-align: right;
    }

    .history-count {
        text-align: left;
    }

    .history-chevron {
        color: var(--text-secondary);
    }

    .history-details {
        border-top: 1px solid var(--border);
        padding: 10px;
        display: flex;
        flex-direction: column;
        gap: 10px;
    }

    .detail-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 8px;
    }

    .detail-item {
        display: flex;
        flex-direction: column;
        gap: 4px;
        min-width: 0;
    }

    .detail-key {
        color: var(--text-secondary);
        font-size: 10px;
    }

    .detail-value {
        font-size: 11px;
        color: var(--text);
        word-break: break-all;
    }

    .history-json-block {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 8px;
    }

    .history-json-col h4 {
        margin: 0 0 6px;
        color: var(--text-secondary);
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.3px;
    }

    .history-json-col pre {
        margin: 0;
        max-height: 130px;
        overflow: auto;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 6px;
        font-size: 11px;
        color: var(--text);
        white-space: pre-wrap;
        word-break: break-word;
    }

    @media (max-width: 880px) {
        .history-panel {
            width: 100%;
            max-height: 100%;
            margin: 0;
            border-radius: 0;
            padding: 10px;
        }

        .history-row {
            grid-template-columns: 80px minmax(120px, 1fr) auto 20px;
        }

        .history-count,
        .history-meta {
            display: none;
        }

        .detail-grid,
        .history-json-block {
            grid-template-columns: 1fr;
        }
    }
</style>
