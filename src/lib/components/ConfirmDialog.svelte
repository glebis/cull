<script lang="ts">
    import ModalDialog from '$lib/components/ModalDialog.svelte';
    import { confirmDialog, resolveConfirmDialog } from '$lib/stores';

    function cancel() {
        resolveConfirmDialog(false);
    }

    function confirm() {
        resolveConfirmDialog(true);
    }
</script>

{#if $confirmDialog}
<ModalDialog
    titleId="confirm-dialog-title"
    descriptionId={$confirmDialog.description ? 'confirm-dialog-description' : undefined}
    overlayClass="dialog-overlay"
    panelClass="dialog"
    onclose={cancel}
>
    <div class="dialog-header">
        <h3 id="confirm-dialog-title">{$confirmDialog.title}</h3>
        <button class="close-btn" onclick={cancel} aria-label="Close confirmation">&times;</button>
    </div>

    {#if $confirmDialog.description}
        <div class="dialog-body">
            <p id="confirm-dialog-description" class="confirm-text">{$confirmDialog.description}</p>
        </div>
    {/if}

    <div class="dialog-footer">
        <button class="btn secondary" data-modal-initial-focus onclick={cancel}>
            {$confirmDialog.cancelLabel ?? 'Cancel'}
        </button>
        <button class="btn primary" class:danger={$confirmDialog.danger} onclick={confirm}>
            {$confirmDialog.confirmLabel ?? 'Confirm'}
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
        background: none;
        border: none;
        color: var(--text-secondary);
        font-size: 18px;
        cursor: pointer;
        padding: 0 4px;
    }
    .close-btn:hover { color: var(--text); }
    .dialog-body {
        padding: calc(var(--spacing) * 2);
    }
    .confirm-text {
        margin: 0;
        font-size: 13px;
        color: var(--text);
        line-height: 1.45;
        overflow-wrap: anywhere;
        word-break: break-word;
    }
    .dialog-footer {
        display: flex;
        justify-content: flex-end;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
        border-top: 1px solid var(--border);
    }
    .btn {
        padding: 6px 16px;
        border-radius: var(--radius);
        font-size: 12px;
        font-family: var(--font);
        cursor: pointer;
        border: 1px solid var(--border);
    }
    .btn:focus-visible, .close-btn:focus-visible {
        outline: 2px solid var(--blue);
        outline-offset: 2px;
    }
    .btn.secondary {
        background: var(--surface);
        color: var(--text-secondary);
    }
    .btn.secondary:hover {
        color: var(--text);
        border-color: var(--text-secondary);
    }
    .btn.primary {
        background: color-mix(in srgb, var(--green) 16%, var(--surface));
        border-color: var(--green);
        color: var(--green);
    }
    .btn.primary:hover {
        background: color-mix(in srgb, var(--green) 24%, var(--surface));
    }
    .btn.primary.danger {
        background: var(--red);
        border-color: var(--red);
        color: var(--bg);
    }
    .btn.primary.danger:hover {
        filter: brightness(1.1);
        background: var(--red);
    }
</style>
