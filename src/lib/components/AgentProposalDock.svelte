<script lang="ts">
    import type {
        AgentActionProposal,
        AgentSelectionPreset,
        AgentVisualLevel,
    } from '$lib/api';

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
        visualLevel,
        activePresetId,
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
        visualLevel: AgentVisualLevel;
        activePresetId: string | null;
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

    const activeProposal = $derived(proposals.find(p => p.status === 'pending') ?? null);
    const activePreset = $derived(
        presets.find(p => p.id === activePresetId) ?? presets[0] ?? null,
    );
    const candidates = $derived(parseCandidates(activeProposal?.items_json));
    const contextLabel = $derived(visualLevel === 'text'
        ? 'Text-only'
        : visualLevel[0].toUpperCase() + visualLevel.slice(1));

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
                <strong>Claude Agent</strong>
                <span>{activeProposal?.lens ?? activePreset?.purpose ?? 'selection'} - proposal mode</span>
            </div>
            <div class="header-actions">
                <button class="icon-button" type="button" title={pinned ? 'Float panel' : 'Pin panel'} onclick={onpintoggle}>{pinned ? ']' : '['}</button>
                <button class="icon-button" type="button" title="Close" onclick={onclose}>x</button>
            </div>
        </header>

        <button class="context-chip" type="button" title="Change visual level" onclick={onvisuallevelcycle}>
            Context: {contextLabel} - EUR {activeProposal?.estimated_cost_eur?.toFixed(3) ?? '0.000'} est - {activeProposal?.estimated_input_tokens ?? 0} tokens
        </button>

        <section class="chat-box" aria-label="Agent chat">
            <div class="section-header">
                <span>Chat</span>
                <span>{selectedCount} selected</span>
            </div>
            <textarea
                bind:value={instruction}
                placeholder="Ask for a selection proposal or edit the active preset"
                rows="3"
            ></textarea>
            <button class="primary" type="button" onclick={submitInstruction} disabled={!instruction.trim() || busy}>
                {busy ? 'Asking Claude' : 'Ask Claude'}
            </button>
            {#if lastMessage}
                <p class="agent-message">{lastMessage}</p>
            {/if}
        </section>

        <section class="preset-box" aria-label="Selection presets">
            <div class="section-header">
                <span>Presets</span>
                <span>{presets.length}</span>
            </div>
            <div class="preset-list">
                {#each presets as preset}
                    <article class:active={preset.id === activePreset?.id} class="preset-item">
                        <button type="button" class="preset-main" onclick={() => onselectpreset(preset.id)}>
                            <strong>{preset.name}</strong>
                            <span>{preset.purpose}</span>
                        </button>
                        <button class="small-button" type="button" onclick={() => startEditPreset(preset)}>Edit</button>
                    </article>
                {/each}
            </div>

            {#if editingPresetId}
                <div class="preset-editor">
                    <textarea bind:value={presetPromptDraft} rows="4" aria-label="Preset prompt"></textarea>
                    <div class="editor-actions">
                        <button type="button" onclick={() => { editingPresetId = null; presetPromptDraft = ''; }}>Cancel</button>
                        <button class="primary" type="button" onclick={savePreset} disabled={!presetPromptDraft.trim()}>Save preset</button>
                    </div>
                </div>
            {/if}
        </section>

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
                <p>Choose a preset, then ask for a selection proposal.</p>
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
        gap: var(--spacing);
        min-width: 320px;
        padding: var(--spacing);
    }

    .agent-dock.floating {
        bottom: 36px;
        box-shadow: 0 20px 70px rgba(0, 0, 0, 0.48);
        position: fixed;
        right: 16px;
        top: 56px;
        width: 360px;
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
        padding: 6px 8px;
    }

    button:disabled {
        cursor: default;
        opacity: 0.45;
    }

    textarea {
        min-width: 0;
        padding: 8px;
        resize: vertical;
        width: 100%;
    }

    .context-chip {
        color: var(--text-secondary);
        padding: 6px 8px;
        text-align: left;
    }

    .icon-button {
        min-width: 28px;
    }

    .primary,
    .persona-row span.active {
        border-color: var(--blue);
        color: var(--blue);
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
    .preset-editor {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: var(--spacing);
    }

    .chat-box,
    .preset-box,
    .preset-editor {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
    }

    .preset-list,
    .candidate-list {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
    }

    .preset-item {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        display: grid;
        gap: var(--spacing);
        grid-template-columns: 1fr auto;
        padding: 4px;
    }

    .preset-item.active {
        border-color: var(--green);
    }

    .preset-main {
        border: none;
        text-align: left;
    }

    .small-button {
        font-size: 11px;
    }

    .candidate {
        display: grid;
        grid-template-columns: 40px 1fr;
        gap: var(--spacing);
    }

    .mini-thumb {
        aspect-ratio: 1 / 1;
        background: var(--border);
        border-radius: var(--radius);
    }
</style>
