<script lang="ts">
    import { sessions, activeSession, sessionCanvases, activeCanvas, showToast } from '$lib/stores';
    import { listSessions, createSession, listCanvases, validateSessionFolder } from '$lib/api';
    import { onMount } from 'svelte';

    let open = $state(false);
    let search = $state('');
    let creating = $state(false);
    let newName = $state('');
    let rootEl = $state<HTMLDivElement | undefined>();

    function close() {
        open = false;
        search = '';
        creating = false;
    }

    function handleDocumentPointerdown(e: PointerEvent) {
        if (!open) return;
        if (rootEl && e.target instanceof Node && !rootEl.contains(e.target)) close();
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape' && open) {
            e.stopPropagation();
            close();
        }
    }

    let filtered = $derived(
        $sessions.filter(s =>
            s.name.toLowerCase().includes(search.toLowerCase())
        )
    );

    onMount(async () => {
        try {
            const s = await listSessions();
            sessions.set(s);
        } catch (e) {
            console.error('Failed to load sessions:', e);
        }
    });

    async function selectSession(session: typeof $sessions[0] | null) {
        activeCanvas.set(null);
        if (session) {
            const valid = await validateSessionFolder(session.id);
            if (!valid) {
                showToast('Session folder missing — files may be unavailable', { type: 'warning' });
            }
            const canvases = await listCanvases(session.id);
            sessionCanvases.set(canvases);
        } else {
            sessionCanvases.set([]);
        }
        activeSession.set(session);
        open = false;
        search = '';
    }

    async function handleCreate() {
        if (!newName.trim()) return;
        try {
            const session = await createSession(newName.trim());
            sessions.update(s => [session, ...s]);
            await selectSession(session);
            showToast(`Session "${session.name}" created`, { type: 'success' });
        } catch (e) {
            showToast(`Failed to create session: ${e}`, { type: 'error' });
        }
        creating = false;
        newName = '';
    }
</script>

<svelte:document onpointerdown={handleDocumentPointerdown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="session-switcher" bind:this={rootEl} onkeydown={handleKeydown}>
    <button
        class="session-toggle"
        onclick={() => open ? close() : open = true}
        aria-expanded={open}
        aria-haspopup="listbox"
    >
        <span class="session-label">
            {$activeSession?.name ?? 'All Images'}
        </span>
        <span class="chevron" class:open>&#x25BE;</span>
    </button>

    {#if open}
        <div class="session-dropdown">
            <input
                class="session-search"
                type="text"
                placeholder="Search sessions..."
                bind:value={search}
            />

            <button
                class="session-item"
                class:active={!$activeSession}
                onclick={() => selectSession(null)}
            >
                All Images
            </button>

            {#each filtered as session}
                <button
                    class="session-item"
                    class:active={$activeSession?.id === session.id}
                    onclick={() => selectSession(session)}
                >
                    <span class="session-name">{session.name}</span>
                    <span class="count">{session.image_count}</span>
                </button>
            {/each}

            {#if creating}
                <div class="session-create-form">
                    <input
                        class="session-search"
                        type="text"
                        placeholder="Session name..."
                        bind:value={newName}
                        onkeydown={(e) => e.key === 'Enter' && handleCreate()}
                    />
                    <button class="create-btn" onclick={handleCreate}>Create</button>
                </div>
            {:else}
                <button class="session-item new-session" onclick={() => creating = true}>
                    + New Session
                </button>
            {/if}
        </div>
    {/if}
</div>

<style>
    .session-switcher {
        position: relative;
        padding: 8px;
        border-bottom: 1px solid var(--border);
    }
    .session-toggle {
        display: flex;
        align-items: center;
        justify-content: space-between;
        width: 100%;
        padding: 6px 8px;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 4px;
        color: var(--text);
        cursor: pointer;
        font: inherit;
    }
    .session-toggle:hover {
        border-color: var(--blue);
    }
    .session-label {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .chevron {
        transition: transform 0.15s;
        font-size: 10px;
        color: var(--text-secondary);
    }
    .chevron.open {
        transform: rotate(180deg);
    }
    .session-dropdown {
        position: absolute;
        top: 100%;
        left: 8px;
        right: 8px;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 4px;
        z-index: 100;
        max-height: 300px;
        overflow-y: auto;
    }
    .session-search {
        width: 100%;
        padding: 6px 8px;
        background: var(--bg);
        border: none;
        border-bottom: 1px solid var(--border);
        color: var(--text);
        font: inherit;
        outline: none;
        box-sizing: border-box;
    }
    .session-item {
        display: flex;
        align-items: center;
        justify-content: space-between;
        width: 100%;
        padding: 6px 8px;
        background: none;
        border: none;
        color: var(--text);
        cursor: pointer;
        font: inherit;
        text-align: left;
    }
    .session-item:hover {
        background: var(--bg);
    }
    .session-item.active {
        color: var(--blue);
    }
    .session-name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .count {
        color: var(--text-secondary);
        font-size: 11px;
        flex-shrink: 0;
    }
    .new-session {
        color: var(--blue);
        border-top: 1px solid var(--border);
    }
    .session-create-form {
        display: flex;
        gap: 4px;
        padding: 4px;
        border-top: 1px solid var(--border);
    }
    .create-btn {
        padding: 4px 8px;
        background: var(--blue);
        border: none;
        border-radius: 4px;
        color: var(--bg);
        cursor: pointer;
        font: inherit;
        white-space: nowrap;
    }
</style>
