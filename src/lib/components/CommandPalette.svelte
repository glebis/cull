<script lang="ts">
    import {
        activeCollection,
        activeFolder,
        activeSmartCollection,
        collections,
        commandPaletteMode,
        commandPaletteOpen,
        focusedImage,
        folders,
        images,
        requestTextInput,
        selectedIds,
        shortcutsOpen,
        smartCollections,
        viewMode,
        type CommandPaletteMode,
    } from '$lib/stores';
    import { renameWorkflow, deleteWorkflow } from '$lib/workflows';
    import {
        canAssignCommandHotkey,
        getCommandPaletteItems,
        getShortcutConflict,
        readCommandFrequencies,
        readCommandHotkeys,
        readPinnedCommandIds,
        readRecentCommandIds,
        pruneStalePins,
        recordCommandUse,
        removeRecentCommand,
        runCommandPaletteItem,
        setCommandHotkey,
        setCommandPinned,
        shortcutForItem,
        shortcutFromKeyboardEvent,
        setCommandAlias,
        sortCommandPaletteItems,
        WORKFLOW_CREATE_COMMAND_ID,
        type CommandPaletteItem,
    } from '$lib/command-palette';
    import ModalDialog from './ModalDialog.svelte';

    let query = $state('');
    let inputEl: HTMLInputElement | undefined = $state();
    let items = $state<CommandPaletteItem[]>([]);
    let selectedIndex = $state(0);
    let pinnedIds = $state<string[]>([]);
    let recentIds = $state<string[]>([]);
    let frequencies = $state<Record<string, number>>({});
    let hotkeys = $state<Record<string, string>>({});
    let contextMenu = $state<{ itemId: string; x: number; y: number } | null>(null);
    let hotkeyTargetId = $state<string | null>(null);
    let capturedShortcut = $state('');

    const COMMAND_PALETTE_RESULTS_ID = 'command-palette-results';
    const COMMAND_PALETTE_TITLE_ID = 'command-palette-title';
    const COMMAND_PALETTE_DESCRIPTION_ID = 'command-palette-description';

    let visibleItems = $derived(sortCommandPaletteItems(items, query, {
        mode: $commandPaletteMode,
        pinnedIds,
        recentIds,
        frequencies,
        hotkeys,
    }));
    let selectedItem = $derived(visibleItems[selectedIndex] ?? null);
    let contextItem = $derived(contextMenu ? items.find(item => item.id === contextMenu?.itemId) ?? null : null);
    let hotkeyTarget = $derived(hotkeyTargetId ? items.find(item => item.id === hotkeyTargetId) ?? null : null);
    let shortcutConflict = $derived(
        capturedShortcut && hotkeyTarget
            ? getShortcutConflict(capturedShortcut, hotkeyTarget.id, items, hotkeys)
            : null
    );
    let canSaveHotkey = $derived(
        Boolean(hotkeyTarget) &&
        canAssignCommandHotkey(capturedShortcut, hotkeyTarget?.id ?? '', items, hotkeys)
    );

    function refreshPreferences() {
        pinnedIds = readPinnedCommandIds();
        recentIds = readRecentCommandIds();
        frequencies = readCommandFrequencies();
        hotkeys = readCommandHotkeys();
    }

    function refreshItems(): CommandPaletteItem[] {
        // Return the freshly computed list so callers can use it WITHOUT reading
        // the `items` $state — reading that state inside an $effect would make the
        // effect depend on it and re-run (resetting query) whenever items change.
        const computed = getCommandPaletteItems($commandPaletteMode);
        items = computed;
        return computed;
    }

    function setMode(mode: CommandPaletteMode) {
        commandPaletteMode.set(mode);
        selectedIndex = 0;
        contextMenu = null;
    }

    function closePalette() {
        commandPaletteOpen.set(false);
        contextMenu = null;
        hotkeyTargetId = null;
        capturedShortcut = '';
    }

    async function executeItem(item: CommandPaletteItem | null) {
        if (!item || item.disabled) return;
        contextMenu = null;
        await runCommandPaletteItem(item);
        recentIds = recordCommandUse(item.id);
        frequencies = readCommandFrequencies();
        closePalette();
    }

    function moveSelection(delta: number) {
        if (visibleItems.length === 0) return;
        selectedIndex = (selectedIndex + delta + visibleItems.length) % visibleItems.length;
    }

    function handleInputKeydown(event: KeyboardEvent) {
        if (hotkeyTarget) return;
        if (event.key === 'ArrowDown') {
            event.preventDefault();
            moveSelection(1);
            return;
        }
        if (event.key === 'ArrowUp') {
            event.preventDefault();
            moveSelection(-1);
            return;
        }
        if (event.key === 'Enter') {
            event.preventDefault();
            executeItem(selectedItem);
            return;
        }
        if (event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            if (contextMenu) {
                contextMenu = null;
                return;
            }
            closePalette();
            return;
        }
        if ((event.key === 'F10' && event.shiftKey) || event.key === 'ContextMenu') {
            event.preventDefault();
            openContextMenuForSelected();
        }
    }

    function openContextMenu(event: MouseEvent, item: CommandPaletteItem, index: number) {
        event.preventDefault();
        event.stopPropagation();
        selectedIndex = index;
        const width = 220;
        const height = 188;
        contextMenu = {
            itemId: item.id,
            x: Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8)),
            y: Math.max(8, Math.min(event.clientY, window.innerHeight - height - 8)),
        };
    }

    function openContextMenuForSelected() {
        const item = selectedItem;
        if (!item) return;
        const rect = inputEl?.getBoundingClientRect();
        contextMenu = {
            itemId: item.id,
            x: Math.max(8, (rect?.left ?? 0) + 24),
            y: Math.max(8, (rect?.bottom ?? 0) + 48),
        };
    }

    function isPinned(item: CommandPaletteItem) {
        return pinnedIds.includes(item.id);
    }

    function togglePinned(item: CommandPaletteItem) {
        pinnedIds = setCommandPinned(item.id, !isPinned(item));
        contextMenu = null;
    }

    function clearRecent(item: CommandPaletteItem) {
        recentIds = removeRecentCommand(item.id);
        contextMenu = null;
    }

    function isWorkflowItem(item: CommandPaletteItem) {
        return item.id.startsWith('workflow.') && item.id !== WORKFLOW_CREATE_COMMAND_ID;
    }

    async function addAlias(item: CommandPaletteItem) {
        contextMenu = null;
        const alias = await requestTextInput({
            title: 'Add Alias',
            label: 'Search alias',
            description: `Extra search terms for "${item.title}". Leave empty to clear.`,
            placeholder: 'e.g. gallery wall',
            confirmLabel: 'Save',
        });
        if (alias === null) return;
        setCommandAlias(item.id, alias.trim() || null);
        refreshItems();
    }

    function openInSettings() {
        contextMenu = null;
        closePalette();
        shortcutsOpen.set(true);
    }

    async function renameWorkflowItem(item: CommandPaletteItem) {
        contextMenu = null;
        const name = await requestTextInput({
            title: 'Rename Workflow',
            label: 'Workflow name',
            placeholder: item.title,
            confirmLabel: 'Rename',
        });
        if (!name?.trim()) return;
        renameWorkflow(item.id, name.trim());
        refreshItems();
    }

    function deleteWorkflowItem(item: CommandPaletteItem) {
        contextMenu = null;
        pinnedIds = setCommandPinned(item.id, false);
        deleteWorkflow(item.id);
        refreshItems();
    }

    function startHotkeyCapture(item: CommandPaletteItem) {
        hotkeyTargetId = item.id;
        capturedShortcut = hotkeys[item.id] ?? item.defaultShortcut ?? '';
        contextMenu = null;
    }

    function closeHotkeyCapture() {
        hotkeyTargetId = null;
        capturedShortcut = '';
    }

    function clearHotkey(item: CommandPaletteItem) {
        hotkeys = setCommandHotkey(item.id, null);
        contextMenu = null;
    }

    function handleHotkeyKeydown(event: KeyboardEvent) {
        event.preventDefault();
        event.stopPropagation();
        if (event.key === 'Escape') {
            closeHotkeyCapture();
            return;
        }
        if (event.key === 'Backspace' && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
            capturedShortcut = '';
            return;
        }
        const shortcut = shortcutFromKeyboardEvent(event);
        if (shortcut) capturedShortcut = shortcut;
    }

    function saveHotkey() {
        if (!hotkeyTarget) return;
        if (!canAssignCommandHotkey(capturedShortcut, hotkeyTarget.id, items, hotkeys)) return;
        const customShortcut = capturedShortcut === hotkeyTarget.defaultShortcut ? null : capturedShortcut || null;
        hotkeys = setCommandHotkey(hotkeyTarget.id, customShortcut);
        closeHotkeyCapture();
    }

    async function copyCommandId(item: CommandPaletteItem) {
        await navigator.clipboard?.writeText(item.id);
        contextMenu = null;
    }

    function itemShortcut(item: CommandPaletteItem): string | undefined {
        return shortcutForItem(item, hotkeys);
    }

    function commandOptionId(id: string): string {
        let hash = 0;
        for (const char of id) {
            hash = ((hash << 5) - hash + char.charCodeAt(0)) | 0;
        }
        const slug = id.replace(/[^a-zA-Z0-9_-]+/g, '-').replace(/^-+|-+$/g, '') || 'item';
        return `command-palette-option-${slug.slice(0, 48)}-${Math.abs(hash).toString(36)}`;
    }

    $effect(() => {
        if ($commandPaletteOpen) {
            query = '';
            selectedIndex = 0;
            refreshPreferences();
            const live = refreshItems();
            // Drop pins for destinations that no longer exist (deleted
            // collections, folders, etc.). Done once per open against the local
            // list — never by reading the `items` $state, which would make this
            // effect re-run on every item change and clear the query mid-type.
            pinnedIds = pruneStalePins(live.map(item => item.id));
        }
    });

    $effect(() => {
        $collections;
        $folders;
        $smartCollections;
        $viewMode;
        $focusedImage;
        $images;
        $selectedIds;
        $activeCollection;
        $activeFolder;
        $activeSmartCollection;
        if ($commandPaletteOpen) refreshItems();
    });

    $effect(() => {
        if (selectedIndex >= visibleItems.length) {
            selectedIndex = Math.max(0, visibleItems.length - 1);
        }
    });
