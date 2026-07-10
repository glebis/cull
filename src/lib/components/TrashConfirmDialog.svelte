<script lang="ts">
    import ModalDialog from '$lib/components/ModalDialog.svelte';

    interface Props {
        visible: boolean;
        fileName: string;
        onconfirm: (suppress: 'none' | 'session' | 'always') => void;
        oncancel: () => void;
    }

    let { visible, fileName, onconfirm, oncancel }: Props = $props();

    let dontAsk = $state(false);
    let scope = $state<'session' | 'always'>('session');
    $effect(() => {
        if (visible) {
            dontAsk = false;
            scope = 'session';
        }
    });

    function confirm() {
        onconfirm(dontAsk ? scope : 'none');
    }
</script>

{#if visible}
<ModalDialog
    titleId="trash-confirm-title"
    descriptionId="trash-confirm-description"
    overlayClass="dialog-overlay"
    panelClass="dialog"
    onclose={oncancel}
>
    <div class="dialog-header">
        <h3 id="trash-confirm-title">Move to Trash</h3>
        <button class="close-btn" onclick={oncancel} aria-label="Close trash confirmation">&times;</button>
    </div>

    <div class="dialog-body">
        <p id="trash-confirm-description" class="confirm-text">Move <strong class="file-name" title={fileName}>{fileName}</strong> to Trash?</p>

        <label class="checkbox-row">
            <input type="checkbox" bind:checked={dontAsk} />
            <span>Don't ask again</span>
        </label>

        {#if dontAsk}
            <div class="scope-options">
                <label class="radio-row">
                    <input type="radio" bind:group={scope} value="session" />
                    <span>This session (until app restart)</span>
                </label>
                <label class="radio-row">
                    <input type="radio" bind:group={scope} value="always" />
                    <span>Always</span>
                </label>
            </div>
        {/if}
    </div>

    <div class="dialog-footer">
        <button class="btn secondary" onclick={oncancel}>Cancel</button>
        <button class="btn primary" data-modal-initial-focus onclick={confirm}>Move to Trash</button>
    </div>
</ModalDialog>
{/if}

<style>
    :global(.dialog-overlay) {
        position: fixed;
        inset: 0;
        background: color-mix(in srgb, var(--bg) 80%, transparent);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: var(--z-modal);
    }
    :global(.dialog) {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        width: 400px;
        max-width: 90vw;
    }
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
        display: flex;
        flex-direction: column;
        gap: calc(var(--spacing) * 1.5);
    }
    .confirm-text {
        margin: 0;
        font-size: 13px;
        color: var(--text);
        line-height: 1.45;
        min-width: 0;
    }
    .file-name {
        color: var(--blue);
        overflow-wrap: anywhere;
        word-break: break-word;
    }
    .checkbox-row, .radio-row {
        display: flex;
        align-items: center;
        gap: var(--spacing);
        font-size: 12px;
        color: var(--text-secondary);
        cursor: pointer;
    }
    .scope-options {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
        padding-left: calc(var(--spacing) * 2.5);
    }
    input[type="checkbox"], input[type="radio"] {
        accent-color: var(--blue);
    }
    input[type="checkbox"]:focus-visible,
    input[type="radio"]:focus-visible,
    .close-btn:focus-visible,
    .btn:focus-visible {
        outline: 2px solid var(--blue);
        outline-offset: 2px;
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
    .btn.secondary {
        background: var(--surface);
        color: var(--text-secondary);
    }
    .btn.secondary:hover {
        color: var(--text);
        border-color: var(--text-secondary);
    }
    .btn.primary {
        background: var(--red);
        color: var(--bg);
        border-color: var(--red);
    }
    .btn.primary:hover {
        filter: brightness(1.1);
    }
</style>
