<script lang="ts">
    import { parseNlQuery, evaluateSmartCollection } from '$lib/api';
    import { images } from '$lib/stores';
    import type { FilterNode } from '$lib/api';
    import RuleBuilder from './RuleBuilder.svelte';

    let query = $state('');
    let parsedFilter: FilterNode | null = $state(null);
    let matchCount = $state(0);
    let showRules = $state(false);
    let applied = $state(false);

    async function handleParse() {
        if (!query.trim()) {
            parsedFilter = null;
            showRules = false;
            applied = false;
            return;
        }
        const filterJson = await parseNlQuery(query);
        parsedFilter = JSON.parse(filterJson);
        showRules = true;
        applied = false;
    }

    async function handleApply() {
        if (!parsedFilter) return;
        const filterJson = JSON.stringify(parsedFilter);
        const results = await evaluateSmartCollection(filterJson);
        $images = results;
        matchCount = results.length;
        applied = true;
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter') handleParse();
    }

    function handleClear() {
        query = '';
        parsedFilter = null;
        showRules = false;
        applied = false;
    }
</script>

<div class="command-bar-wrapper">
    <div class="command-bar">
        <span class="command-icon">/</span>
        <input
            type="text"
            bind:value={query}
            onkeydown={handleKeydown}
            placeholder="landscape midjourney 4 stars or more..."
            class="command-input"
        />
        {#if query}
            <button class="clear-btn" onclick={handleClear}>&times;</button>
        {/if}
    </div>

    {#if showRules && parsedFilter}
        <div class="parsed-rules">
            <div class="rules-header">
                <span class="parsed-label">Parsed as:</span>
                <div class="rules-actions">
                    {#if applied}
                        <span class="match-count">{matchCount} images</span>
                    {/if}
                    <button class="apply-btn" onclick={handleApply}>
                        {applied ? 'Refresh' : 'Apply'}
                    </button>
                </div>
            </div>
            <RuleBuilder filter={parsedFilter} />
        </div>
    {/if}
</div>

<style>
    .command-bar-wrapper {
        display: flex;
        flex-direction: column;
    }

    .command-bar {
        display: flex;
        align-items: center;
        gap: 12px;
        min-height: 48px;
        padding: 0 16px;
        background: linear-gradient(180deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01)), var(--surface);
        border: 1px solid var(--border);
        border-radius: 8px;
        transition: border-color 120ms ease, box-shadow 120ms ease;
    }

    .command-bar:focus-within {
        border-color: var(--blue);
        box-shadow: 0 0 0 1px var(--blue), 0 0 0 4px rgba(122, 162, 247, 0.12);
    }

    .command-icon {
        color: var(--text-secondary);
        font-size: 16px;
        font-weight: 600;
        opacity: 0.8;
        flex-shrink: 0;
    }

    .command-input {
        flex: 1;
        min-width: 0;
        height: 46px;
        border: 0;
        outline: none;
        background: transparent;
        color: var(--text);
        font-family: var(--font);
        font-size: 14px;
        letter-spacing: -0.01em;
    }

    .command-input::placeholder {
        color: var(--text-secondary);
        opacity: 0.6;
    }

    .clear-btn {
        width: 28px;
        height: 28px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: none;
        border: 1px solid var(--border);
        border-radius: 6px;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 14px;
        transition: color 120ms, border-color 120ms, background 120ms;
    }

    .clear-btn:hover {
        color: var(--red);
        border-color: rgba(247, 118, 142, 0.4);
        background: rgba(247, 118, 142, 0.08);
    }

    .parsed-rules {
        padding: 16px;
        background: linear-gradient(180deg, rgba(255,255,255,0.025), rgba(255,255,255,0.008)), var(--surface);
        border: 1px solid var(--border);
        border-top: none;
        border-radius: 0 0 8px 8px;
    }

    .rules-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 12px;
    }

    .parsed-label {
        color: var(--text-secondary);
        font-size: 12px;
        font-weight: 500;
        text-transform: uppercase;
        letter-spacing: 0.04em;
    }

    .rules-actions {
        display: flex;
        align-items: center;
        gap: 12px;
    }

    .match-count {
        color: var(--text-secondary);
        font-family: var(--font);
        font-size: 12px;
    }

    .apply-btn {
        height: 32px;
        padding: 0 16px;
        border-radius: 6px;
        border: 1px solid var(--blue);
        background: linear-gradient(180deg, rgba(122,162,247,0.2), rgba(122,162,247,0.1));
        color: var(--blue);
        cursor: pointer;
        font-size: 13px;
        font-weight: 500;
        transition: background 120ms, color 120ms, transform 80ms;
    }

    .apply-btn:hover {
        background: linear-gradient(180deg, rgba(122,162,247,0.3), rgba(122,162,247,0.15));
        color: #8fb3ff;
    }

    .apply-btn:active {
        transform: translateY(1px);
    }
</style>
