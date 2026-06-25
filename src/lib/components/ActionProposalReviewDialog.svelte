<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import ModalDialog from '$lib/components/ModalDialog.svelte';
    import type { AgentActionProposal, ImageWithFile } from '$lib/api';
    import { safeAssetPreviewPath } from '$lib/view-utils';

    type Candidate = { image_id: string; reason?: string };

    let {
        proposal,
        visible,
        visibleImages = [],
        onapplyproposal = () => {},
        oncancelreview = () => {},
    }: {
        proposal: AgentActionProposal | null;
        visible: boolean;
        visibleImages?: ImageWithFile[];
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

    function imageForCandidate(imageId: string) {
        return visibleImages.find(item => item.image.id === imageId) ?? null;
    }

    function previewSrc(imageId: string) {
        const image = imageForCandidate(imageId);
        if (!image) return null;
        const previewPath = safeAssetPreviewPath(image, { displayPx: 120 });
        return previewPath ? convertFileSrc(previewPath) : null;
    }

    function filenameForCandidate(imageId: string) {
        const image = imageForCandidate(imageId);
        return image?.path.split('/').pop() ?? imageId;
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
                    <span class="candidate-preview" aria-hidden="true">
                        {#if previewSrc(candidate.image_id)}
                            <img src={previewSrc(candidate.image_id) ?? ''} alt="" loading="lazy" decoding="async" draggable="false" />
                        {:else}
                            <small>{candidate.image_id.slice(0, 8)}</small>
                        {/if}
                    </span>
                    <span>
                        <strong>{filenameForCandidate(candidate.image_id)}</strong>
                        <em>{candidate.image_id.slice(0, 8)}</em>
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
        grid-template-columns: auto 72px 1fr;
        padding: var(--spacing);
    }

    .candidate-preview {
        align-items: center;
        aspect-ratio: 4 / 3;
        background: var(--border);
        border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
        border-radius: var(--radius);
        display: flex;
        justify-content: center;
        overflow: hidden;
    }

    .candidate-preview img {
        height: 100%;
        object-fit: cover;
        width: 100%;
    }

    .candidate em {
        color: var(--text-secondary);
        display: block;
        font-style: normal;
        margin-top: 2px;
    }

    .candidate small {
        display: block;
        margin-top: 2px;
    }
</style>
