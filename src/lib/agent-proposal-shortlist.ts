// Apply flow for shortlist_add / shortlist_remove proposals.
//
// The backend apply endpoint validates and mutates shortlist membership
// atomically for the run captured in proposal.source_context_json.selection_id.
// The UI's only job is to review the exact approved IDs and hand them over:
// no visibility filtering, no transient highlight writes, and no direct
// shortlist commands (a two-step apply is rejected server-side).

import { applyActionProposal, getSelectionRun, type AgentActionProposal } from './api';
import {
    handleSelectionRunUpdated,
} from './selection-mode';
import {
    isShortlistProposalKind,
    parseAgentProposalSourceContext,
    shortlistProposalActionLabel,
    sourceContextSelectionId,
} from './agent-proposal-context';
import { showToast } from './stores';

export { isShortlistProposalKind, shortlistProposalActionLabel, sourceContextSelectionId };

export interface ShortlistApplyOptions {
    /** Invoked once after a committed apply (and its state refresh) so the
     *  caller can close the review and refresh the pending-proposal panel.
     *  Never invoked when the apply itself failed. */
    onApplied?: () => void | Promise<void>;
}

function failureTitle(kind: string): string {
    return kind === 'shortlist_remove'
        ? 'Could not remove from the shortlist'
        : 'Could not add to the shortlist';
}

function successTitle(kind: string): string {
    return shortlistProposalActionLabel(kind);
}

/** Human label for the captured target: its name when the context carries
 *  one, otherwise the full run id (never a truncated internal form). */
function shortlistTargetLabel(context: ReturnType<typeof parseAgentProposalSourceContext>): string {
    const name = typeof context.selection_name === 'string' && context.selection_name.trim()
        ? context.selection_name.trim()
        : null;
    const id = sourceContextSelectionId(context);
    return name ?? (id ? `selection ${id}` : 'unknown selection');
}

/**
 * Apply a shortlist proposal for the exact approved IDs.
 *
 * The target run is the one captured in the proposal's source context — even
 * if the user has since opened a different run. After a committed apply the
 * captured run's state is refreshed and fed through the ordinary
 * selection-run update path; a refresh failure never re-applies the mutation.
 * A failed apply keeps the proposal pending and offers a Retry that reuses
 * the same approved IDs.
 */
export async function applyShortlistProposal(
    proposal: AgentActionProposal,
    approvedImageIds: string[],
    options: ShortlistApplyOptions = {},
): Promise<void> {
    const sourceContext = parseAgentProposalSourceContext(proposal.source_context_json);
    const targetRunId = sourceContextSelectionId(sourceContext);
    if (!targetRunId) {
        showToast('This proposal has no captured selection target', {
            detail: 'It cannot change a shortlist. Dismiss it and ask again while the selection is active.',
            type: 'warning',
            duration: 10000,
        });
        return;
    }
    if (approvedImageIds.length === 0) {
        showToast('No images approved', { type: 'warning', duration: 5000 });
        return;
    }

    try {
        // Single mutation call: the backend validates membership against the
        // captured source, applies one grouped undoable change, and updates
        // the proposal status atomically.
        await applyActionProposal(proposal.id, approvedImageIds, JSON.stringify({
            approved_count: approvedImageIds.length,
            selection_id: targetRunId,
        }));
    } catch (error) {
        // The proposal stays pending. Retry reuses the exact approved IDs.
        showToast(failureTitle(proposal.kind), {
            detail: `${String(error)} The proposal is kept — retry uses the same approved images.`,
            type: 'error',
            duration: 12000,
            actions: [{
                label: 'Retry',
                onclick: () => { void applyShortlistProposal(proposal, approvedImageIds, options); },
            }],
        });
        return;
    }

    // Refresh the captured target run separately from the commit. A refresh
    // failure must not re-apply the mutation.
    try {
        const state = await getSelectionRun(targetRunId);
        handleSelectionRunUpdated({ run: state.run, shortlist_ids: state.shortlist_ids });
    } catch (error) {
        console.error('Failed to refresh selection run after proposal apply:', error);
        showToast('Shortlist updated, but the counts could not be refreshed', {
            detail: 'The change is saved. Reopen the selection to see fresh counts.',
            type: 'warning',
            duration: 10000,
        });
    }

    showToast(successTitle(proposal.kind), {
        detail: `${approvedImageIds.length} image${approvedImageIds.length === 1 ? '' : 's'} · ${shortlistTargetLabel(sourceContext)}`,
        type: 'success',
        duration: 6000,
    });
    await options.onApplied?.();
}