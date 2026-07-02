<script lang="ts">
    import { toasts } from '$lib/stores';

    function dismiss(id: number) {
        toasts.update(t => t.filter(x => x.id !== id));
    }
</script>

<!-- Always-rendered polite live region so screen readers announce toasts;
     error toasts escalate to role="alert" (implicitly assertive). -->
<div class="toast-container" role="status" aria-live="polite">
    {#each $toasts as toast (toast.id)}
        <div class="toast toast-{toast.type}" role={toast.type === 'error' ? 'alert' : undefined}>
            <div class="toast-message">{toast.message}</div>
            {#if toast.detail}
                <div class="toast-detail">{toast.detail}</div>
            {/if}
            {#if toast.actions && toast.actions.length > 0}
                <div class="toast-actions">
                    {#each toast.actions as action}
                        <button class="toast-action-btn" onclick={() => { action.onclick(); dismiss(toast.id); }}>
                            {action.label}
                        </button>
                    {/each}
                </div>
            {/if}
        </div>
    {/each}
</div>

<style>
    .toast-container {
        position: fixed;
        top: 48px;
        right: 16px;
        z-index: var(--z-toast);
        display: flex;
        flex-direction: column;
        gap: 8px;
        max-width: 360px;
    }
    .toast {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 4px;
        padding: 10px 14px;
        animation: toast-in 0.2s ease-out;
    }
    .toast-success {
        border-color: var(--green);
    }
    .toast-info {
        border-color: var(--blue);
    }
    .toast-warning {
        border-color: var(--orange);
    }
    .toast-error {
        border-color: var(--red);
    }
    .toast-message {
        font-size: 12px;
        font-weight: 700;
        color: var(--text);
    }
    .toast-detail {
        font-size: 10px;
        color: var(--text-secondary);
        margin-top: 3px;
    }
    .toast-actions {
        display: flex;
        gap: 8px;
        margin-top: 4px;
    }
    .toast-action-btn {
        background: none;
        border: none;
        color: var(--green);
        cursor: pointer;
        font-size: 12px;
        padding: 0;
        text-decoration: underline;
    }
    .toast-action-btn:hover {
        opacity: 0.8;
    }
    @keyframes toast-in {
        from { opacity: 0; transform: translateX(20px); }
        to { opacity: 1; transform: translateX(0); }
    }
</style>
