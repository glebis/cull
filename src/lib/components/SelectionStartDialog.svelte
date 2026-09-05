<script lang="ts">
    // Start Selection sheet: editable name defaulted from the current source,
    // backend-resolved source count (never inferred from the visible page),
    // optional positive target, and the empty-shortlist / no-changes contract.
    import { tick } from 'svelte';
    import ModalDialog from '$lib/components/ModalDialog.svelte';
    import { selectionStartOpen, showToast } from '$lib/stores';
    import { previewSelectionSource } from '$lib/api';
    import {
        selectionStartAvailability,
        startSelectionRun,
    } from '$lib/selection-mode';

    let name = $state('');
    // Svelte's type=number binding supplies a number (or undefined when the
    // field is empty or mid-edit), never a string — keep the state typed and
    // normalize defensively so entering a target can never crash.
    let targetInput = $state<number | undefined>(undefined);
    let previewCount = $state<number | null>(null);
    let previewError = $state<string | null>(null);
    let resolving = $state(false);
    let starting = $state(false);
    let nameInputEl: HTMLInputElement | undefined = $state();

    let availability = $state(selectionStartAvailability());
    const scope = $derived(availability?.scope ?? null);
    const defaultName = $derived(availability?.label ?? 'Selection');

    $effect(() => {
        if (!$selectionStartOpen) return;
        availability = selectionStartAvailability();
        name = defaultName;
        targetInput = undefined;
        previewCount = null;
        previewError = null;
        resolving = true;
        tick().then(() => nameInputEl?.select());
    });

    $effect(() => {
        if (!$selectionStartOpen) return;
        const currentScope = scope;
        if (!currentScope) {
            previewCount = null;
            return;
        }
        resolving = true;
        previewError = null;
        previewSelectionSource(currentScope)
            .then(result => {
                previewCount = result.count;
            })
            .catch(e => {
                previewError = e instanceof Error ? e.message : String(e);
            })
            .finally(() => {
                resolving = false;
            });
    });

    /** Accepts the raw number-input binding (number, undefined when empty or
     *  mid-edit, or a string from programmatic writes). Empty stays optional
     *  null; only positive whole numbers are accepted. */
    function normalizeTargetInput(value: number | string | null | undefined): { value: number | null; error: string | null } {
        if (value === null || value === undefined || value === '') {
            return { value: null, error: null };
        }
        const numeric = typeof value === 'number' ? value : Number(value.trim());
        if (!Number.isFinite(numeric) || !Number.isInteger(numeric) || numeric < 1) {
            return { value: null, error: 'Target must be a positive whole number' };
        }
        return { value: numeric, error: null };
    }

    let parsedTarget = $derived(normalizeTargetInput(targetInput));

    let blockedReason = $derived.by(() => {
        if (!availability?.available) return availability?.reason ?? 'Selection cannot start here.';
        if (!scope) return 'No resolvable source in this view.';
        if (resolving) return null;
        if (previewError) return `The source could not be resolved: ${previewError}`;
        if (previewCount === 0) return 'This source has no images, so there is nothing to shortlist from.';
        return null;
    });

    let canStart = $derived(
        availability?.available
            && scope !== null
            && name.trim().length > 0
            && !resolving
            && previewError === null
            && (previewCount ?? 0) > 0
            && parsedTarget.error === null
            && !starting
    );

    function cancel() {
        if (starting) return;
        selectionStartOpen.set(false);
    }

    async function submit() {
        if (!canStart || !scope) return;
        starting = true;
        try {
            await startSelectionRun(name, parsedTarget.value);
            selectionStartOpen.set(false);
        } catch (e) {
            showToast('Could not start the selection', {
                detail: e instanceof Error ? e.message : String(e),
                type: 'error',
                duration: 9000,
            });
        } finally {
            starting = false;
        }
    }
</script>

{#if $selectionStartOpen}
    <ModalDialog
        titleId="selection-start-title"
        descriptionId="selection-start-description"
        overlayClass="dialog-overlay"
        panelClass="dialog"
        onclose={cancel}
    >
        <div class="dialog-header">
            <h3 id="selection-start-title">Start Selection</h3>
            <button class="close-btn" onclick={cancel} aria-label="Close start dialog" disabled={starting}>&times;</button>
        </div>

        <div class="dialog-body" id="selection-start-description">
            <label class="field">
                <span class="field-label">Name</span>
                <input
                    bind:this={nameInputEl}
                    bind:value={name}
                    type="text"
                    placeholder="Selection name"
                />
            </label>

            <div class="field">
                <span class="field-label">Source</span>
                <span class="source-line">
                    {availability?.label ?? 'No resolvable source'}
                    {#if resolving}
                        — counting…
                    {:else if previewError}
                        — could not count the source
                    {:else if previewCount !== null}
                        — {previewCount} image{previewCount === 1 ? '' : 's'}
                    {/if}
                </span>
            </div>

            <label class="field">
                <span class="field-label">Target count (optional)</span>
                <input
                    bind:value={targetInput}
                    type="number"
                    min="1"
                    step="1"
                    placeholder="e.g. 5"
                />
                {#if parsedTarget.error}
                    <span class="field-error">{parsedTarget.error}</span>
                {/if}
            </label>

            <ul class="contract-list">
                <li>Starts with an empty shortlist — highlighted images are not carried over.</li>
                <li>Originals and review decisions are not changed.</li>
            </ul>

            {#if blockedReason}
                <p class="blocked-note" role="status">{blockedReason}</p>
            {/if}
        </div>

        <div class="dialog-footer">
            <button class="btn secondary" data-modal-initial-focus onclick={cancel} disabled={starting}>
                Cancel
            </button>
            <button class="btn primary" onclick={submit} disabled={!canStart}>
                {starting ? 'Starting…' : 'Start Selection'}
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
        gap: 14px;
    }
    .field {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .field-label {
        font-size: 11px;
        color: var(--text-caption);
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }
    input {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: 4px;
        color: var(--text);
        padding: 6px 8px;
        font-size: 13px;
    }
    input:focus {
        outline: none;
        border-color: var(--blue);
    }
    .source-line {
        font-size: 12px;
        color: var(--text);
    }
    .field-error {
        font-size: 11px;
        color: var(--red);
    }
    .contract-list {
        margin: 0;
        padding-left: 18px;
        font-size: 12px;
        color: var(--text-secondary);
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .blocked-note {
        margin: 0;
        font-size: 12px;
        color: var(--orange);
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