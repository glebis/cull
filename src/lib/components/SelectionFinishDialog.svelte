<script lang="ts">
    // Finish summary: name, source count, shortlist count vs optional target,
    // rejected-but-shortlisted conflicts, and the explicit statement that the
    // result becomes a normal collection while files and decisions stay put.
    import ModalDialog from '$lib/components/ModalDialog.svelte';
    import { selectionFinishOpen, selectionRun, shortlistIds } from '$lib/stores';
    import { finishSelection } from '$lib/selection-mode';

    let finishing = $state(false);

    const run = $derived($selectionRun);
    const shortlistCount = $derived(run?.shortlist_count ?? $shortlistIds.size);
    const targetDiff = $derived.by(() => {
        if (!run || run.target_count === null) return null;
        const diff = shortlistCount - run.target_count;
        if (diff === 0) return 'matches the target';
        return diff < 0
            ? `${-diff} fewer than the target of ${run.target_count}`
            : `${diff} more than the target of ${run.target_count}`;
    });
    const rejectedCount = $derived(run?.rejected_shortlist_count ?? 0);
    const emptyShortlist = $derived(shortlistCount === 0);

    function cancel() {
        if (finishing) return;
        selectionFinishOpen.set(false);
    }

    async function confirmFinish() {
        if (finishing || emptyShortlist) return;
        finishing = true;
        try {
            await finishSelection();
            selectionFinishOpen.set(false);
        } catch (e) {
            console.error('Failed to finish selection:', e);
        } finally {
            finishing = false;
        }
    }
</script>

{#if $selectionFinishOpen && run}
    <ModalDialog
        titleId="selection-finish-title"
        descriptionId="selection-finish-description"
        overlayClass="dialog-overlay"
        panelClass="dialog"
        onclose={cancel}
    >
        <div class="dialog-header">
            <h3 id="selection-finish-title">Finish Selection…</h3>
            <button class="close-btn" onclick={cancel} aria-label="Close finish dialog" disabled={finishing}>&times;</button>
        </div>

        <div class="dialog-body" id="selection-finish-description">
            <p class="summary-line"><strong>{run.name}</strong></p>
            <p class="summary-line">Source: {run.source_count} images</p>
            <p class="summary-line" data-finish-shortlist-count>
                Shortlist: {shortlistCount} image{shortlistCount === 1 ? '' : 's'}{targetDiff ? ` — ${targetDiff}` : ''}
            </p>
            {#if rejectedCount > 0}
                <p class="conflict" role="status">
                    {rejectedCount} shortlisted image{rejectedCount === 1 ? ' is' : 's are'} also marked rejected.
                    Finishing keeps them in the collection; the rejection itself is not changed.
                </p>
            {/if}
            {#if emptyShortlist}
                <p class="blocked" role="status">
                    The shortlist is empty. Add images before finishing, or archive the selection instead.
                </p>
            {/if}
            <p class="contract">
                Finishing creates a normal collection named “{run.name}”. No files are copied, moved, or deleted,
                and no accept/reject decisions change.
            </p>
        </div>

        <div class="dialog-footer">
            <button class="btn secondary" data-modal-initial-focus onclick={cancel} disabled={finishing}>
                Cancel
            </button>
            <button class="btn primary" onclick={() => void confirmFinish()} disabled={finishing || emptyShortlist}>
                {finishing ? 'Finishing…' : 'Finish Selection'}
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
        gap: 8px;
    }
    .summary-line {
        margin: 0;
        font-size: 13px;
        color: var(--text);
    }
    .conflict,
    .blocked {
        margin: 0;
        font-size: 12px;
        color: var(--orange);
    }
    .contract {
        margin: 0;
        font-size: 12px;
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
        padding: 6px 14px;
        font-size: 12px;
        cursor: pointer;
        background: transparent;
        color: var(--text);
    }
    .btn.primary {
        background: var(--blue);
        border-color: var(--blue);
        color: var(--bg);
        font-weight: 600;
    }
    .btn:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }
</style>