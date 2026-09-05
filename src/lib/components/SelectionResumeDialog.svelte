<script lang="ts">
    // Resume dialog: lists selection runs by lifecycle. Active runs resume,
    // finished runs continue under the same identity, archived runs are
    // restored first. Nothing is duplicated or deleted.
    import ModalDialog from '$lib/components/ModalDialog.svelte';
    import { selectionResumeOpen, showToast } from '$lib/stores';
    import { listSelectionRuns, type SelectionRun } from '$lib/api';
    import { resumeSelectionRun } from '$lib/selection-mode';

    let runs = $state<SelectionRun[]>([]);
    let loading = $state(false);
    let loadError = $state<string | null>(null);
    let busyId = $state<string | null>(null);

    $effect(() => {
        if (!$selectionResumeOpen) return;
        void refresh();
    });

    async function refresh() {
        loading = true;
        loadError = null;
        try {
            const [active, finished, archived] = await Promise.all([
                listSelectionRuns('active'),
                listSelectionRuns('finished'),
                listSelectionRuns('archived'),
            ]);
            const byRecency = (a: SelectionRun, b: SelectionRun) => b.updated_at.localeCompare(a.updated_at);
            runs = [
                ...active.sort(byRecency),
                ...finished.sort(byRecency),
                ...archived.sort(byRecency),
            ];
        } catch (e) {
            loadError = e instanceof Error ? e.message : String(e);
        } finally {
            loading = false;
        }
    }

    const activeRuns = $derived(runs.filter(r => r.status === 'active'));
    const finishedRuns = $derived(runs.filter(r => r.status === 'finished'));
    const archivedRuns = $derived(runs.filter(r => r.status === 'archived'));

    function actionLabel(run: SelectionRun): string {
        if (run.status === 'active') return 'Resume';
        if (run.status === 'finished') return 'Continue as Selection';
        return 'Restore and Resume';
    }

    function countLine(run: SelectionRun): string {
        const target = run.target_count !== null ? ` / target ${run.target_count}` : '';
        return `Source ${run.source_count} · Shortlist ${run.shortlist_count}${target}`;
    }

    function cancel() {
        if (busyId) return;
        selectionResumeOpen.set(false);
    }

    async function open(run: SelectionRun) {
        if (busyId) return;
        busyId = run.id;
        try {
            await resumeSelectionRun(run.id);
            selectionResumeOpen.set(false);
        } catch (e) {
            showToast('Could not resume the selection', {
                detail: e instanceof Error ? e.message : String(e),
                type: 'error',
                duration: 9000,
            });
        } finally {
            busyId = null;
        }
    }
</script>

{#if $selectionResumeOpen}
    <ModalDialog
        titleId="selection-resume-title"
        descriptionId="selection-resume-description"
        overlayClass="dialog-overlay"
        panelClass="dialog"
        onclose={cancel}
    >
        <div class="dialog-header">
            <h3 id="selection-resume-title">Resume Selection…</h3>
            <button class="close-btn" onclick={cancel} aria-label="Close resume dialog" disabled={busyId !== null}>&times;</button>
        </div>

        <div class="dialog-body" id="selection-resume-description">
            {#if loading}
                <p class="note" role="status">Loading selections…</p>
            {:else if loadError}
                <p class="error" role="status">Could not load selections: {loadError}</p>
            {:else if runs.length === 0}
                <p class="note" role="status">No selection runs yet. Start one with “Start Selection…”.</p>
            {:else}
                {#if activeRuns.length > 0}
                    <div class="group" role="group" aria-label="Active selections">
                        <div class="group-label">Active</div>
                        {#each activeRuns as run (run.id)}
                            <div class="row" data-run-id={run.id} data-run-status={run.status}>
                                <div class="row-text">
                                    <span class="row-name">{run.name}</span>
                                    <span class="row-counts">{countLine(run)}</span>
                                </div>
                                <button class="btn" onclick={() => void open(run)} disabled={busyId !== null}>
                                    {busyId === run.id ? 'Opening…' : actionLabel(run)}
                                </button>
                            </div>
                        {/each}
                    </div>
                {/if}
                {#if finishedRuns.length > 0}
                    <div class="group" role="group" aria-label="Finished selections">
                        <div class="group-label">Finished</div>
                        {#each finishedRuns as run (run.id)}
                            <div class="row" data-run-id={run.id} data-run-status={run.status}>
                                <div class="row-text">
                                    <span class="row-name">{run.name}</span>
                                    <span class="row-counts">{countLine(run)}</span>
                                </div>
                                <button class="btn" onclick={() => void open(run)} disabled={busyId !== null}>
                                    {busyId === run.id ? 'Opening…' : actionLabel(run)}
                                </button>
                            </div>
                        {/each}
                    </div>
                {/if}
                {#if archivedRuns.length > 0}
                    <div class="group" role="group" aria-label="Archived selections">
                        <div class="group-label">Archived</div>
                        {#each archivedRuns as run (run.id)}
                            <div class="row" data-run-id={run.id} data-run-status={run.status}>
                                <div class="row-text">
                                    <span class="row-name">{run.name}</span>
                                    <span class="row-counts">{countLine(run)}</span>
                                </div>
                                <button class="btn" onclick={() => void open(run)} disabled={busyId !== null}>
                                    {busyId === run.id ? 'Opening…' : actionLabel(run)}
                                </button>
                            </div>
                        {/each}
                    </div>
                {/if}
            {/if}
        </div>

        <div class="dialog-footer">
            <button class="btn secondary" data-modal-initial-focus onclick={cancel} disabled={busyId !== null}>
                Close
            </button>
        </div>
    </ModalDialog>
{/if}

<style>
    .dialog-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: calc(var(--spacing) * 2);
        border-bottom: 1px solid var(--border);
    }
    .dialog-header h3 {
        margin: 0;
        font-size: 14px;
        color: var(--text);
    }
    .close-btn {
        background: transparent;
        border: none;
        color: var(--text-secondary);
        font-size: 16px;
        cursor: pointer;
    }
    .dialog-body {
        padding: calc(var(--spacing) * 2);
        display: flex;
        flex-direction: column;
        gap: 12px;
        max-height: 60vh;
        overflow-y: auto;
    }
    .note {
        margin: 0;
        font-size: 12px;
        color: var(--text-secondary);
    }
    .error {
        margin: 0;
        font-size: 12px;
        color: var(--red);
    }
    .group {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    .group-label {
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        color: var(--text-caption);
    }
    .row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
        padding: 6px 8px;
        border: 1px solid var(--border-subtle);
        border-radius: 4px;
    }
    .row-text {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
    }
    .row-name {
        font-size: 13px;
        color: var(--text);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .row-counts {
        font-size: 11px;
        color: var(--text-secondary);
    }
    .dialog-footer {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
        padding: calc(var(--spacing) * 2);
        border-top: 1px solid var(--border);
    }
    .btn {
        border-radius: 4px;
        border: 1px solid var(--border);
        padding: 6px 12px;
        font-size: 12px;
        cursor: pointer;
        background: transparent;
        color: var(--text);
        white-space: nowrap;
    }
    .btn.secondary {
        color: var(--text-secondary);
    }
    .btn:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }
</style>