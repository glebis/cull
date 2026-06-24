<script lang="ts">
    import type {
        AgentActionProposal,
        AgentSelectionPreset,
        ClaudeAgentStreamEvent,
        AgentVisualLevel,
    } from '$lib/api';
    import { estimateAgentBudget } from '$lib/agent-token-estimate';

    type Candidate = {
        image_id: string;
        reason?: string;
        confidence?: number | string;
    };

    let {
        proposals,
        presets,
        selectedCount,
        pinned,
        visible,
        busy = false,
        lastMessage = null,
        streamEvents = [],
        visualLevel,
        activePresetId,
        activeProposalId = null,
        candidateCount = 0,
        onreviewproposal = () => {},
        ondismissproposal = () => {},
        oncreateproposal = () => {},
        onupdatepreset = () => {},
        onselectpreset = () => {},
        onvisuallevelcycle = () => {},
        onclose = () => {},
        onpintoggle = () => {},
    }: {
        proposals: AgentActionProposal[];
        presets: AgentSelectionPreset[];
        selectedCount: number;
        pinned: boolean;
        visible: boolean;
        busy?: boolean;
        lastMessage?: string | null;
        streamEvents?: ClaudeAgentStreamEvent[];
        visualLevel: AgentVisualLevel;
        activePresetId: string | null;
        activeProposalId?: string | null;
        candidateCount?: number;
        onreviewproposal?: (proposalId: string) => void;
        ondismissproposal?: (proposalId: string) => void;
        oncreateproposal?: (presetId: string | null, instruction: string) => void;
        onupdatepreset?: (presetId: string, prompt: string) => void;
        onselectpreset?: (presetId: string) => void;
        onvisuallevelcycle?: () => void;
        onclose?: () => void;
        onpintoggle?: () => void;
    } = $props();

    let instruction = $state('');
    let editingPresetId = $state<string | null>(null);
    let presetPromptDraft = $state('');

    const activeProposal = $derived(
        (activeProposalId ? proposals.find(p => p.id === activeProposalId) : null)
            ?? proposals.find(p => p.status === 'pending')
            ?? null,
    );
    const activePreset = $derived(
        presets.find(p => p.id === activePresetId) ?? presets[0] ?? null,
    );
    const editingPreset = $derived(
        editingPresetId ? presets.find(p => p.id === editingPresetId) ?? null : null,
    );
    const candidates = $derived(parseCandidates(activeProposal?.items_json));
    const contextLabel = $derived(visualLevel === 'text'
        ? 'Text-only'
        : visualLevel[0].toUpperCase() + visualLevel.slice(1));
    const draftEstimate = $derived(estimateAgentBudget({
        candidateCount,
        instruction,
        visualLevel,
    }));
    const visibleStreamEvents = $derived(streamEvents.slice(-6));
    const displayInputTokens = $derived(activeProposal?.estimated_input_tokens ?? draftEstimate.inputTokens);
    const displayCostEur = $derived(activeProposal?.estimated_cost_eur ?? draftEstimate.costEur);

    function parseCandidates(itemsJson: string | undefined): Candidate[] {
        if (!itemsJson) return [];
        try {
            const parsed = JSON.parse(itemsJson);
            return Array.isArray(parsed) ? parsed : [];
        } catch {
            return [];
        }
    }

    function startEditPreset(preset: AgentSelectionPreset) {
        editingPresetId = preset.id;
        presetPromptDraft = preset.prompt;
    }

    function startEditActivePreset() {
        if (!activePreset) return;
        startEditPreset(activePreset);
    }

    function selectPreset(presetId: string) {
        onselectpreset(presetId);
        if (editingPresetId && editingPresetId !== presetId) {
            editingPresetId = null;
            presetPromptDraft = '';
        }
    }

    function savePreset() {
        if (!editingPresetId || !presetPromptDraft.trim()) return;
        onupdatepreset(editingPresetId, presetPromptDraft.trim());
        editingPresetId = null;
        presetPromptDraft = '';
    }

    function submitInstruction() {
        const message = instruction.trim();
        if (!message) return;
        oncreateproposal(activePreset?.id ?? null, message);
        instruction = '';
    }
</script>

