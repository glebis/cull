<script lang="ts">
    import { parseNlQuery, evaluateSmartCollection, createSmartCollection, listSmartCollections } from '$lib/api';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { images, smartCollections, activeSmartCollection, activeFolder, activeCollection, searchOpen, viewMode, navigateTo, navigateBack } from '$lib/stores';
    import type { FilterNode } from '$lib/api';
    import RuleBuilder from './RuleBuilder.svelte';
    import { onMount, onDestroy, tick } from 'svelte';

    let query = $state('');
    let parsedFilter: FilterNode | null = $state(null);
    let matchCount = $state(0);
    let showRules = $state(false);
    let applied = $state(false);
    let isCollapsed = $state(false);
    let isDirtyFromManualEdit = $state(false);
    let isApplying = $state(false);
    let isListening = $state(false);
    let applyRequestId = $state(0);
    let dictationLocale = $state('en-US');

    let saving = $state(false);
    let collectionName = $state('');
    let savedMessage = $state('');
    let savedTimeout: ReturnType<typeof setTimeout> | null = null;

    let silenceTimer: ReturnType<typeof setTimeout> | null = null;
    let unlisteners: Array<() => void> = [];

    let inputEl: HTMLInputElement | undefined = $state();
    let barEl: HTMLDivElement | undefined = $state();

    let applyDebounceTimer: ReturnType<typeof setTimeout> | null = null;

    // Open when searchOpen store becomes true
    $effect(() => {
        if ($searchOpen) {
            tick().then(() => inputEl?.focus());
        }
    });

    onMount(async () => {
        const grid = document.querySelector('.grid-container');
        if (grid) {
            const onScroll = () => {
                isCollapsed = grid.scrollTop > 50 && (showRules || applied);
            };
            grid.addEventListener('scroll', onScroll, { passive: true });
        }

        const unlistenResult = await listen<{ text: string; is_final: boolean }>('dictation-result', (event) => {
            query = event.payload.text;
            if (silenceTimer) clearTimeout(silenceTimer);
            if (event.payload.is_final) {
                silenceTimer = setTimeout(() => {
                    stopVoice();
                    if (query.trim()) handleParse();
                }, 1500);
            }
        });
        unlisteners.push(unlistenResult);

        const unlistenStarted = await listen('dictation-started', () => {
            isListening = true;
        });
        unlisteners.push(unlistenStarted);

        const unlistenError = await listen<{ message: string }>('dictation-error', (event) => {
            console.error('Dictation error:', event.payload.message);
            isListening = false;
        });
        unlisteners.push(unlistenError);
    });

    onDestroy(() => {
        unlisteners.forEach(fn => fn());
        if (silenceTimer) clearTimeout(silenceTimer);
    });

    function generateName(q: string): string {
        if (isDirtyFromManualEdit && parsedFilter) {
            return chipSummary(parsedFilter);
        }
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

    // Produce a short human-readable summary from a FilterNode for dirty-edited names
    function chipSummary(node: FilterNode): string {
        if (node.type === 'group') {
            const parts = node.children.map(chipSummary).join(` ${node.op} `);
            return parts.length > 40 ? parts.slice(0, 37) + '…' : parts;
        }
        if (node.type === 'not') {
            return `NOT ${chipSummary(node.child)}`;
        }
        return `${node.field} ${node.op} ${node.value}`;
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
        isDirtyFromManualEdit = false;
        await handleApply();
    }

    async function handleApply() {
        if (!parsedFilter) return;
        const filterJson = JSON.stringify(parsedFilter);
        const results = await evaluateSmartCollection(filterJson);
        $images = results;
        matchCount = results.length;
        applied = true;
        if ($viewMode !== 'grid') {
            navigateTo('grid');
        }
    }

    async function debouncedApply() {
        if (applyDebounceTimer) clearTimeout(applyDebounceTimer);
        applyDebounceTimer = setTimeout(async () => {
            const reqId = ++applyRequestId;
            isApplying = true;
            try {
                const filterJson = JSON.stringify(parsedFilter);
                const results = await evaluateSmartCollection(filterJson);
                if (reqId === applyRequestId) {
                    $images = results;
                    matchCount = results.length;
                    applied = true;
                }
            } finally {
                if (reqId === applyRequestId) isApplying = false;
            }
        }, 300);
    }

    function handleFilterChange(next: FilterNode) {
        parsedFilter = next;
        isDirtyFromManualEdit = true;
        debouncedApply();
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter') {
            if (saving) {
                handleSaveConfirm();
            } else if (applied) {
                handleDismiss();
            } else if (showRules && parsedFilter) {
                handleApply();
            } else {
                handleParse();
            }
        } else if (e.key === 'Escape') {
            handleClose();
        }
    }

    function handleDismiss() {
        stopVoice();
        query = '';
        parsedFilter = null;
        showRules = false;
        applied = false;
        saving = false;
        savedMessage = '';
        isDirtyFromManualEdit = false;
        isCollapsed = false;
        $searchOpen = false;
        navigateBack();
    }

    function openSearch() {
        $searchOpen = true;
        tick().then(() => inputEl?.focus());
    }

    function handleClose() {
        stopVoice();
        query = '';
        parsedFilter = null;
        showRules = false;
        applied = false;
        saving = false;
        savedMessage = '';
        isDirtyFromManualEdit = false;
        isCollapsed = false;
        $searchOpen = false;
    }

    function handleClear() {
        query = '';
        parsedFilter = null;
        showRules = false;
        applied = false;
        saving = false;
        savedMessage = '';
        isDirtyFromManualEdit = false;
    }

    function expandFromCollapse() {
        isCollapsed = false;
        const grid = document.querySelector('.grid-container');
        if (grid) grid.scrollTop = 0;
    }

    function handleSaveStart() {
        collectionName = generateName(query);
        if (isDirtyFromManualEdit) {
            collectionName += ' (edited)';
        }
        saving = true;
    }

    function handleSaveCancel() {
        saving = false;
    }

    async function handleSaveConfirm() {
        if (!parsedFilter || !collectionName.trim()) return;
        const filterJson = JSON.stringify(parsedFilter);
        const nlQuery = isDirtyFromManualEdit ? query + ' (edited)' : query;
        try {
            await createSmartCollection(collectionName.trim(), filterJson, nlQuery);
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
            isDirtyFromManualEdit = false;
            $searchOpen = false;

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

    function toggleVoice() {
        if (isListening) {
            stopVoice();
        } else {
            startVoice();
        }
    }

    function toggleLocale() {
        dictationLocale = dictationLocale === 'en-US' ? 'ru-RU' : 'en-US';
    }

    async function startVoice() {
        try {
            await invoke('start_dictation', { locale: dictationLocale });
        } catch (e) {
            console.error('Failed to start dictation:', e);
            isListening = false;
        }
    }

    async function stopVoice() {
        if (silenceTimer) clearTimeout(silenceTimer);
        try {
            await invoke('stop_dictation');
        } catch (_) {}
        isListening = false;
    }
</script>

{#if savedMessage}
    <div class="saved-toast">
        <span class="saved-icon">&#10003;</span>
        {savedMessage}
    </div>
{:else if isCollapsed && (showRules || applied)}
    <div class="collapsed-pill" onclick={expandFromCollapse} role="button" tabindex="0" onkeydown={e => e.key === 'Enter' && expandFromCollapse()}>
        <span class="pill-icon">🔍</span>
        <span class="pill-query">{query || 'Filter active'}</span>
        <span class="pill-count">{matchCount}</span>
        <button class="pill-close" onclick={(e) => { e.stopPropagation(); handleClose(); }}>×</button>
    </div>
{:else if $searchOpen}
    <div class="command-bar-wrapper" bind:this={barEl}>
        <div class="command-bar">
            <span class="command-icon">/</span>
            <input
                bind:this={inputEl}
                bind:value={query}
                onkeydown={handleKeydown}
                placeholder="landscape midjourney 4 stars or more..."
                class="command-input"
                role="searchbox"
                aria-label="Search images"
            />
            <button
                class="locale-btn"
                onclick={toggleLocale}
                aria-label="Switch dictation language"
                title={dictationLocale === 'en-US' ? 'English' : 'Russian'}
            >
                {dictationLocale === 'en-US' ? 'EN' : 'RU'}
            </button>
            <button
                class="mic-btn"
                class:listening={isListening}
                onclick={toggleVoice}
                aria-label="Toggle voice input"
                aria-pressed={isListening}
            >
                {isListening ? '⏸' : '🎤'}
            </button>
            <span class="esc-badge">esc</span>
            {#if query}
                <button class="clear-btn" onclick={handleClear}>×</button>
            {/if}
            <button class="close-btn" onclick={handleClose}>×</button>
        </div>

        {#if showRules && parsedFilter}
            <div class="parsed-rules" class:dimmed={saving}>
                <div class="rules-header">
                    <span class="parsed-label">Parsed as:</span>
                    <div class="rules-actions">
                        {#if applied}
                            <span class="match-count">{matchCount} images</span>
                        {/if}
                        <button class="apply-btn" onclick={handleApply} disabled={isApplying}>
                            {isApplying ? '...' : applied ? 'Refresh' : 'Apply'}
                        </button>
                        {#if applied && !saving}
                            <button class="save-btn" onclick={handleSaveStart}>Save Collection</button>
                        {/if}
                    </div>
                </div>
                <RuleBuilder filter={parsedFilter} onchange={handleFilterChange} />
            </div>
        {/if}

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
    </div>
{:else}
    <div class="search-hint" onclick={openSearch} role="button" tabindex="0" onkeydown={e => e.key === 'Enter' && openSearch()}>
        <span class="hint-slash">/</span>
        <span class="hint-text">to search</span>
    </div>
{/if}

<style>
    .search-hint {
        display: flex;
        align-items: center;
        gap: 6px;
        height: 36px;
        padding: 0 16px;
        cursor: pointer;
        border-radius: 8px;
        transition: background 120ms;
        user-select: none;
    }

    .search-hint:hover {
        background: rgba(255,255,255,0.04);
    }

    .hint-slash {
        color: var(--text-secondary);
        font-size: 14px;
        font-weight: 600;
        opacity: 0.5;
    }

    .hint-text {
        color: var(--text-secondary);
        font-size: 13px;
        opacity: 0.4;
    }

    .collapsed-pill {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        height: 28px;
        padding: 0 10px;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 999px;
        cursor: pointer;
        max-width: 320px;
        animation: pill-in 150ms ease-out;
    }

    .pill-icon {
        font-size: 12px;
        flex-shrink: 0;
    }

    .pill-query {
        font-size: 12px;
        color: var(--text);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        max-width: 180px;
    }

    .pill-count {
        font-size: 11px;
        color: var(--text-secondary);
        background: rgba(255,255,255,0.06);
        padding: 1px 6px;
        border-radius: 999px;
        flex-shrink: 0;
    }

    .pill-close {
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 14px;
        line-height: 1;
        padding: 0;
        flex-shrink: 0;
        transition: color 120ms;
    }

    .pill-close:hover {
        color: var(--red);
    }

    @keyframes pill-in {
        from { opacity: 0; transform: scale(0.95); }
        to { opacity: 1; transform: scale(1); }
    }

    .command-bar-wrapper {
        display: flex;
        flex-direction: column;
        animation: wrapper-in 200ms ease-out;
    }

    @keyframes wrapper-in {
        from { opacity: 0; transform: translateY(-4px); }
        to { opacity: 1; transform: translateY(0); }
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

    .locale-btn {
        height: 22px;
        padding: 0 6px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: rgba(122,162,247,0.08);
        border: 1px solid rgba(122,162,247,0.2);
        border-radius: 4px;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 10px;
        font-weight: 600;
        letter-spacing: 0.04em;
        flex-shrink: 0;
        transition: background 120ms, color 120ms;
    }

    .locale-btn:hover {
        background: rgba(122,162,247,0.15);
        color: var(--blue);
    }

    .mic-btn {
        width: 28px;
        height: 28px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: rgba(122,162,247,0.1);
        border: 1px solid rgba(122,162,247,0.25);
        border-radius: 50%;
        color: var(--blue);
        cursor: pointer;
        font-size: 13px;
        flex-shrink: 0;
        transition: background 120ms, border-color 120ms;
    }

    .mic-btn:hover {
        background: rgba(122,162,247,0.2);
    }

    .mic-btn.listening {
        animation: mic-pulse 1s ease-in-out infinite;
        border-color: var(--blue);
    }

    @keyframes mic-pulse {
        0%, 100% { box-shadow: 0 0 0 0 rgba(122,162,247,0.4); }
        50% { box-shadow: 0 0 0 8px rgba(122,162,247,0); }
    }

    .esc-badge {
        font-size: 10px;
        color: var(--text-secondary);
        background: rgba(255,255,255,0.06);
        border: 1px solid var(--border);
        border-radius: 4px;
        padding: 2px 5px;
        flex-shrink: 0;
        opacity: 0.6;
        cursor: default;
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
        flex-shrink: 0;
        transition: color 120ms, border-color 120ms, background 120ms;
    }

    .clear-btn:hover {
        color: var(--red);
        border-color: rgba(247, 118, 142, 0.4);
        background: rgba(247, 118, 142, 0.08);
    }

    .close-btn {
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
        flex-shrink: 0;
        transition: color 120ms, border-color 120ms, background 120ms;
    }

    .close-btn:hover {
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

    .apply-btn:disabled {
        opacity: 0.5;
        cursor: default;
    }

    .apply-btn:not(:disabled):hover {
        background: linear-gradient(180deg, rgba(122,162,247,0.3), rgba(122,162,247,0.15));
        color: #8fb3ff;
    }

    .apply-btn:not(:disabled):active {
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
