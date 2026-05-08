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
    .command-bar-wrapper { display: flex; flex-direction: column; gap: 0; }
    .command-bar { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border: 2px solid var(--accent, #4a9eed); border-radius: 8px; background: var(--bg, #fff); }
    .command-icon { color: var(--accent, #4a9eed); font-size: 18px; font-weight: bold; }
    .command-input { flex: 1; border: none; outline: none; font-size: 16px; background: transparent; color: inherit; }
    .clear-btn { background: none; border: none; color: #999; cursor: pointer; font-size: 18px; padding: 0 4px; }
    .parsed-rules { padding: 12px; background: var(--surface, #fafafa); border: 1px solid var(--border, #e0e0e0); border-radius: 0 0 8px 8px; border-top: none; }
    .rules-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
    .parsed-label { color: var(--muted, #757575); font-size: 14px; }
    .rules-actions { display: flex; align-items: center; gap: 8px; }
    .match-count { color: var(--muted, #757575); font-size: 14px; }
    .apply-btn { padding: 4px 12px; border-radius: 6px; border: 1px solid var(--accent, #4a9eed); background: var(--accent, #4a9eed); color: #fff; cursor: pointer; font-size: 13px; }
</style>
