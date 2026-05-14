<script lang="ts">
    import { tick } from 'svelte';
    import { resolveTextInputDialog, textInputDialog } from '$lib/stores';

    let value = $state('');
    let error = $state('');
    let currentId = $state(0);
    let inputEl: HTMLInputElement | undefined = $state();

    $effect(() => {
        const request = $textInputDialog;
        if (!request) {
            currentId = 0;
            return;
        }
        if (request.id === currentId) return;

        currentId = request.id;
        value = request.initialValue ?? '';
        error = '';

        tick().then(() => {
            inputEl?.focus();
            inputEl?.select();
        });
    });

    function cancel() {
        resolveTextInputDialog(null);
    }

    function submit() {
        const request = $textInputDialog;
        if (!request) return;

        const trimmed = value.trim();
        if (!trimmed) {
            error = `${request.label ?? 'Name'} is required`;
            tick().then(() => inputEl?.focus());
            return;
        }

        resolveTextInputDialog(trimmed);
    }

    function handleKeydown(e: KeyboardEvent) {
        e.stopPropagation();
        if (e.key === 'Escape') {
            e.preventDefault();
            cancel();
        }
        if (e.key === 'Enter') {
            e.preventDefault();
            submit();
        }
    }
</script>

{#if $textInputDialog}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-overlay" onclick={cancel} onkeydown={handleKeydown}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <dialog
        open
        class="dialog"
        aria-modal="true"
        aria-labelledby="text-input-dialog-title"
        aria-describedby={$textInputDialog.description ? 'text-input-dialog-description' : undefined}
        onclick={(e: MouseEvent) => e.stopPropagation()}
        onkeydown={handleKeydown}
    >
        <div class="dialog-header">
            <h3 id="text-input-dialog-title">{$textInputDialog.title}</h3>
            <button class="close-btn" onclick={cancel} aria-label="Close">&times;</button>
        </div>

        <div class="dialog-body">
            {#if $textInputDialog.description}
                <p id="text-input-dialog-description" class="description">{$textInputDialog.description}</p>
            {/if}

            <label class="field-label" for="text-input-dialog-input">
                {$textInputDialog.label ?? 'Name'}
            </label>
            <input
                id="text-input-dialog-input"
                bind:this={inputEl}
                bind:value={value}
                class="name-input"
                type="text"
                placeholder={$textInputDialog.placeholder ?? ''}
                autocomplete="off"
                spellcheck="false"
                aria-invalid={error ? 'true' : 'false'}
                aria-describedby={error ? 'text-input-dialog-error' : undefined}
            />

            {#if error}
                <div id="text-input-dialog-error" class="error" role="alert">{error}</div>
            {/if}
        </div>

        <div class="dialog-footer">
            <button class="btn secondary" onclick={cancel}>{$textInputDialog.cancelLabel ?? 'Cancel'}</button>
            <button class="btn primary" onclick={submit} disabled={!value.trim()}>
                {$textInputDialog.confirmLabel ?? 'Save'}
            </button>
        </div>
    </dialog>
</div>
{/if}

<style>
    .dialog-overlay {
        position: fixed;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: calc(var(--spacing) * 2);
        background: color-mix(in srgb, var(--bg) 78%, transparent);
        z-index: 11000;
    }

    .dialog {
        position: static;
        display: block;
        width: min(420px, 100%);
        margin: 0;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        box-shadow: 0 18px 60px color-mix(in srgb, var(--bg) 82%, transparent);
        outline: none;
    }

    .dialog-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
        border-bottom: 1px solid var(--border);
    }

    h3 {
        margin: 0;
        color: var(--text);
        font-size: 14px;
        font-weight: 700;
    }

    .close-btn {
        border: none;
        background: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-family: var(--font);
        font-size: 18px;
        line-height: 1;
        padding: 0 4px;
    }

    .close-btn:hover {
        color: var(--text);
    }

    .dialog-body {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
    }

    .description {
        margin: 0 0 2px;
        color: var(--text-secondary);
        font-size: 12px;
    }

    .field-label {
        color: var(--blue);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }

    .name-input {
        width: 100%;
        min-width: 0;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font-family: var(--font);
        font-size: 13px;
        padding: 8px 10px;
    }

    .name-input:focus {
        border-color: var(--blue);
        outline: none;
    }

    .error {
        color: var(--red);
        font-size: 11px;
    }

    .dialog-footer {
        display: flex;
        justify-content: flex-end;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
        border-top: 1px solid var(--border);
    }

    .btn {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        cursor: pointer;
        font-family: var(--font);
        font-size: 12px;
        padding: 6px 14px;
    }

    .btn:disabled {
        cursor: not-allowed;
        opacity: 0.5;
    }

    .btn.secondary {
        background: var(--surface);
        color: var(--text-secondary);
    }

    .btn.secondary:hover {
        border-color: var(--text-secondary);
        color: var(--text);
    }

    .btn.primary {
        background: color-mix(in srgb, var(--green) 16%, var(--surface));
        border-color: var(--green);
        color: var(--green);
    }

    .btn.primary:hover:not(:disabled) {
        background: color-mix(in srgb, var(--green) 24%, var(--surface));
    }
</style>
