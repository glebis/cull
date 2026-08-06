<script lang="ts">
    import { sessions, activeSession, sessionCanvases, activeCanvas, collections, showToast, requestConfirm } from '$lib/stores';
    import { listSessions, listCollections, createSession, listCanvases, validateSessionFolder, deleteSession, convertSessionToCollection, type Session } from '$lib/api';
    import { revealItemInDir } from '@tauri-apps/plugin-opener';
    import { onMount } from 'svelte';
    import ActionMenu, { type ActionMenuItem } from './ActionMenu.svelte';

    let open = $state(false);
    let search = $state('');
    let creating = $state(false);
    let newName = $state('');
    let rootEl = $state<HTMLDivElement | undefined>();
    let sessionContextMenu = $state<{ session: Session; x: number; y: number } | null>(null);

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

    function contextPoint(event: MouseEvent | KeyboardEvent): { x: number; y: number } {
        if (event instanceof MouseEvent && event.type === 'contextmenu') {
            return { x: event.clientX, y: event.clientY };
        }
        const target = event.currentTarget as HTMLElement | null;
        const row = target?.closest<HTMLElement>('.session-row') ?? target;
        const rect = row?.getBoundingClientRect();
        return rect
            ? { x: rect.left + Math.min(32, rect.width / 2), y: rect.top + Math.min(24, rect.height) }
            : { x: 16, y: 16 };
    }

    function isContextMenuKey(event: KeyboardEvent): boolean {
        return event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10');
    }

    function openSessionContextMenu(event: MouseEvent | KeyboardEvent, session: Session) {
        event.preventDefault();
        event.stopPropagation();
        sessionContextMenu = { session, ...contextPoint(event) };
    }

    async function openSession(session: Session) {
        try {
            await selectSession(session);
        } catch (e) {
            showToast('Failed to open session', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function revealSessionFolder(session: Session) {
        try {
            await revealItemInDir(session.folder_path);
        } catch (e) {
            showToast('Could not reveal session folder in Finder', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function refreshSessionLists() {
        const [sessionResult, collectionResult] = await Promise.allSettled([listSessions(), listCollections()]);
        if (sessionResult.status === 'fulfilled') sessions.set(sessionResult.value);
        if (collectionResult.status === 'fulfilled') collections.set(collectionResult.value);
        const failures = [sessionResult, collectionResult].filter(result => result.status === 'rejected');
        if (failures.length > 0) {
            showToast('Change saved, but some sidebar lists could not refresh', { type: 'warning', duration: 8000 });
        }
    }

    async function convertSession(session: Session) {
        const confirmed = await requestConfirm({
            title: 'Convert Session to Collection',
            description: `Convert “${session.name}” to a collection? Its canvas layouts will be permanently deleted; images and original files stay available.`,
            confirmLabel: 'Convert Session',
            danger: true,
        });
        if (!confirmed) return;
        try {
            await convertSessionToCollection(session.id);
            if ($activeSession?.id === session.id) {
                activeSession.set(null);
                activeCanvas.set(null);
                sessionCanvases.set([]);
            }
            showToast(`Session "${session.name}" converted to a collection`, { type: 'success' });
        } catch (e) {
            showToast('Failed to convert session to collection', { detail: String(e), type: 'error', duration: 8000 });
            return;
        }
        await refreshSessionLists();
    }

    async function removeSession(session: Session) {
        const confirmed = await requestConfirm({
            title: 'Delete Session',
            description: `Delete session "${session.name}"? Original files stay on disk.`,
            confirmLabel: 'Delete Session',
            danger: true,
        });
        if (!confirmed) return;
        try {
            await deleteSession(session.id, false);
            if ($activeSession?.id === session.id) {
                activeSession.set(null);
                activeCanvas.set(null);
                sessionCanvases.set([]);
            }
            showToast(`Session "${session.name}" deleted`, { type: 'success' });
        } catch (e) {
            showToast('Failed to delete session', { detail: String(e), type: 'error', duration: 8000 });
            return;
        }
        await refreshSessionLists();
    }

    let sessionContextItems = $derived.by((): ActionMenuItem[] => {
        const target = sessionContextMenu;
        if (!target) return [];
        const { session } = target;
        return [
            { id: 'session-open', label: 'Open Session', action: () => openSession(session) },
            { id: 'session-reveal', label: 'Reveal Session Folder in Finder', action: () => revealSessionFolder(session) },
            { id: 'session-convert', label: 'Convert to Collection…', action: () => convertSession(session) },
            {
                id: 'session-delete',
                label: 'Delete Session…',
                action: () => removeSession(session),
                danger: true,
                separatorBefore: true,
            },
        ];
    });
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
                <div class="session-row" class:active={$activeSession?.id === session.id}>
                    <button
                        class="session-item"
                        class:active={$activeSession?.id === session.id}
                        onclick={() => selectSession(session)}
                        oncontextmenu={(event) => openSessionContextMenu(event, session)}
                        onkeydown={(event) => { if (isContextMenuKey(event)) openSessionContextMenu(event, session); }}
                    >
                        <span class="session-name">{session.name}</span>
                        <span class="count">{session.image_count}</span>
                    </button>
                    <button
                        class="session-menu-button"
                        onclick={(event) => openSessionContextMenu(event, session)}
                        title="Session actions"
                        aria-label={`Session actions: ${session.name}`}
                        aria-haspopup="menu"
                    >…</button>
                </div>
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

{#if sessionContextMenu}
    <ActionMenu
        title={sessionContextMenu.session.name}
        x={sessionContextMenu.x}
        y={sessionContextMenu.y}
        items={sessionContextItems}
        onclose={() => sessionContextMenu = null}
    />
{/if}

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
    .session-row {
        align-items: center;
        display: flex;
    }
    .session-row .session-item {
        min-width: 0;
    }
    .session-menu-button {
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        flex-shrink: 0;
        font: inherit;
        opacity: 0;
        padding: 6px 8px;
    }
    .session-row:hover .session-menu-button,
    .session-row:focus-within .session-menu-button {
        opacity: 1;
    }
    .session-menu-button:hover,
    .session-menu-button:focus-visible {
        color: var(--text);
        outline: none;
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
