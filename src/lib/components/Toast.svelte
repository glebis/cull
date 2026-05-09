<script lang="ts">
    import { toasts } from '$lib/stores';

    function dismiss(id: number) {
        toasts.update(t => t.filter(x => x.id !== id));
    }
</script>

{#if $toasts.length > 0}
    <div class="toast-container">
        {#each $toasts as toast (toast.id)}
            <div class="toast toast-{toast.type}">
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
{/if}

<style>
    .toast-container {
        position: fixed;
        top: 48px;
        right: 16px;
        z-index: 9999;
        display: flex;
        flex-direction: column;
        gap: 8px;
        max-width: 360px;
    }
    .toast {
        background: var(--surface, #0c0c12);
        border: 1px solid var(--border, #1a1a2e);
        border-radius: 4px;
        padding: 10px 14px;
        animation: toast-in 0.2s ease-out;
    }
    .toast-success {
        border-left: 3px solid var(--green, #9ece6a);
    }
    .toast-info {
        border-left: 3px solid var(--blue, #7aa2f7);
    }
    .toast-warning {
        border-left: 3px solid var(--orange, #e0af68);
    }
    .toast-error {
        border-left: 3px solid var(--red, #f7768e);
    }
    .toast-message {
        font-size: 12px;
        font-weight: 700;
        color: var(--text-primary, #e0e0e0);
    }
    .toast-detail {
        font-size: 10px;
        color: var(--text-secondary, #565f89);
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
        color: var(--accent, #8cc63f);
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
