<script lang="ts">
    // Persistent Selection Mode chrome: run name, Source/Shortlist scope
    // control, counts, and the lifecycle actions. Visible in every image view
    // while a run is active.
    import { selectionRun, selectionScope, shortlistIds, selectionSourceSearch } from '$lib/stores';
    import {
        archiveSelection,
        isSelectionModeActive,
        leaveSelectionMode,
        selectionAnnouncement,
        switchSelectionModeScope,
    } from '$lib/selection-mode';
    import { selectionFinishOpen } from '$lib/stores';

    let switching = $state(false);

    const run = $derived($selectionRun);
    const shortlistCount = $derived($shortlistIds.size);
    const targetSuffix = $derived(run?.target_count !== null && run?.target_count !== undefined ? ` / target ${run.target_count}` : '');
    const targetMismatch = $derived(
        run?.target_count != null && shortlistCount !== run.target_count
    );
    const rejectedConflicts = $derived(run?.rejected_shortlist_count ?? 0);

    async function chooseScope(scope: 'source' | 'shortlist') {
        if (switching || $selectionScope === scope) return;
        switching = true;
        try {
            await switchSelectionModeScope(scope);
        } finally {
            switching = false;
        }
    }

    function openFinish() {
        if (shortlistCount === 0) return;
        selectionFinishOpen.set(true);
    }
</script>

{#if run && isSelectionModeActive()}
    <div class="selection-bar" role="toolbar" aria-label="Selection Mode">
        <span class="selection-title" data-selection-run={run.id}>
            Selection: {run.name}
        </span>

        <div class="scope-control" role="radiogroup" aria-label="Selection view scope">
            <button
                role="radio"
                aria-checked={$selectionScope === 'source'}
                class="scope-btn"
                class:active={$selectionScope === 'source'}
                onclick={() => void chooseScope('source')}
            >
                Source
            </button>
            <button
                role="radio"
                aria-checked={$selectionScope === 'shortlist'}
                class="scope-btn"
                class:active={$selectionScope === 'shortlist'}
                onclick={() => void chooseScope('shortlist')}
            >
                Shortlist
            </button>
        </div>

        <span class="counts" data-selection-counts>
            Source {run.source_count}
            <span class="sep">·</span>
            Shortlist {shortlistCount}{targetSuffix}
        </span>

        {#if $selectionSourceSearch}
            <span class="search-note">Searching “{$selectionSourceSearch}” within the source</span>
        {/if}

        {#if targetMismatch}
            <span class="target-note" role="status">
                {shortlistCount < (run.target_count ?? 0)
                    ? `${(run.target_count ?? 0) - shortlistCount} more needed to reach the target`
                    : `${shortlistCount - (run.target_count ?? 0)} over the target`}
            </span>
        {/if}
        {#if rejectedConflicts > 0}
            <span class="conflict-note" role="status">
                {rejectedConflicts} shortlisted image{rejectedConflicts === 1 ? ' is' : 's are'} also rejected
            </span>
        {/if}

        <span class="spacer"></span>

        <span class="visually-hidden" aria-live="polite">{$selectionAnnouncement ?? ''}</span>

        <button class="bar-btn" onclick={openFinish} disabled={shortlistCount === 0}
            title={shortlistCount === 0
                ? 'Add images to the shortlist before finishing — archive instead to keep an empty run'
                : 'Finish: the shortlist becomes a normal collection'}>
            Finish…
        </button>
        <button class="bar-btn" onclick={() => void archiveSelection()}
            title="Archive: keeps the shortlist and history, restorable later">
            Archive…
        </button>
        <button class="bar-btn" onclick={() => void leaveSelectionMode()}
            title="Leave: the selection stays active and resumable">
            Leave
        </button>
    </div>
{/if}

<style>
    .selection-bar {
        grid-area: selection;
        min-width: 0;
        flex-wrap: wrap;
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 6px 12px;
        border-bottom: 1px solid var(--border);
        background: var(--surface);
        font-size: 12px;
        color: var(--text);
        min-height: 34px;
    }
    .selection-title {
        font-weight: 600;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: min(30vw, 320px);
    }
    .scope-control {
        flex-shrink: 0;
        display: flex;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        overflow: hidden;
    }
    .scope-btn {
        border: none;
        background: transparent;
        color: var(--text-secondary);
        font-size: 12px;
        padding: 3px 12px;
        cursor: pointer;
    }
    .scope-btn.active {
        background: var(--blue);
        color: var(--bg);
        font-weight: 600;
    }
    .counts {
        white-space: nowrap;
        color: var(--text-secondary);
    }
    .counts .sep {
        margin: 0 4px;
    }
    .search-note,
    .target-note,
    .conflict-note {
        font-style: italic;
        font-size: 11px;
        overflow-wrap: anywhere;
    }
    .target-note {
        color: var(--orange);
    }
    .conflict-note {
        color: var(--orange);
    }
    .spacer {
        flex: 1;
    }
    .bar-btn {
        flex-shrink: 0;
        border: 1px solid var(--border);
        background: transparent;
        color: var(--text);
        font-size: 12px;
        padding: 3px 10px;
        border-radius: var(--radius);
        cursor: pointer;
        white-space: nowrap;
    }
    .bar-btn:hover:not(:disabled) {
        border-color: var(--blue);
        color: var(--blue);
    }
    .bar-btn:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }
    .visually-hidden {
        position: absolute;
        width: 1px;
        height: 1px;
        margin: -1px;
        padding: 0;
        overflow: hidden;
        clip: rect(0 0 0 0);
        white-space: nowrap;
        border: 0;
    }
</style>