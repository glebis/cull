<script lang="ts">
    import { parseNlQuery, countSmartCollection, createSmartCollection, listSmartCollections, startDictation, stopDictation } from '$lib/api';
    import { listen } from '@tauri-apps/api/event';
    import { smartCollections, activeSmartCollection, activeFolder, activeCollection, activeDetectedClass, searchOpen, viewMode, navigateTo, navigateBack, voiceDictationEnabled } from '$lib/stores';
    import type { FilterNode } from '$lib/api';
    import { buildSearchPresetLists, type SearchPreset, type SearchPresetKind } from '$lib/search-presets';
    import RuleBuilder from './RuleBuilder.svelte';
    import { onMount, onDestroy, tick } from 'svelte';
    import { loadImagesForCurrentScope } from '$lib/image-loading';

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
    let searchPresetRequestId = 0;
    let dictationLocale = $state('en-US');

    let saving = $state(false);
    let collectionName = $state('');
    let savedMessage = $state('');
    let savedTimeout: ReturnType<typeof setTimeout> | null = null;
    let savedSearchPresets: SearchPreset[] = $state([]);
    let autoSearchPresets: SearchPreset[] = $state([]);
    let isLoadingSearchPresets = $state(false);
    let activeAppliedPresetKind: SearchPresetKind | null = $state(null);

    let silenceTimer: ReturnType<typeof setTimeout> | null = null;
    let unlisteners: Array<() => void> = [];

    let inputEl: HTMLInputElement | undefined = $state();
    let nameInputEl: HTMLInputElement | undefined = $state();
    let barEl: HTMLDivElement | undefined = $state();

    let applyDebounceTimer: ReturnType<typeof setTimeout> | null = null;

    async function rebuildSearchPresetLists(collections = $smartCollections) {
        const lists = await buildSearchPresetLists(collections, countSmartCollection);
        savedSearchPresets = lists.saved;
        autoSearchPresets = lists.auto;
    }

    async function loadSearchPresets() {
        const reqId = ++searchPresetRequestId;
        isLoadingSearchPresets = true;
        try {
            const updated = await listSmartCollections();
            if (reqId !== searchPresetRequestId) return;
            $smartCollections = updated;
            await rebuildSearchPresetLists(updated);
        } catch (e) {
            console.error('Failed to load search presets:', e);
        } finally {
            if (reqId === searchPresetRequestId) isLoadingSearchPresets = false;
        }
    }

    function activateAdHocFilter(filterJson: string, count: number | null) {
        $activeSmartCollection = {
            id: '__adhoc__',
            name: query.trim() || 'Search',
            description: null,
            collection_type: 'smart',
            filter_json: filterJson,
            nl_query: query.trim() || null,
            is_preset: false,
            sort_order: 0,
            created_at: new Date().toISOString(),
            image_count: count,
        };
        $activeFolder = null;
        $activeCollection = null;
        $activeDetectedClass = null;
    }

    async function applyFilter(filterJson: string, reqId: number) {
        activateAdHocFilter(filterJson, null);
        const [count] = await Promise.all([
            countSmartCollection(filterJson),
            loadImagesForCurrentScope({ force: true }),
        ]);
        if (reqId !== applyRequestId) return;
        matchCount = count;
        activateAdHocFilter(filterJson, count);
        applied = true;
    }

    // Open when searchOpen store becomes true
    $effect(() => {
        if ($searchOpen) {
            loadSearchPresets();
            tick().then(() => inputEl?.focus());
        }
    });

    $effect(() => {
        if (saving) {
            tick().then(() => nameInputEl?.focus());
        }
    });

    onMount(async () => {
        const grid = document.querySelector('.grid-container');
        if (grid) {
            const onScroll = () => {
                isCollapsed = grid.scrollTop > 50 && (showRules || applied);
            };
            grid.addEventListener('scroll', onScroll, { passive: true });
            unlisteners.push(() => grid.removeEventListener('scroll', onScroll));
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

        const unlistenImagesChanged = await listen<void>('images:changed', () => {
            loadSearchPresets();
        });
        unlisteners.push(unlistenImagesChanged);

        const onSessionEventsRefresh = () => loadSearchPresets();
        window.addEventListener('session-events-refresh', onSessionEventsRefresh);
        unlisteners.push(() => window.removeEventListener('session-events-refresh', onSessionEventsRefresh));

        await loadSearchPresets();
    });

    onDestroy(() => {
        unlisteners.forEach(fn => fn());
        if (silenceTimer) clearTimeout(silenceTimer);
        if (savedTimeout) clearTimeout(savedTimeout);
        if (applyDebounceTimer) clearTimeout(applyDebounceTimer);
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
            activeAppliedPresetKind = null;
            return;
        }
        const filterJson = await parseNlQuery(query);
        parsedFilter = JSON.parse(filterJson);
        showRules = true;
        applied = false;
        saving = false;
        isDirtyFromManualEdit = false;
        activeAppliedPresetKind = null;
        await handleApply();
    }

    async function handleApply() {
        if (!parsedFilter) return;
        const reqId = ++applyRequestId;
        isApplying = true;
        const filterJson = JSON.stringify(parsedFilter);
        try {
            await applyFilter(filterJson, reqId);
            if ($viewMode !== 'grid') {
                navigateTo('grid');
            }
        } finally {
            if (reqId === applyRequestId) isApplying = false;
        }
    }

    async function debouncedApply() {
        if (applyDebounceTimer) clearTimeout(applyDebounceTimer);
        applyDebounceTimer = setTimeout(async () => {
            const reqId = ++applyRequestId;
            isApplying = true;
            try {
                const filterJson = JSON.stringify(parsedFilter);
                await applyFilter(filterJson, reqId);
            } finally {
                if (reqId === applyRequestId) isApplying = false;
            }
        }, 300);
    }

    function handleFilterChange(next: FilterNode) {
        parsedFilter = next;
        isDirtyFromManualEdit = true;
        activeAppliedPresetKind = null;
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
        activeAppliedPresetKind = null;
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
        activeAppliedPresetKind = null;
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
        activeAppliedPresetKind = null;
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
            await rebuildSearchPresetLists(updated);

            const saved = updated.find(sc => sc.name === collectionName.trim());
            if (saved) {
                $activeSmartCollection = saved;
                $activeFolder = null;
                $activeCollection = null;
                $activeDetectedClass = null;
                await loadImagesForCurrentScope({ resetFocus: false });
            }

            savedMessage = `Saved as "${collectionName.trim()}" — ${matchCount} images`;
            saving = false;
            showRules = false;
            query = '';
            parsedFilter = null;
            applied = false;
            isDirtyFromManualEdit = false;
            activeAppliedPresetKind = null;
            $searchOpen = false;

            if (savedTimeout) clearTimeout(savedTimeout);
            savedTimeout = setTimeout(() => { savedMessage = ''; }, 3000);
        } catch (e) {
            console.error('Failed to save collection:', e);
        }
    }

    async function handlePresetSelect(preset: SearchPreset) {
        query = preset.query;
        parsedFilter = JSON.parse(preset.filterJson);
        showRules = true;
        saving = false;
        isDirtyFromManualEdit = false;
        activeAppliedPresetKind = preset.kind;

        if (preset.kind === 'saved') {
            const saved = $smartCollections.find(sc => sc.id === preset.id);
            if (saved) {
                $activeSmartCollection = saved;
                $activeFolder = null;
                $activeCollection = null;
                $activeDetectedClass = null;
                matchCount = preset.imageCount;
                applied = true;
                if ($viewMode !== 'grid') {
                    navigateTo('grid');
                }
                await loadImagesForCurrentScope({ force: true });
                return;
            }
        }

        applied = false;
        await handleApply();
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
            await startDictation(dictationLocale);
        } catch (e) {
            console.error('Failed to start dictation:', e);
            isListening = false;
        }
    }

    async function stopVoice() {
        if (silenceTimer) clearTimeout(silenceTimer);
        try {
            await stopDictation();
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
        <button class="pill-close" onclick={(e) => { e.stopPropagation(); handleClose(); }} aria-label="Remove filter" title="Remove filter">×</button>
    </div>
{:else if $searchOpen}
    <div class="command-bar-wrapper" bind:this={barEl}>
        <div class="command-bar">
            <span class="command-icon">/</span>
            <input
                bind:this={inputEl}
                bind:value={query}
                onkeydown={handleKeydown}
                placeholder="Search images..."
                class="command-input"
                role="searchbox"
                aria-label="Search images"
            />
            {#if $voiceDictationEnabled}
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
            {/if}
            <span class="esc-badge">esc</span>
            {#if query}
                <button class="clear-btn" onclick={handleClear} aria-label="Clear query" title="Clear query">⌫</button>
            {/if}
            <button class="close-btn" onclick={handleClose} aria-label="Close search" title="Close search">×</button>
        </div>

        {#if !query.trim() && !showRules && !saving && (savedSearchPresets.length > 0 || autoSearchPresets.length > 0 || isLoadingSearchPresets)}
            <div class="search-presets-panel">
                {#if savedSearchPresets.length > 0}
                    <div class="preset-section">
                        <div class="preset-section-header">SAVED</div>
                        <div class="preset-list">
                            {#each savedSearchPresets as preset}
                                <button
                                    class="search-preset-btn"
                                    onclick={() => handlePresetSelect(preset)}
                                    title={`${preset.imageCount} images`}
                                >
                                    <span class="preset-name">{preset.name}</span>
                                    <span class="preset-count">{preset.imageCount}</span>
                                </button>
                            {/each}
                        </div>
                    </div>
                {/if}

                {#if autoSearchPresets.length > 0}
                    <div class="preset-section">
                        <div class="preset-section-header">SUGGESTED</div>
                        <div class="preset-list">
                            {#each autoSearchPresets as preset}
                                <button
                                    class="search-preset-btn auto"
                                    onclick={() => handlePresetSelect(preset)}
                                    title={`${preset.imageCount} images`}
                                >
                                    <span class="preset-name">{preset.name}</span>
                                    <span class="preset-count">{preset.imageCount}</span>
                                </button>
                            {/each}
                        </div>
                    </div>
                {:else if isLoadingSearchPresets}
                    <div class="preset-loading">Loading presets...</div>
                {/if}
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
                        <button class="apply-btn" onclick={handleApply} disabled={isApplying}>
                            {isApplying ? '...' : applied ? 'Refresh' : 'Apply'}
                        </button>
                        {#if applied && !saving && activeAppliedPresetKind !== 'saved'}
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
                    bind:this={nameInputEl}
                    bind:value={collectionName}
                    onkeydown={handleNameKeydown}
                    class="name-input"
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
        background: color-mix(in srgb, var(--text) 4%, transparent);
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
        background: color-mix(in srgb, var(--text) 6%, transparent);
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
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 8px;
        transition: border-color 120ms ease, box-shadow 120ms ease;
    }

    .command-bar:focus-within {
        border-color: var(--blue);
        box-shadow: 0 0 0 1px var(--blue), 0 0 0 4px color-mix(in srgb, var(--blue) 12%, transparent);
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
        letter-spacing: 0;
    }

    .command-input::placeholder {
        color: var(--text-secondary);
        opacity: 0.6;
    }

    .locale-btn {
        min-height: 32px;
        padding: 0 6px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: color-mix(in srgb, var(--blue) 8%, transparent);
        border: 1px solid color-mix(in srgb, var(--blue) 20%, transparent);
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
        background: color-mix(in srgb, var(--blue) 15%, transparent);
        color: var(--blue);
    }

    .mic-btn {
        width: 32px;
        height: 32px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: color-mix(in srgb, var(--blue) 10%, transparent);
        border: 1px solid color-mix(in srgb, var(--blue) 25%, transparent);
        border-radius: 50%;
        color: var(--blue);
        cursor: pointer;
        font-size: 13px;
        flex-shrink: 0;
        transition: background 120ms, border-color 120ms;
    }

    .mic-btn:hover {
        background: color-mix(in srgb, var(--blue) 20%, transparent);
    }

    .mic-btn.listening {
        animation: mic-pulse 1s ease-in-out infinite;
        border-color: var(--blue);
    }

    @keyframes mic-pulse {
        0%, 100% { box-shadow: 0 0 0 0 color-mix(in srgb, var(--blue) 40%, transparent); }
        50% { box-shadow: 0 0 0 8px transparent; }
    }

    .esc-badge {
        font-size: 10px;
        color: var(--text-secondary);
        background: color-mix(in srgb, var(--text) 6%, transparent);
        border: 1px solid var(--border);
        border-radius: 4px;
        padding: 2px 5px;
        flex-shrink: 0;
        opacity: 0.6;
        cursor: default;
    }

    .clear-btn {
        width: 32px;
        height: 32px;
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
        border-color: color-mix(in srgb, var(--red) 40%, transparent);
        background: color-mix(in srgb, var(--red) 8%, transparent);
    }

    .close-btn {
        width: 32px;
        height: 32px;
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
        border-color: color-mix(in srgb, var(--red) 40%, transparent);
        background: color-mix(in srgb, var(--red) 8%, transparent);
    }

    .search-presets-panel {
        display: flex;
        flex-direction: column;
        gap: 12px;
        padding: 14px 16px 16px;
        background: var(--surface);
        border: 1px solid var(--border);
        border-top: none;
        border-radius: 0 0 8px 8px;
    }

    .preset-section {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }

    .preset-section-header {
        color: var(--text-secondary);
        font-size: 10px;
        font-weight: 600;
        letter-spacing: 0.04em;
    }

    .preset-list {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
    }

    .search-preset-btn {
        min-height: 30px;
        max-width: 180px;
        display: inline-flex;
        align-items: center;
        gap: 8px;
        padding: 0 10px;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: 6px;
        color: var(--text);
        cursor: pointer;
        font-family: var(--font);
        font-size: 12px;
        transition: border-color 120ms, color 120ms, transform 80ms;
    }

    .search-preset-btn:hover {
        border-color: var(--blue);
        color: var(--blue);
    }

    .search-preset-btn:active {
        transform: translateY(1px);
    }

    .search-preset-btn.auto {
        color: var(--text-secondary);
    }

    .search-preset-btn.auto:hover {
        color: var(--blue);
    }

    .preset-name {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .preset-count {
        color: var(--text-secondary);
        flex-shrink: 0;
        font-size: 11px;
    }

    .preset-loading {
        color: var(--text-secondary);
        font-size: 12px;
    }

    .parsed-rules {
        padding: 16px;
        background: var(--surface);
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
        background: color-mix(in srgb, var(--blue) 14%, transparent);
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
        background: color-mix(in srgb, var(--blue) 22%, transparent);
        color: var(--blue);
    }

    .apply-btn:not(:disabled):active {
        transform: translateY(1px);
    }

    .save-btn {
        height: 32px;
        padding: 0 16px;
        border-radius: 6px;
        border: 1px solid var(--green);
        background: color-mix(in srgb, var(--green) 14%, transparent);
        color: var(--green);
        cursor: pointer;
        font-size: 13px;
        font-weight: 500;
        transition: background 120ms, color 120ms, transform 80ms;
    }

    .save-btn:hover {
        background: color-mix(in srgb, var(--green) 22%, transparent);
    }

    .save-btn:active {
        transform: translateY(1px);
    }

    .name-bar {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 10px 14px;
        background: color-mix(in srgb, var(--green) 4%, var(--surface));
        border: 1px solid color-mix(in srgb, var(--green) 30%, transparent);
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
        height: 32px;
        padding: 0 14px;
        border-radius: 6px;
        border: 1px solid var(--green);
        background: color-mix(in srgb, var(--green) 14%, transparent);
        color: var(--green);
        cursor: pointer;
        font-size: 12px;
        font-weight: 500;
        transition: background 120ms, transform 80ms;
    }

    .save-confirm-btn:hover {
        background: color-mix(in srgb, var(--green) 22%, transparent);
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
        border-color: color-mix(in srgb, var(--text) 12%, transparent);
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
        background: color-mix(in srgb, var(--green) 7%, var(--surface));
        border: 1px solid color-mix(in srgb, var(--green) 30%, transparent);
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