{#if visible || pinned}
    <aside class:pinned class:floating={!pinned} class="agent-dock" aria-label="Agent chat and proposal panel">
        <header class="agent-header">
            <div>
                <strong>AI Agent</strong>
                <span>{activeProposal?.lens ?? activePreset?.purpose ?? 'selection'} proposal</span>
            </div>
            <div class="header-actions">
                <button class="icon-button" type="button" title={pinned ? 'Float panel' : 'Pin panel'} onclick={onpintoggle}>{pinned ? ']' : '['}</button>
                <button class="icon-button" type="button" title="Close" onclick={onclose}>x</button>
            </div>
        </header>

        <button class="context-chip" type="button" title="Change visual level" onclick={onvisuallevelcycle}>
            <span>Context {contextLabel}</span>
            <span>EUR {displayCostEur.toFixed(3)}</span>
            <span>{displayInputTokens} tok</span>
        </button>

        <section class="preset-box" aria-label="Selection presets">
            <div class="section-header">
                <span>Job</span>
                <button class="link-button" type="button" onclick={startEditActivePreset} disabled={!activePreset}>Edit active</button>
            </div>
            {#if presets.length > 0}
                <div class="preset-grid">
                    {#each presets as preset}
                        <button
                            type="button"
                            aria-pressed={preset.id === activePreset?.id}
                            class:active={preset.id === activePreset?.id}
                            class="preset-item"
                            title={preset.prompt}
                            onclick={() => selectPreset(preset.id)}
                        >
                            <span class="preset-main">
                                <strong>{preset.name}</strong>
                                <span>{preset.purpose}</span>
                            </span>
                        </button>
                    {/each}
                </div>
            {:else}
                <p class="empty-note">No jobs loaded.</p>
            {/if}

            {#if editingPresetId}
                <div class="preset-editor">
                    <div class="section-header">
                        <span>Editing {editingPreset?.name ?? 'preset'}</span>
                        <span>{presetPromptDraft.length} chars</span>
                    </div>
                    <textarea bind:value={presetPromptDraft} rows="4" aria-label="Preset prompt"></textarea>
                    <div class="editor-actions">
                        <button type="button" onclick={() => { editingPresetId = null; presetPromptDraft = ''; }}>Cancel</button>
                        <button class="primary" type="button" onclick={savePreset} disabled={!presetPromptDraft.trim()}>Save preset</button>
                    </div>
                </div>
            {/if}
        </section>

        <section class="chat-box" aria-label="Agent chat">
            <div class="section-header">
                <span>Request</span>
                <span>{selectedCount} selected</span>
            </div>
            <textarea
                bind:value={instruction}
                placeholder="Describe the proposal you need"
                rows="4"
            ></textarea>
            <button class="primary" type="button" onclick={submitInstruction} disabled={!instruction.trim() || busy}>
                {busy ? 'Asking Claude' : 'Create proposal'}
            </button>
            {#if lastMessage}
                <p class="agent-message">{lastMessage}</p>
            {/if}
        </section>

        {#if visibleStreamEvents.length > 0}
            <section class="stream-box" aria-label="Agent run events" aria-live="polite">
                <div class="section-header">
                    <span>Run</span>
                    <span>{busy ? 'live' : 'done'}</span>
                </div>
                <ol class="stream-list">
                    {#each visibleStreamEvents as event}
                        <li class:error={event.is_error} class:final={event.is_final}>
                            <span class="stream-dot"></span>
                            <div>
                                <strong>{event.phase.replace('sdk_', '')}</strong>
                                <span>{event.message}</span>
                            </div>
                        </li>
                    {/each}
                </ol>
            </section>
        {/if}

        {#if activeProposal}
            <section class="summary">
                <div class="persona-row">
                    <span class:active={activeProposal.persona === 'curator'}>Curator</span>
                    <span class:active={activeProposal.persona === 'copilot'}>Copilot</span>
                    <span class:active={activeProposal.persona === 'operator'}>Operator</span>
                </div>
                <h3>{activeProposal.kind === 'trash_images' ? 'Trash proposal ready' : 'Selection proposal ready'}</h3>
                <p>{activeProposal.criteria}</p>
                <div class="proposal-actions">
                    <button class="primary" type="button" onclick={() => onreviewproposal(activeProposal.id)}>
                        Open review gate
                    </button>
                    <button type="button" onclick={() => ondismissproposal(activeProposal.id)}>Dismiss</button>
                </div>
            </section>

            <section class="candidate-list" aria-label="Candidate reasons">
                {#each candidates.slice(0, 5) as candidate}
                    <article class="candidate">
                        <div class="mini-thumb"></div>
                        <div>
                            <strong>{candidate.image_id}</strong>
                            <span>{candidate.reason ?? 'Candidate selected by proposal criteria'}</span>
                        </div>
                    </article>
                {/each}
            </section>
        {:else}
            <section class="empty">
                <h3>No active proposal</h3>
                <p>Pick a job, write the request, then create a proposal.</p>
            </section>
        {/if}
    </aside>
{/if}

<style>
    .agent-dock {
        background: var(--bg);
        border-left: 1px solid var(--border);
        color: var(--text);
        display: flex;
        flex-direction: column;
        gap: calc(var(--spacing) * 0.75);
        min-width: 304px;
        padding: calc(var(--spacing) * 0.75);
    }

    .agent-dock.floating {
        bottom: 36px;
        box-shadow: 0 20px 70px color-mix(in srgb, var(--bg) 70%, transparent);
        position: fixed;
        right: 16px;
        top: 56px;
        width: 372px;
        z-index: 20;
    }

    .agent-header,
    .header-actions,
    .persona-row,
    .proposal-actions,
    .editor-actions,
    .section-header {
        align-items: center;
        display: flex;
        gap: var(--spacing);
    }

    .agent-header,
    .proposal-actions,
    .editor-actions,
    .section-header {
        justify-content: space-between;
    }

    .agent-header {
        min-height: 32px;
    }

    .agent-header strong {
        display: block;
        font-size: 14px;
        line-height: 1.2;
    }

    .agent-header span,
    .summary p,
    .candidate span,
    .empty p,
    .agent-message,
    .section-header,
    .preset-main span {
        color: var(--text-secondary);
        display: block;
        font-size: 11px;
    }

    button,
    textarea,
    .context-chip {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font: inherit;
    }

    button {
        cursor: pointer;
        padding: 5px 7px;
        transition: background 120ms ease, border-color 120ms ease, transform 80ms ease;
    }

    button:hover:not(:disabled) {
        border-color: var(--text-secondary);
    }

    button:active:not(:disabled) {
        transform: translateY(1px);
    }

    button:focus-visible,
    textarea:focus-visible {
        border-color: var(--blue);
        outline: 1px solid var(--blue);
        outline-offset: 1px;
    }

    button:disabled {
        cursor: default;
        opacity: 0.45;
    }

    textarea {
        background: var(--bg);
        line-height: 1.35;
        min-width: 0;
        padding: 7px;
        resize: vertical;
        width: 100%;
    }

    .context-chip {
        align-items: center;
        color: var(--text-secondary);
        display: grid;
        font-size: 11px;
        gap: var(--spacing);
        grid-template-columns: minmax(0, 1fr) auto auto;
        padding: 5px 7px;
        text-align: left;
    }

    .icon-button {
        min-height: 28px;
        min-width: 28px;
    }

    .primary,
    .persona-row span.active {
        background: color-mix(in srgb, var(--blue) 10%, var(--surface));
        border-color: var(--blue);
        color: var(--blue);
    }

    .primary {
        min-height: 32px;
    }

    .persona-row span {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text-secondary);
        font-size: 10px;
        padding: 4px 6px;
    }

    .summary,
    .candidate,
    .empty,
    .chat-box,
    .preset-box,
    .stream-box {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: calc(var(--spacing) * 0.75);
    }

    .chat-box,
    .preset-box,
    .preset-editor {
        display: flex;
        flex-direction: column;
        gap: calc(var(--spacing) * 0.75);
    }

    .preset-grid {
        display: grid;
        gap: calc(var(--spacing) * 0.5);
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .candidate-list {
        display: flex;
        flex-direction: column;
        gap: calc(var(--spacing) * 0.75);
    }

    .stream-list {
        display: flex;
        flex-direction: column;
        gap: calc(var(--spacing) * 0.5);
        list-style: none;
        margin: calc(var(--spacing) * 0.75) 0 0;
        padding: 0;
    }

    .stream-list li {
        display: grid;
        gap: calc(var(--spacing) * 0.75);
        grid-template-columns: 8px 1fr;
        min-width: 0;
    }

    .stream-dot {
        align-self: start;
        background: var(--blue);
        border-radius: 50%;
        height: 6px;
        margin-top: 5px;
        width: 6px;
    }

    .stream-list li.final .stream-dot {
        background: var(--green);
    }

    .stream-list li.error .stream-dot {
        background: var(--red);
    }

    .stream-list strong {
        color: var(--text-secondary);
        display: block;
        font-size: 10px;
        line-height: 1.2;
        text-transform: uppercase;
    }

    .stream-list span {
        color: var(--text);
        display: block;
        font-size: 11px;
        line-height: 1.3;
        overflow-wrap: anywhere;
    }

    .preset-item {
        align-items: stretch;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        display: block;
        min-height: 45px;
        min-width: 0;
        padding: 6px 7px;
        text-align: left;
    }

    .preset-item.active {
        background: color-mix(in srgb, var(--green) 9%, var(--surface));
        border-color: var(--green);
    }

    .preset-main {
        min-width: 0;
    }

    .preset-main strong {
        display: block;
        font-size: 12px;
        line-height: 1.25;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .preset-main span {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .link-button {
        background: transparent;
        border: 0;
        color: var(--blue);
        font-size: 11px;
        padding: 0;
    }

    .preset-editor {
        border-top: 1px solid var(--border);
        margin-top: calc(var(--spacing) * 0.25);
        padding-top: calc(var(--spacing) * 0.75);
    }

    .editor-actions button {
        min-width: 76px;
    }

    .summary h3,
    .empty h3 {
        font-size: 13px;
        line-height: 1.25;
        margin: 6px 0 4px;
    }

    .summary p,
    .empty p,
    .empty-note,
    .agent-message {
        line-height: 1.35;
        margin: 0;
    }

    .candidate {
        display: grid;
        grid-template-columns: 40px 1fr;
        gap: calc(var(--spacing) * 0.75);
    }

    .mini-thumb {
        aspect-ratio: 1 / 1;
        background: var(--border);
        border-radius: var(--radius);
    }

    @media (max-width: 420px) {
        .agent-dock.floating {
            left: 8px;
            right: 8px;
            width: auto;
        }
    }
</style>
