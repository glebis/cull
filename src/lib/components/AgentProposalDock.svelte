<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import type {
        AgentActionProposal,
        AgentSelectionPreset,
        ClaudeAgentStreamEvent,
        AgentVisualLevel,
        ImageWithFile,
    } from '$lib/api';
    import { estimateAgentBudget } from '$lib/agent-token-estimate';
    import {
        parseAgentProposalSourceContext,
        proposalActorLabel,
        sourceContextIsStale,
        sourceContextScopeLabel,
        type AgentProposalViewContext,
    } from '$lib/agent-proposal-context';
    import { safeAssetPreviewPath } from '$lib/view-utils';

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
        lastInstruction = null,
        streamEvents = [],
        visualLevel,
        activePresetId,
        activeProposalId = null,
        candidateCount = 0,
        currentViewContext = null,
        visibleImages = [],
        onreviewproposal = () => {},
        ondismissproposal = () => {},
        oncreateproposal = () => {},
        onupdatepreset = () => {},
        onselectpreset = () => {},
        onselectproposal = () => {},
        onvisuallevelcycle = () => {},
        oncancelturn = () => {},
        onclose = () => {},
    }: {
        proposals: AgentActionProposal[];
        presets: AgentSelectionPreset[];
        selectedCount: number;
        pinned: boolean;
        visible: boolean;
        busy?: boolean;
        lastMessage?: string | null;
        lastInstruction?: string | null;
        streamEvents?: ClaudeAgentStreamEvent[];
        visualLevel: AgentVisualLevel;
        activePresetId: string | null;
        activeProposalId?: string | null;
        candidateCount?: number;
        currentViewContext?: AgentProposalViewContext | null;
        visibleImages?: ImageWithFile[];
        onreviewproposal?: (proposalId: string) => void;
        ondismissproposal?: (proposalId: string) => void;
        oncreateproposal?: (presetId: string | null, instruction: string) => void;
        onupdatepreset?: (presetId: string, prompt: string) => void;
        onselectpreset?: (presetId: string) => void;
        onselectproposal?: (proposalId: string) => void;
        onvisuallevelcycle?: () => void;
        oncancelturn?: () => void;
        onclose?: () => void;
    } = $props();

    let instruction = $state('');
    let editingPresetId = $state<string | null>(null);
    let presetPromptDraft = $state('');

    const activeProposal = $derived(
        (activeProposalId ? proposals.find(p => p.id === activeProposalId) : null)
            ?? proposals.find(p => p.status === 'pending')
            ?? null,
    );
    const pendingProposals = $derived(proposals.filter(p => p.status === 'pending'));
    const activePreset = $derived(
        presets.find(p => p.id === activePresetId) ?? presets[0] ?? null,
    );
    const editingPreset = $derived(
        editingPresetId ? presets.find(p => p.id === editingPresetId) ?? null : null,
    );
    const candidates = $derived(parseCandidates(activeProposal?.items_json));
    const candidateCountLabel = $derived(`${candidates.length} ${candidates.length === 1 ? 'image' : 'images'}`);
    const primaryCandidate = $derived(candidates[0] ?? null);
    const primaryImage = $derived(primaryCandidate ? imageForCandidate(primaryCandidate.image_id) : null);
    const primaryImagePreviewPath = $derived(primaryImage ? safeAssetPreviewPath(primaryImage, { displayPx: 120 }) : null);
    const primaryImageSrc = $derived(primaryImagePreviewPath ? convertFileSrc(primaryImagePreviewPath) : null);
    const primaryImageName = $derived(primaryImage ? filenameForPath(primaryImage.path) : null);
    const contextLabel = $derived(visualLevel === 'text'
        ? 'Text-only'
        : visualLevel[0].toUpperCase() + visualLevel.slice(1));
    const draftEstimate = $derived(estimateAgentBudget({
        candidateCount,
        instruction,
        visualLevel,
    }));
    const latestRunEvent = $derived(streamEvents.slice().reverse().find(Boolean) ?? null);
    const latestAssistantEvent = $derived(
        streamEvents.slice().reverse().find(event => ['sdk_assistant', 'sdk_stream'].includes(event.phase) && event.message) ?? null,
    );
    const runStatus = $derived(statusForEvent(latestRunEvent, busy));
    const assistantMessage = $derived(lastMessage ?? (busy ? latestAssistantEvent?.message ?? null : latestRunEvent?.is_error ? latestRunEvent.message : null));
    const showChatThread = $derived(Boolean(lastInstruction || assistantMessage || busy));
    const displayInputTokens = $derived(activeProposal?.estimated_input_tokens ?? draftEstimate.inputTokens);
    const displayCostEur = $derived(activeProposal?.estimated_cost_eur ?? draftEstimate.costEur);
    const activeSourceContext = $derived(parseAgentProposalSourceContext(activeProposal?.source_context_json));
    const proposalSourceLabel = $derived(activeProposal ? proposalActorLabel(activeSourceContext, activeProposal.persona) : '');
    const proposalCreatedLabel = $derived(activeProposal ? formatProposalTimestamp(activeProposal.created_at) : '');
    const proposalScopeLabel = $derived(sourceContextScopeLabel(activeSourceContext) ?? currentViewContext?.label ?? 'Unknown view');
    const proposalIsStale = $derived(sourceContextIsStale(activeSourceContext, currentViewContext));

    function parseCandidates(itemsJson: string | undefined): Candidate[] {
        if (!itemsJson) return [];
        try {
            const parsed = JSON.parse(itemsJson);
            return Array.isArray(parsed) ? parsed : [];
        } catch {
            return [];
        }
    }

    function imageForCandidate(imageId: string) {
        return visibleImages.find(item => item.image.id === imageId) ?? null;
    }

    function filenameForPath(path: string) {
        return path.split('/').pop() ?? path;
    }

    function shortImageId(imageId: string) {
        return imageId.slice(0, 8);
    }

    function formatProposalTimestamp(timestamp: string) {
        const date = new Date(timestamp);
        if (Number.isNaN(date.getTime())) return timestamp;
        const day = date.toLocaleDateString([], { month: 'short', day: 'numeric' });
        const time = date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
        return `${day} ${time}`;
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

    function handleProfileChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement | null;
        if (!target?.value) return;
        selectPreset(target.value);
    }

    function profileSummary(preset: AgentSelectionPreset) {
        const prompt = preset.prompt.replace(/\s+/g, ' ').trim();
        if (!prompt) return 'Claude uses this profile as the criteria for the next proposal.';
        return prompt.length > 150 ? `${prompt.slice(0, 149)}…` : prompt;
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

    function handleInstructionKeydown(event: KeyboardEvent) {
        if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
            event.preventDefault();
            if (!busy) submitInstruction();
        }
    }

    function statusForEvent(event: ClaudeAgentStreamEvent | null, isBusy: boolean) {
        if (!isBusy && event?.is_error) return 'Could not complete the request';
        if (!isBusy) return 'Ready';
        if (!event) return 'Thinking';
        if (event.message && !['sdk_init', 'sdk_status'].includes(event.phase)) return event.message;
        if (event.phase === 'parse') return 'Preparing response';
        if (event.phase === 'context' || event.phase === 'runtime' || event.phase === 'queued') return 'Thinking';
        return 'Thinking';
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
                <button class="close-button" type="button" title="Close" aria-label="Close agent panel" onclick={onclose}>×</button>
            </div>
        </header>

        <button class="context-chip" type="button" title="Change visual level" onclick={onvisuallevelcycle}>
            <span>Context {contextLabel}</span>
            <span>EUR {displayCostEur.toFixed(3)}</span>
            <span>{displayInputTokens} tok</span>
        </button>

        {#if activeProposal}
            <section class="proposal-card" class:stale={proposalIsStale} aria-label="Pending agent proposal">
                <div class="proposal-topline">
                    <span>{activeProposal.kind === 'trash_images' ? 'Trash proposal' : 'Selection proposal'}</span>
                    <strong>Needs approval</strong>
                </div>
                {#if pendingProposals.length > 1}
                    <label class="proposal-switcher" for="agent-proposal-select">
                        <span>{pendingProposals.length} pending</span>
                        <select id="agent-proposal-select" value={activeProposal.id} onchange={(event) => onselectproposal((event.currentTarget as HTMLSelectElement).value)}>
                            {#each pendingProposals as proposal}
                                <option value={proposal.id}>
                                    {proposal.kind === 'trash_images' ? 'Trash' : 'Selection'} · {proposal.lens ?? 'proposal'}
                                </option>
                            {/each}
                        </select>
                    </label>
                {/if}
                <div class="proposal-headline">
                    <h3>{activeProposal.kind === 'trash_images' ? `${candidateCountLabel} proposed for Trash` : `${candidateCountLabel} proposed`}</h3>
                    <span>{contextLabel}</span>
                </div>
                <div class="proposal-meta" class:stale={proposalIsStale}>
                    <span>By {proposalSourceLabel}</span>
                    <span>{proposalCreatedLabel}</span>
                    <span>View: {proposalScopeLabel}</span>
                    {#if proposalIsStale}
                        <strong>Stale view</strong>
                    {/if}
                </div>
                {#if primaryCandidate}
                    <article class="candidate featured-candidate">
                        <div class="candidate-preview" aria-hidden="true">
                            {#if primaryImageSrc}
                                <img src={primaryImageSrc} alt="" loading="lazy" decoding="async" draggable="false" />
                            {:else}
                                <span>{shortImageId(primaryCandidate.image_id)}</span>
                            {/if}
                        </div>
                        <div class="candidate-copy">
                            <span class="candidate-label">Candidate</span>
                            <strong>{primaryImageName ?? shortImageId(primaryCandidate.image_id)}</strong>
                            <span class="candidate-id">{shortImageId(primaryCandidate.image_id)}</span>
                            <p>{primaryCandidate.reason ?? 'Candidate selected by proposal criteria'}</p>
                        </div>
                    </article>
                {/if}
                <div class="proposal-actions">
                    <button class="primary" type="button" onclick={() => onreviewproposal(activeProposal.id)}>
                        Review and apply
                    </button>
                    <button type="button" onclick={() => ondismissproposal(activeProposal.id)}>Dismiss</button>
                </div>
            </section>
        {/if}

        <section class="profile-box" aria-label="Agent profile">
            <div class="section-header">
                <span>Profile</span>
                <button class="link-button" type="button" onclick={startEditActivePreset} disabled={!activePreset}>Edit</button>
            </div>
            {#if presets.length > 0}
                <label class="profile-control" for="agent-profile-select">
                    <span>Selection criteria</span>
                    <select id="agent-profile-select" value={activePreset?.id ?? ''} onchange={handleProfileChange}>
                        {#each presets as preset}
                            <option value={preset.id}>{preset.name}</option>
                        {/each}
                    </select>
                </label>
                {#if activePreset}
                    <p class="profile-summary">{profileSummary(activePreset)}</p>
                {/if}
            {:else}
                <p class="empty-note">No profiles loaded.</p>
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
                <span>Chat</span>
                <span>{selectedCount} selected</span>
            </div>
            {#if showChatThread}
                <div class="chat-thread" aria-live="polite">
                    {#if lastInstruction}
                        <article class="chat-message user-message">
                            <span>You</span>
                            <p>{lastInstruction}</p>
                        </article>
                    {/if}
                    {#if busy || assistantMessage}
                        <article class="chat-message assistant-message" class:thinking={busy && !assistantMessage}>
                            <span>Claude</span>
                            {#if busy && !assistantMessage}
                                <p class="thinking-line"><span class="thinking-dot"></span>{runStatus}</p>
                            {:else if assistantMessage}
                                <p>{assistantMessage}</p>
                            {/if}
                        </article>
                    {/if}
                    {#if streamEvents.length > 0}
                        <details class="activity-log">
                            <summary>Activity</summary>
                            <ol>
                                {#each streamEvents.slice(-12) as event}
                                    <li class:error={event.is_error}>
                                        <strong>{event.phase.replace(/^sdk_/, '')}</strong>
                                        <span>{event.message}</span>
                                    </li>
                                {/each}
                            </ol>
                        </details>
                    {/if}
                </div>
            {/if}
            <textarea
                bind:value={instruction}
                placeholder="Ask Claude to propose a selection"
                rows="3"
                onkeydown={handleInstructionKeydown}
            ></textarea>
            <div class="chat-actions">
                <button class="primary" type="button" onclick={submitInstruction} disabled={!instruction.trim() || busy}>
                    {busy ? 'Thinking' : 'Send'}
                </button>
                {#if busy}
                    <button class="stop-button" type="button" onclick={oncancelturn}>
                        Stop
                    </button>
                {/if}
            </div>
        </section>

        {#if !activeProposal}
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
        min-height: 0;
        min-width: 304px;
        overflow: hidden;
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
    .proposal-actions,
    .chat-actions,
    .editor-actions,
    .section-header,
    .proposal-topline,
    .proposal-headline,
    .proposal-meta {
        align-items: center;
        display: flex;
        gap: var(--spacing);
    }

    .agent-header,
    .proposal-actions,
    .chat-actions,
    .editor-actions,
    .section-header,
    .proposal-topline,
    .proposal-headline {
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
    .candidate span,
    .empty p,
    .section-header,
    .proposal-topline,
    .profile-control span,
    .profile-summary {
        color: var(--text-secondary);
        display: block;
        font-size: 11px;
    }

    button,
    select,
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
    select:focus-visible,
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

    select {
        appearance: none;
        background:
            linear-gradient(45deg, transparent 50%, var(--text-secondary) 50%) right 11px center / 6px 6px no-repeat,
            linear-gradient(135deg, var(--text-secondary) 50%, transparent 50%) right 7px center / 6px 6px no-repeat,
            var(--surface);
        min-height: 32px;
        padding: 5px 26px 5px 7px;
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

    .close-button {
        background: transparent;
        border: 0;
        color: var(--text-secondary);
        font-size: 18px;
        line-height: 1;
        min-height: 28px;
        min-width: 28px;
        padding: 0;
    }

    .close-button:hover:not(:disabled) {
        color: var(--text);
    }

    .primary {
        background: color-mix(in srgb, var(--blue) 10%, var(--surface));
        border-color: var(--blue);
        color: var(--blue);
    }

    .primary {
        min-height: 32px;
    }

    .chat-actions {
        align-items: stretch;
    }

    .chat-actions .primary {
        flex: 1 1 auto;
    }

    .stop-button {
        border-color: var(--red);
        color: var(--red);
        min-height: 32px;
    }

    .stop-button:hover:not(:disabled) {
        background: color-mix(in srgb, var(--red) 18%, transparent);
    }

    .candidate,
    .empty,
    .chat-box,
    .profile-box {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: calc(var(--spacing) * 0.75);
    }

    .proposal-card {
        background:
            linear-gradient(180deg, color-mix(in srgb, var(--green) 10%, var(--surface)), var(--surface) 44%);
        border: 1px solid color-mix(in srgb, var(--green) 65%, var(--border));
        border-left: 3px solid var(--green);
        border-radius: var(--radius);
        padding: calc(var(--spacing) * 1);
    }

    .proposal-card.stale {
        background:
            linear-gradient(180deg, color-mix(in srgb, var(--orange) 12%, var(--surface)), var(--surface) 44%);
        border-color: color-mix(in srgb, var(--orange) 70%, var(--border));
        border-left-color: var(--orange);
    }

    .profile-box,
    .chat-box,
    .empty {
        background: transparent;
    }

    .chat-box,
    .profile-box,
    .proposal-card,
    .preset-editor {
        display: flex;
        flex-direction: column;
        gap: calc(var(--spacing) * 0.75);
    }

    .proposal-card {
        flex: 0 0 auto;
    }

    .profile-box,
    .chat-box {
        border-color: color-mix(in srgb, var(--border) 70%, transparent);
    }

    .profile-box {
        flex: 0 0 auto;
    }

    .chat-box {
        flex: 1 1 auto;
        margin-top: calc(var(--spacing) * 0.25);
        min-height: 220px;
        overflow: hidden;
    }

    .chat-thread {
        display: flex;
        flex: 1 1 auto;
        flex-direction: column;
        gap: calc(var(--spacing) * 0.75);
        min-height: 0;
        overflow: auto;
        padding-right: 2px;
    }

    .chat-message {
        max-width: 92%;
        min-width: 0;
    }

    .chat-message span {
        color: var(--text-secondary);
        display: block;
        font-size: 10px;
        line-height: 1.2;
        margin-bottom: 3px;
    }

    .chat-message p {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        line-height: 1.35;
        margin: 0;
        overflow-wrap: anywhere;
        padding: 7px;
    }

    .user-message {
        align-self: flex-end;
    }

    .user-message p {
        background: color-mix(in srgb, var(--blue) 8%, var(--bg));
        border-color: color-mix(in srgb, var(--blue) 55%, var(--border));
    }

    .assistant-message {
        align-self: flex-start;
    }

    .assistant-message p {
        background: var(--bg);
    }

    .activity-log {
        border-top: 1px solid var(--border);
        color: var(--text-secondary);
        font-size: 10px;
        padding-top: calc(var(--spacing) * 0.5);
    }

    .activity-log summary {
        cursor: pointer;
    }

    .activity-log ol {
        display: grid;
        gap: 3px;
        list-style: none;
        margin: 6px 0 0;
        padding: 0;
    }

    .activity-log li {
        display: grid;
        gap: 2px;
        grid-template-columns: 72px minmax(0, 1fr);
    }

    .activity-log li.error {
        color: var(--red);
    }

    .activity-log strong {
        color: var(--text);
        font-weight: 600;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .activity-log span {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .proposal-topline {
        font-size: 11px;
        line-height: 1.2;
    }

    .proposal-topline strong {
        color: var(--orange);
        font-size: 11px;
        font-weight: 700;
    }

    .proposal-card h3 {
        font-size: 16px;
        line-height: 1.25;
        margin: 0;
    }

    .proposal-headline span {
        color: var(--text-secondary);
        font-size: 11px;
    }

    .proposal-meta {
        color: var(--text-secondary);
        flex-wrap: wrap;
        font-size: 10px;
        line-height: 1.3;
    }

    .proposal-meta.stale {
        color: var(--orange);
    }

    .proposal-meta strong {
        color: var(--orange);
        font-size: 10px;
        font-weight: 700;
    }

    .proposal-switcher {
        display: grid;
        gap: 4px;
    }

    .proposal-switcher span {
        color: var(--text-secondary);
        font-size: 10px;
    }

    .featured-candidate {
        background: transparent;
        border: 0;
        border-radius: 0;
        border-top: 1px solid color-mix(in srgb, var(--green) 36%, var(--border));
        grid-template-columns: 86px 1fr;
        padding: calc(var(--spacing) * 0.75) 0 0;
    }

    .candidate-preview {
        align-items: center;
        aspect-ratio: 4 / 3;
        background: var(--border);
        border: 1px solid color-mix(in srgb, var(--green) 36%, var(--border));
        border-radius: var(--radius);
        display: flex;
        justify-content: center;
        min-width: 0;
        overflow: hidden;
    }

    .candidate-preview img {
        height: 100%;
        object-fit: cover;
        width: 100%;
    }

    .candidate-preview span {
        color: var(--text-secondary);
        font-size: 11px;
    }

    .candidate-copy {
        min-width: 0;
    }

    .candidate-label,
    .candidate-id {
        color: var(--text-secondary);
        font-size: 10px;
        line-height: 1.2;
    }

    .candidate-copy strong {
        display: block;
        font-size: 13px;
        line-height: 1.25;
        margin-top: 2px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .candidate-copy p {
        color: var(--text-secondary);
        font-size: 11px;
        line-height: 1.3;
        margin: 5px 0 0;
    }

    .thinking-line {
        align-items: center;
        color: var(--text-secondary);
        display: flex;
        gap: calc(var(--spacing) * 0.75);
    }

    .thinking-dot {
        animation: pulse 1.4s ease-in-out infinite;
        background: var(--blue);
        border-radius: 50%;
        display: inline-block;
        height: 6px;
        width: 6px;
    }

    @keyframes pulse {
        0%, 100% {
            opacity: 0.35;
        }

        50% {
            opacity: 1;
        }
    }

    .link-button {
        background: transparent;
        border: 0;
        color: var(--blue);
        font-size: 11px;
        padding: 0;
    }

    .profile-control {
        display: grid;
        gap: 4px;
    }

    .profile-summary {
        line-height: 1.35;
        margin: 0;
    }

    .preset-editor {
        border-top: 1px solid var(--border);
        margin-top: calc(var(--spacing) * 0.25);
        padding-top: calc(var(--spacing) * 0.75);
    }

    .editor-actions button {
        min-width: 76px;
    }

    .empty h3 {
        font-size: 13px;
        line-height: 1.25;
        margin: 6px 0 4px;
    }

    .empty p,
    .empty-note {
        line-height: 1.35;
        margin: 0;
    }

    .candidate {
        display: grid;
        grid-template-columns: 40px 1fr;
        gap: calc(var(--spacing) * 0.75);
    }

    @media (max-width: 420px) {
        .agent-dock.floating {
            left: 8px;
            right: 8px;
            width: auto;
        }
    }
</style>
