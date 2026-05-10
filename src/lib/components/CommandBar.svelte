<script lang="ts">
    import { parseNlQuery, evaluateSmartCollection, createSmartCollection, listSmartCollections } from '$lib/api';
    import { images, smartCollections, activeSmartCollection, activeFolder, activeCollection } from '$lib/stores';
    import type { FilterNode } from '$lib/api';
    import RuleBuilder from './RuleBuilder.svelte';

    let query = $state('');
    let parsedFilter: FilterNode | null = $state(null);
    let matchCount = $state(0);
    let showRules = $state(false);
    let applied = $state(false);

    let saving = $state(false);
    let collectionName = $state('');
    let savedMessage = $state('');
    let savedTimeout: ReturnType<typeof setTimeout> | null = null;

    function generateName(q: string): string {
        return q.trim()
            .split(/\s+/)
            .map(w => w.charAt(0).toUpperCase() + w.slice(1))
            .join(' ')
            .replace(/\bMidjourney\b/i, 'MJ')
            .replace(/\bStable Diffusion\b/i, 'SD')
            .replace(/\bOr More\b/i, '+')
            .replace(/\bAnd Above\b/i, '+')
            .replace(/\bStars?\b/i, 'Stars');
    }

    async function handleParse() {
        if (!query.trim()) {
            parsedFilter = null;
            showRules = false;
            applied = false;
            saving = false;
            return;
        }
        const filterJson = await parseNlQuery(query);
        parsedFilter = JSON.parse(filterJson);
        showRules = true;
        applied = false;
        saving = false;
        // Auto-apply: parse + apply in one step
        await handleApply();
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
        if (e.key === 'Enter') {
            if (saving) {
                handleSaveConfirm();
            } else if (showRules && parsedFilter) {
                handleApply();
            } else {
                handleParse();
            }
        }
    }

    function handleClear() {
        query = '';
        parsedFilter = null;
        showRules = false;
        applied = false;
        saving = false;
        savedMessage = '';
    }

    function handleSaveStart() {
        collectionName = generateName(query);
        saving = true;
    }

    function handleSaveCancel() {
        saving = false;
    }

    async function handleSaveConfirm() {
        if (!parsedFilter || !collectionName.trim()) return;
        const filterJson = JSON.stringify(parsedFilter);
        try {
            await createSmartCollection(collectionName.trim(), filterJson, query);
            const updated = await listSmartCollections();
            $smartCollections = updated;

            const saved = updated.find(sc => sc.name === collectionName.trim());
            if (saved) {
                $activeSmartCollection = saved;
                $activeFolder = null;
                $activeCollection = null;
            }

            savedMessage = `Saved as "${collectionName.trim()}" — ${matchCount} images`;
            saving = false;
            showRules = false;
            query = '';
            parsedFilter = null;
            applied = false;

            if (savedTimeout) clearTimeout(savedTimeout);
            savedTimeout = setTimeout(() => { savedMessage = ''; }, 3000);
        } catch (e) {
            console.error('Failed to save collection:', e);
        }
    }

    function handleNameKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter') handleSaveConfirm();
        if (e.key === 'Escape') handleSaveCancel();
    }
</script>

