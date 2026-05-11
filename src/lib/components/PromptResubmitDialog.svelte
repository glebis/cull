<script lang="ts">
    import { resubmitPrompt, estimateGenerationCost, type CostEstimate } from '$lib/api';

    interface Props {
        visible: boolean;
        initialPrompt: string;
        sourceImageId: string | null;
        onclose: () => void;
        ongenerated: (imageIds: string[], jobId: string) => void;
    }

    let { visible, initialPrompt, sourceImageId, onclose, ongenerated }: Props = $props();

    let prompt = $state('');
    let model = $state('gpt-image-2');
    let size = $state('1024x1024');
    let quality = $state('auto');
    let n = $state(4);
    let submitting = $state(false);
    let error = $state<string | null>(null);
    let costEstimate = $state<CostEstimate | null>(null);

    const SIZES = ['1024x1024', '1024x1536', '1536x1024', 'auto'];
    const QUALITIES = ['auto', 'low', 'high'];

    $effect(() => {
        if (visible) {
            prompt = initialPrompt;
            error = null;
            submitting = false;
        }
    });

    $effect(() => {
        if (visible) {
            estimateGenerationCost(model, size, quality, n)
                .then(c => costEstimate = c)
                .catch(() => costEstimate = null);
        }
    });

    async function updateCost() {
        try {
            costEstimate = await estimateGenerationCost(model, size, quality, n);
        } catch {
            costEstimate = null;
        }
    }

    async function submit() {
        if (!prompt.trim() || submitting) return;
        submitting = true;
        error = null;
        try {
            const resp = await resubmitPrompt({
                source_image_id: sourceImageId,
                prompt: prompt.trim(),
                n,
                model,
                size,
                quality,
            });
            ongenerated([], resp.job_id);
            onclose();
        } catch (e: any) {
            error = e?.toString() ?? 'Generation failed';
            submitting = false;
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') onclose();
        if (e.key === 'Enter' && e.metaKey) submit();
    }
</script>

{#if visible}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-overlay" onclick={onclose} onkeydown={handleKeydown}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dialog" onclick={(e) => e.stopPropagation()} onkeydown={handleKeydown}>
        <div class="dialog-header">
            <h3>Re-generate</h3>
            <button class="close-btn" onclick={onclose}>&times;</button>
        </div>

        <div class="dialog-body">
            <label class="field">
                <span class="field-label">Prompt</span>
                <textarea
                    bind:value={prompt}
                    rows={4}
                    placeholder="Describe the image..."
                ></textarea>
            </label>

            <div class="settings-row">
                <label class="field compact">
                    <span class="field-label">Size</span>
                    <select bind:value={size} onchange={updateCost}>
                        {#each SIZES as s}
                            <option value={s}>{s}</option>
                        {/each}
                    </select>
                </label>

                <label class="field compact">
                    <span class="field-label">Quality</span>
                    <select bind:value={quality} onchange={updateCost}>
                        {#each QUALITIES as q}
                            <option value={q}>{q}</option>
                        {/each}
                    </select>
                </label>

                <label class="field compact">
                    <span class="field-label">Variations</span>
                    <select bind:value={n} onchange={updateCost}>
                        {#each [1, 2, 3, 4] as v}
                            <option value={v}>{v}</option>
                        {/each}
                    </select>
                </label>
            </div>

            {#if costEstimate}
                <div class="cost-estimate">
                    Estimated cost: ~${costEstimate.estimated_cost.toFixed(3)}
                </div>
            {/if}

            {#if error}
                <div class="error-msg">{error}</div>
            {/if}
        </div>

        <div class="dialog-footer">
            <button class="btn secondary" onclick={onclose}>Cancel</button>
            <button class="btn primary" onclick={submit} disabled={submitting || !prompt.trim()}>
                {submitting ? 'Generating...' : `Generate ${n} variation${n > 1 ? 's' : ''}`}
            </button>
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
        width: 480px;
        max-width: 90vw;
        max-height: 80vh;
        overflow-y: auto;
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
    .field {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .field-label {
        font-size: 11px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 0.04em;
    }
    textarea, select {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font-family: var(--font);
        font-size: 13px;
        padding: var(--spacing);
        resize: vertical;
    }
    textarea:focus, select:focus {
        outline: none;
        border-color: var(--blue);
    }
    .settings-row {
        display: flex;
        gap: var(--spacing);
    }
    .compact { flex: 1; }
    .compact select { width: 100%; }
    .cost-estimate {
        font-size: 12px;
        color: var(--text-secondary);
        padding: var(--spacing);
        background: var(--bg);
        border-radius: var(--radius);
        text-align: center;
    }
    .error-msg {
        font-size: 12px;
        color: var(--red);
        padding: var(--spacing);
        background: color-mix(in srgb, var(--red) 10%, transparent);
        border-radius: var(--radius);
    }
    .dialog-footer {
        display: flex;
        justify-content: flex-end;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
        border-top: 1px solid var(--border);
    }
    .btn {
        padding: var(--spacing) calc(var(--spacing) * 2);
        border-radius: var(--radius);
        font-size: 13px;
        font-family: var(--font);
        cursor: pointer;
        border: 1px solid var(--border);
    }
    .btn.secondary {
        background: var(--bg);
        color: var(--text-secondary);
    }
    .btn.primary {
        background: var(--blue);
        color: var(--bg);
        border-color: var(--blue);
    }
    .btn.primary:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .btn:hover:not(:disabled) {
        filter: brightness(1.1);
    }
</style>
