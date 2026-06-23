<script lang="ts">
    import ModalDialog from '$lib/components/ModalDialog.svelte';
    import type { AgentActionProposal } from '$lib/api';

    type Candidate = { image_id: string; reason?: string };

    let {
        proposal,
        visible,
        onapplyproposal = () => {},
        oncancelreview = () => {},
    }: {
        proposal: AgentActionProposal | null;
        visible: boolean;
        onapplyproposal?: (proposalId: string, approvedImageIds: string[]) => void;
        oncancelreview?: () => void;
    } = $props();

    let approvedIds = $state<Set<string>>(new Set());
    const candidates = $derived(parseCandidates(proposal?.items_json));
    const actionLabel = $derived(proposal?.kind === 'trash_images' ? 'Move approved to Trash' : 'Select approved');

    $effect(() => {
        approvedIds = new Set(candidates.map(candidate => candidate.image_id));
    });

    function parseCandidates(itemsJson: string | undefined): Candidate[] {
        if (!itemsJson) return [];
        try {
            const parsed = JSON.parse(itemsJson);
            return Array.isArray(parsed) ? parsed : [];
        } catch {
            return [];
        }
    }

    function toggle(imageId: string) {
        const next = new Set(approvedIds);
        if (next.has(imageId)) next.delete(imageId);
        else next.add(imageId);
        approvedIds = next;
    }
</script>

{#if visible && proposal}
    <ModalDialog
        titleId="agent-proposal-review-title"
        descriptionId="agent-proposal-review-description"
        overlayClass="agent-review-overlay"
        panelClass="agent-review-dialog"
        onclose={oncancelreview}
    >
        <header class="dialog-header">
            <div>
                <h2 id="agent-proposal-review-title">
                    {proposal.kind === 'trash_images' ? 'Review Trash proposal' : 'Review selection proposal'}
                </h2>
                <p id="agent-proposal-review-description">{proposal.criteria}</p>
            </div>
            <button type="button" onclick={oncancelreview}>Cancel</button>
        </header>

        <div class="summary">
            <span>{approvedIds.size} of {candidates.length} approved</span>
            <span>Context: {proposal.visual_level}</span>
            <span>EUR {proposal.estimated_cost_eur?.toFixed(3) ?? '0.000'} est</span>
        </div>

        <div class="candidate-list">
            {#each candidates as candidate}
                <label class="candidate">
                    <input
                        type="checkbox"
                        checked={approvedIds.has(candidate.image_id)}
                        aria-label={`Include ${candidate.image_id}`}
                        onchange={() => toggle(candidate.image_id)}
                    />
                    <span>
                        <strong>{candidate.image_id}</strong>
                        <small>{candidate.reason ?? 'Candidate selected by proposal criteria'}</small>
                    </span>
                </label>
            {/each}
        </div>

        <footer class="dialog-footer">
            <button type="button" onclick={oncancelreview}>Keep reviewing</button>
            <button
                class:danger={proposal.kind === 'trash_images'}
                class="primary"
                type="button"
                onclick={() => onapplyproposal(proposal.id, Array.from(approvedIds))}
                disabled={approvedIds.size === 0}
            >
                {actionLabel}
            </button>
        </footer>
    </ModalDialog>
{/if}

<style>
    :global(.agent-review-overlay) {
        align-items: center;
        background: color-mix(in srgb, var(--bg) 70%, transparent);
        display: flex;
        inset: 0;
        justify-content: center;
        position: fixed;
        z-index: 1000;
    }

    :global(.agent-review-dialog) {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
        max-height: 80vh;
        max-width: 720px;
        padding: calc(var(--spacing) * 2);
        width: min(720px, calc(100vw - 32px));
    }

    .dialog-header,
    .dialog-footer,
    .summary {
        align-items: center;
        display: flex;
        gap: var(--spacing);
        justify-content: space-between;
    }

    h2 {
        font-size: 16px;
        margin: 0;
    }

    p,
    small,
    .summary {
        color: var(--text-secondary);
    }

    button {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font: inherit;
        padding: 6px 8px;
    }

    button.primary {
        border-color: var(--blue);
        color: var(--blue);
    }

    button.danger {
        border-color: var(--red);
        color: var(--red);
    }

    button:disabled {
        opacity: 0.45;
    }

    .candidate-list {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
        overflow: auto;
    }

    .candidate {
        align-items: center;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        display: grid;
        gap: var(--spacing);
        grid-template-columns: auto 1fr;
        padding: var(--spacing);
    }

    .candidate small {
        display: block;
        margin-top: 2px;
    }
</style>