</script>

{#if $commandPaletteOpen}
    <ModalDialog
        titleId={COMMAND_PALETTE_TITLE_ID}
        descriptionId={COMMAND_PALETTE_DESCRIPTION_ID}
        onclose={closePalette}
        overlayClass="command-palette-overlay"
        panelClass="palette-panel"
        initialFocus={() => inputEl ?? null}
    >
        <div class="palette-header">
            <div class="palette-title-group">
                <span id={COMMAND_PALETTE_TITLE_ID} class="palette-title">Command Palette</span>
                <span id={COMMAND_PALETTE_DESCRIPTION_ID} class="palette-subtitle">
                    {$commandPaletteMode === 'commands' ? 'Commands' : 'Commands and destinations'}
                </span>
            </div>
            <div class="palette-segment" role="tablist" aria-label="Command palette mode">
                <button
                    type="button"
                    class:active={$commandPaletteMode === 'all'}
                    onclick={() => setMode('all')}
                >
                    All
                </button>
                <button
                    type="button"
                    class:active={$commandPaletteMode === 'commands'}
                    onclick={() => setMode('commands')}
                >
                    Commands
                </button>
            </div>
        </div>

        <input
            bind:this={inputEl}
            class="palette-input"
            bind:value={query}
            placeholder={$commandPaletteMode === 'commands' ? 'Run a command' : 'Run a command or jump to a scope'}
            role="combobox"
            aria-label="Command palette query"
            aria-autocomplete="list"
            aria-expanded={visibleItems.length > 0}
            aria-haspopup="listbox"
            aria-controls={COMMAND_PALETTE_RESULTS_ID}
            aria-activedescendant={selectedItem ? commandOptionId(selectedItem.id) : undefined}
            onkeydown={handleInputKeydown}
        />

        <div
            id={COMMAND_PALETTE_RESULTS_ID}
            class="palette-results"
            role="listbox"
            aria-label="Command palette results"
        >
            {#if visibleItems.length === 0}
                <div class="empty-result">No matches</div>
            {:else}
                {#each visibleItems as item, index (item.id)}
                    <button
                        id={commandOptionId(item.id)}
                        type="button"
                        class="palette-row"
                        class:selected={index === selectedIndex}
                        class:disabled={item.disabled}
                        role="option"
                        aria-selected={index === selectedIndex}
                        disabled={item.disabled}
                        onmouseenter={() => selectedIndex = index}
                        onclick={() => executeItem(item)}
                        oncontextmenu={(event) => openContextMenu(event, item, index)}
                    >
                        <span class="row-mark">{isPinned(item) ? '*' : ''}</span>
                        <span class="row-main">
                            <span class="row-title">{item.title}</span>
                            {#if item.subtitle}
                                <span class="row-subtitle">{item.subtitle}</span>
                            {/if}
                        </span>
                        <span class="row-meta">
                            <span class="row-category">{item.category}</span>
                            {#if itemShortcut(item)}
                                <kbd>{itemShortcut(item)}</kbd>
                            {/if}
                        </span>
                        <span
                            class="row-menu"
                            title="Result actions"
                        >
                            ...
                        </span>
                    </button>
                {/each}
            {/if}
        </div>

        {#if contextMenu && contextItem}
            <div
                class="palette-context-menu"
                style={`left: ${contextMenu.x}px; top: ${contextMenu.y}px;`}
                role="menu"
                tabindex="-1"
                onpointerdown={(event) => event.stopPropagation()}
            >
                <button type="button" role="menuitem" onclick={() => executeItem(contextItem)} disabled={contextItem.disabled}>
                    Run
                </button>
                <button type="button" role="menuitem" onclick={() => togglePinned(contextItem)}>
                    {isPinned(contextItem) ? 'Unfavorite' : 'Favorite'}
                </button>
                <button type="button" role="menuitem" onclick={() => startHotkeyCapture(contextItem)}>
                    Set Hotkey...
                </button>
                {#if hotkeys[contextItem.id]}
                    <button type="button" role="menuitem" onclick={() => clearHotkey(contextItem)}>
                        Clear Hotkey
                    </button>
                {/if}
                <button type="button" role="menuitem" onclick={() => addAlias(contextItem)}>
                    Add Alias...
                </button>
                {#if recentIds.includes(contextItem.id)}
                    <button type="button" role="menuitem" onclick={() => clearRecent(contextItem)}>
                        Remove from Recents
                    </button>
                {/if}
                {#if isWorkflowItem(contextItem)}
                    <button type="button" role="menuitem" onclick={() => renameWorkflowItem(contextItem)}>
                        Rename Workflow...
                    </button>
                    <button type="button" role="menuitem" class="danger" onclick={() => deleteWorkflowItem(contextItem)}>
                        Delete Workflow
                    </button>
                {/if}
                <button type="button" role="menuitem" onclick={() => copyCommandId(contextItem)}>
                    Copy Command ID
                </button>
                <button type="button" role="menuitem" onclick={openInSettings}>
                    Open in Settings
                </button>
            </div>
        {/if}

        {#if hotkeyTarget}
            <ModalDialog
                titleId="set-hotkey-title"
                descriptionId="set-hotkey-description"
                onclose={closeHotkeyCapture}
                overlayClass="hotkey-modal-overlay"
                panelClass="hotkey-card"
            >
                <div class="hotkey-title" id="set-hotkey-title">Set Hotkey</div>
                <div class="hotkey-command" id="set-hotkey-description">{hotkeyTarget.title}</div>
                <button
                    type="button"
                    class="hotkey-capture"
                    data-modal-initial-focus
                    onkeydown={handleHotkeyKeydown}
                >
                    {capturedShortcut || 'Press shortcut'}
                </button>
                {#if shortcutConflict}
                    <div class="hotkey-warning">Already in use: {shortcutConflict}</div>
                {/if}
                <div class="hotkey-actions">
                    <button type="button" onclick={closeHotkeyCapture}>
                        Cancel
                    </button>
                    <button type="button" class="primary" onclick={saveHotkey} disabled={!canSaveHotkey}>
                        Save
                    </button>
                </div>
            </ModalDialog>
        {/if}
    </ModalDialog>
{/if}

<style>
    :global(.command-palette-overlay) {
        position: fixed;
        inset: 0;
        z-index: 1200;
        display: flex;
        align-items: flex-start;
        justify-content: center;
        padding-top: 9vh;
        background: color-mix(in srgb, var(--bg) 74%, transparent);
        backdrop-filter: blur(8px);
    }

    :global(.palette-panel) {
        position: relative;
        width: min(760px, calc(100vw - 32px));
        max-height: min(720px, calc(100vh - 80px));
        display: flex;
        flex-direction: column;
        overflow: hidden;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        box-shadow: 0 24px 88px color-mix(in srgb, var(--bg) 82%, transparent);
    }

    .palette-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        padding: 14px 16px 10px;
        border-bottom: 1px solid var(--border-subtle);
    }

    .palette-title-group {
        display: flex;
        flex-direction: column;
        min-width: 0;
    }

    .palette-title {
        font-size: 13px;
        font-weight: 700;
        letter-spacing: 0;
        color: var(--text);
    }

    .palette-subtitle {
        font-size: 11px;
        color: var(--text-secondary);
    }

    .palette-segment {
        display: inline-flex;
        border: 1px solid var(--border-subtle);
        border-radius: var(--radius);
        overflow: hidden;
        background: var(--bg);
    }

    .palette-segment button {
        height: 28px;
        padding: 0 10px;
        border: 0;
        border-right: 1px solid var(--border-subtle);
        background: transparent;
        color: var(--text-secondary);
        font: inherit;
        cursor: pointer;
    }

    .palette-segment button:last-child {
        border-right: 0;
    }

    .palette-segment button.active {
        background: color-mix(in srgb, var(--blue) 18%, var(--surface));
        color: var(--blue);
    }

    .palette-input {
        width: calc(100% - 32px);
        height: 44px;
        margin: 14px 16px 10px;
        padding: 0 12px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
        font: inherit;
        font-size: 14px;
    }

    .palette-input::placeholder {
        color: var(--text-secondary);
    }

    .palette-results {
        min-height: 220px;
        max-height: min(520px, calc(100vh - 260px));
        overflow: auto;
        padding: 4px 8px 10px;
    }

    .empty-result {
        padding: 32px 12px;
        color: var(--text-secondary);
        text-align: center;
    }

    .palette-row {
        position: relative;
        width: 100%;
        min-height: 52px;
        display: grid;
        grid-template-columns: 18px minmax(0, 1fr) auto 32px;
        align-items: center;
        gap: 8px;
        padding: 8px 6px;
        border: 1px solid transparent;
        border-radius: var(--radius);
        background: transparent;
        color: var(--text);
        font: inherit;
        text-align: left;
        cursor: pointer;
    }

    .palette-row:hover,
    .palette-row.selected {
        border-color: var(--border);
        background: color-mix(in srgb, var(--blue) 10%, transparent);
    }

    .palette-row.disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }

    .row-mark {
        color: var(--orange);
        text-align: center;
        font-weight: 700;
    }

    .row-main {
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 1px;
    }

    .row-title,
    .row-subtitle {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .row-title {
        color: var(--text);
        font-size: 13px;
        font-weight: 500;
    }

    .row-subtitle {
        color: var(--text-secondary);
        font-size: 11px;
    }

    .row-meta {
        display: inline-flex;
        align-items: center;
        justify-content: flex-end;
        gap: 8px;
        min-width: 0;
    }

    .row-category {
        color: var(--purple);
        font-size: 11px;
        white-space: nowrap;
    }

    kbd {
        min-width: 22px;
        height: 22px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        padding: 0 6px;
        border: 1px solid var(--border);
        border-bottom-color: var(--text-secondary);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text-secondary);
        font: inherit;
        font-size: 10px;
        white-space: nowrap;
    }

    .row-menu {
        width: 26px;
        height: 26px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        border-radius: var(--radius);
        color: var(--text-secondary);
    }

    .row-menu:hover {
        background: var(--border-subtle);
        color: var(--text);
    }

    .palette-context-menu {
        position: fixed;
        z-index: 1220;
        width: 220px;
        padding: 4px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        box-shadow: 0 16px 44px color-mix(in srgb, var(--bg) 78%, transparent);
    }

    .palette-context-menu button {
        width: 100%;
        height: 30px;
        padding: 0 8px;
        border: 0;
        border-radius: var(--radius);
        background: transparent;
        color: var(--text);
        font: inherit;
        font-size: 12px;
        text-align: left;
        cursor: pointer;
    }

    .palette-context-menu button:hover {
        background: color-mix(in srgb, var(--blue) 12%, transparent);
    }

    .palette-context-menu button:disabled {
        color: var(--text-secondary);
        cursor: not-allowed;
    }

    :global(.hotkey-modal-overlay) {
        position: fixed;
        inset: 0;
        z-index: 1230;
        display: flex;
        align-items: center;
        justify-content: center;
        background: color-mix(in srgb, var(--bg) 58%, transparent);
    }

    :global(.hotkey-card) {
        width: min(360px, calc(100vw - 32px));
        padding: 16px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        box-shadow: 0 18px 56px color-mix(in srgb, var(--bg) 82%, transparent);
    }

    .hotkey-title {
        font-size: 13px;
        font-weight: 700;
        color: var(--text);
    }

    .hotkey-command {
        margin-top: 2px;
        color: var(--text-secondary);
        font-size: 12px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .hotkey-capture {
        height: 48px;
        margin-top: 14px;
        display: flex;
        align-items: center;
        justify-content: center;
        border: 1px dashed var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--blue);
        font-weight: 700;
    }

    .hotkey-warning {
        margin-top: 8px;
        color: var(--orange);
        font-size: 11px;
    }

    .hotkey-actions {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
        margin-top: 14px;
    }

    .hotkey-actions button {
        height: 30px;
        padding: 0 10px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
        font: inherit;
        cursor: pointer;
    }

    .hotkey-actions button.primary {
        border-color: var(--blue);
        color: var(--blue);
    }

    .hotkey-actions button:disabled {
        border-color: var(--border-subtle);
        color: var(--text-secondary);
        cursor: not-allowed;
        opacity: 0.55;
    }

    @media (max-width: 640px) {
        :global(.command-palette-overlay) {
            padding-top: 16px;
            align-items: flex-start;
        }

        :global(.palette-panel) {
            width: calc(100vw - 16px);
            max-height: calc(100vh - 32px);
        }

        .palette-header {
            align-items: stretch;
            flex-direction: column;
            gap: 10px;
        }

        .palette-segment {
            width: 100%;
        }

        .palette-segment button {
            flex: 1;
        }

        .palette-row {
            grid-template-columns: 16px minmax(0, 1fr) 30px;
        }

        .row-meta {
            display: none;
        }
    }
</style>