<div class="command-bar-wrapper">
    {#if savedMessage}
        <div class="saved-toast">
            <span class="saved-icon">&#10003;</span>
            {savedMessage}
        </div>
    {/if}

    {#if !savedMessage}
        <div class="command-bar">
            <span class="command-icon">/</span>
            <input
                type="text"
                bind:value={query}
                onkeydown={handleKeydown}
                placeholder="landscape midjourney 4 stars or more..."
                class="command-input"
                aria-label="Filter images by natural language query"
            />
            {#if query}
                <button class="clear-btn" onclick={handleClear}>&times;</button>
            {/if}
        </div>

        {#if saving}
            <div class="name-bar">
                <span class="name-label">Name</span>
                <input
                    type="text"
                    bind:value={collectionName}
                    onkeydown={handleNameKeydown}
                    class="name-input"
                    autofocus
                />
                <button class="save-confirm-btn" onclick={handleSaveConfirm}>Save</button>
                <button class="save-cancel-btn" onclick={handleSaveCancel}>Cancel</button>
            </div>
        {/if}

        {#if showRules && parsedFilter}
            <div class="parsed-rules" class:dimmed={saving}>
                <div class="rules-header">
                    <span class="parsed-label">Parsed as:</span>
                    <div class="rules-actions">
                        {#if applied}
                            <span class="match-count">{matchCount} images</span>
                        {/if}
                        <button class="apply-btn" onclick={handleApply}>
                            {applied ? 'Refresh' : 'Apply'}
                        </button>
                        {#if applied && !saving}
                            <button class="save-btn" onclick={handleSaveStart}>
                                Save Collection
                            </button>
                        {/if}
                    </div>
                </div>
                <RuleBuilder filter={parsedFilter} />
            </div>
        {/if}
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

    .save-btn {
        height: 32px;
        padding: 0 16px;
        border-radius: 6px;
        border: 1px solid var(--green);
        background: linear-gradient(180deg, rgba(158,206,106,0.2), rgba(158,206,106,0.1));
        color: var(--green);
        cursor: pointer;
        font-size: 13px;
        font-weight: 500;
        transition: background 120ms, color 120ms, transform 80ms;
    }

    .save-btn:hover {
        background: linear-gradient(180deg, rgba(158,206,106,0.3), rgba(158,206,106,0.15));
    }

    .save-btn:active {
        transform: translateY(1px);
    }

    .name-bar {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 10px 14px;
        background: linear-gradient(180deg, rgba(158,206,106,0.06), rgba(158,206,106,0.02)), var(--surface);
        border: 1px solid rgba(158,206,106,0.3);
        border-top: none;
        border-radius: 0 0 8px 8px;
    }

    .name-label {
        font-size: 11px;
        color: var(--green);
        text-transform: uppercase;
        letter-spacing: 0.04em;
        font-weight: 600;
        white-space: nowrap;
    }

    .name-input {
        flex: 1;
        background: none;
        border: none;
        color: var(--text);
        font-family: var(--font);
        font-size: 14px;
        font-weight: 500;
        outline: none;
    }

    .save-confirm-btn {
        height: 28px;
        padding: 0 14px;
        border-radius: 6px;
        border: 1px solid var(--green);
        background: linear-gradient(180deg, rgba(158,206,106,0.2), rgba(158,206,106,0.1));
        color: var(--green);
        cursor: pointer;
        font-size: 12px;
        font-weight: 500;
        transition: background 120ms, transform 80ms;
    }

    .save-confirm-btn:hover {
        background: linear-gradient(180deg, rgba(158,206,106,0.3), rgba(158,206,106,0.15));
    }

    .save-cancel-btn {
        height: 28px;
        padding: 0 12px;
        border-radius: 6px;
        border: 1px solid var(--border);
        background: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 12px;
        transition: border-color 120ms, color 120ms;
    }

    .save-cancel-btn:hover {
        border-color: rgba(255,255,255,0.12);
        color: var(--text);
    }

    .dimmed {
        opacity: 0.5;
        pointer-events: none;
    }

    .saved-toast {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 12px 16px;
        background: linear-gradient(180deg, rgba(158,206,106,0.1), rgba(158,206,106,0.04)), var(--surface);
        border: 1px solid rgba(158,206,106,0.3);
        border-radius: 8px;
        color: var(--green);
        font-size: 13px;
        font-weight: 500;
        animation: toast-in 200ms ease-out;
    }

    .saved-icon {
        font-size: 16px;
    }

    @keyframes toast-in {
        from { opacity: 0; transform: translateY(-4px); }
        to { opacity: 1; transform: translateY(0); }
    }
</style>
