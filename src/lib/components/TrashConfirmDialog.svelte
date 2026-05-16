<script lang="ts">
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

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') oncancel();
        if (e.key === 'Enter') {
            e.preventDefault();
            confirm();
        }
    }
</script>

{#if visible}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-overlay" onclick={oncancel} onkeydown={handleKeydown}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dialog" onclick={(e) => e.stopPropagation()} onkeydown={handleKeydown}>
        <div class="dialog-header">
            <h3>Move to Trash</h3>
            <button class="close-btn" onclick={oncancel}>&times;</button>
        </div>

        <div class="dialog-body">
            <p class="confirm-text">Move <strong class="file-name" title={fileName}>{fileName}</strong> to Trash?</p>

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
            <button class="btn primary" onclick={confirm}>Move to Trash</button>
        </div>
    </div>
</div>
{/if}

<style>
    .dialog-overlay {
        position: fixed;
        inset: 0;
        background: color-mix(in srgb, var(--bg) 80%, transparent);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
    }
    .dialog {
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
