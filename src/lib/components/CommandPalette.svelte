<script lang="ts">
    import { tick } from 'svelte';
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
        selectedIds,
        smartCollections,
        viewMode,
        type CommandPaletteMode,
    } from '$lib/stores';
    import {
        canAssignCommandHotkey,
        getCommandPaletteItems,
        getShortcutConflict,
        readCommandHotkeys,
        readPinnedCommandIds,
        readRecentCommandIds,
        recordCommandUse,
        removeRecentCommand,
        runCommandPaletteItem,
        setCommandHotkey,
        setCommandPinned,
        shortcutForItem,
        shortcutFromKeyboardEvent,
        sortCommandPaletteItems,
        type CommandPaletteItem,
    } from '$lib/command-palette';

    let query = $state('');
    let inputEl: HTMLInputElement | undefined = $state();
    let hotkeyCaptureEl: HTMLButtonElement | undefined = $state();
    let items = $state<CommandPaletteItem[]>([]);
    let selectedIndex = $state(0);
    let pinnedIds = $state<string[]>([]);
    let recentIds = $state<string[]>([]);
    let hotkeys = $state<Record<string, string>>({});
    let contextMenu = $state<{ itemId: string; x: number; y: number } | null>(null);
    let hotkeyTargetId = $state<string | null>(null);
    let capturedShortcut = $state('');

    let visibleItems = $derived(sortCommandPaletteItems(items, query, {
        mode: $commandPaletteMode,
        pinnedIds,
        recentIds,
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
        hotkeys = readCommandHotkeys();
    }

    function refreshItems() {
        items = getCommandPaletteItems($commandPaletteMode);
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

    function handleOverlayPointerDown(event: PointerEvent) {
        if (event.target === event.currentTarget) closePalette();
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

    function startHotkeyCapture(item: CommandPaletteItem) {
        hotkeyTargetId = item.id;
        capturedShortcut = hotkeys[item.id] ?? item.defaultShortcut ?? '';
        contextMenu = null;
    }

    function clearHotkey(item: CommandPaletteItem) {
        hotkeys = setCommandHotkey(item.id, null);
        contextMenu = null;
    }

    function handleHotkeyKeydown(event: KeyboardEvent) {
        event.preventDefault();
        event.stopPropagation();
        if (event.key === 'Escape') {
            hotkeyTargetId = null;
            capturedShortcut = '';
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
        hotkeyTargetId = null;
        capturedShortcut = '';
    }

    async function copyCommandId(item: CommandPaletteItem) {
        await navigator.clipboard?.writeText(item.id);
        contextMenu = null;
    }

    function itemShortcut(item: CommandPaletteItem): string | undefined {
        return shortcutForItem(item, hotkeys);
    }

    $effect(() => {
        if ($commandPaletteOpen) {
            query = '';
            selectedIndex = 0;
            refreshPreferences();
            refreshItems();
            tick().then(() => inputEl?.focus());
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

    $effect(() => {
        if (hotkeyTarget) {
            tick().then(() => hotkeyCaptureEl?.focus());
        }
    });
</script>

{#if $commandPaletteOpen}
    <div
        class="palette-overlay"
        role="presentation"
        onpointerdown={handleOverlayPointerDown}
    >
        <div class="palette-panel" role="dialog" aria-modal="true" aria-label="Command palette">
            <div class="palette-header">
                <div class="palette-title-group">
                    <span class="palette-title">Command Palette</span>
                    <span class="palette-subtitle">{$commandPaletteMode === 'commands' ? 'Commands' : 'Commands and destinations'}</span>
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
                aria-label="Command palette query"
                onkeydown={handleInputKeydown}
            />

            <div class="palette-results" role="listbox" aria-label="Command palette results">
                {#if visibleItems.length === 0}
                    <div class="empty-result">No matches</div>
                {:else}
                    {#each visibleItems as item, index (item.id)}
                        <button
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
                        {isPinned(contextItem) ? 'Unpin Result' : 'Pin Result'}
                    </button>
                    <button type="button" role="menuitem" onclick={() => startHotkeyCapture(contextItem)}>
                        Set Hotkey...
                    </button>
                    {#if hotkeys[contextItem.id]}
                        <button type="button" role="menuitem" onclick={() => clearHotkey(contextItem)}>
                            Clear Hotkey
                        </button>
                    {/if}
                    {#if recentIds.includes(contextItem.id)}
                        <button type="button" role="menuitem" onclick={() => clearRecent(contextItem)}>
                            Remove from Recents
                        </button>
                    {/if}
                    <button type="button" role="menuitem" onclick={() => copyCommandId(contextItem)}>
                        Copy Command ID
                    </button>
                </div>
            {/if}

            {#if hotkeyTarget}
                <div class="hotkey-panel" role="dialog" aria-modal="true" aria-label="Set command hotkey">
                    <div class="hotkey-card">
                        <div class="hotkey-title">Set Hotkey</div>
                        <div class="hotkey-command">{hotkeyTarget.title}</div>
                        <button
                            type="button"
                            bind:this={hotkeyCaptureEl}
                            class="hotkey-capture"
                            onkeydown={handleHotkeyKeydown}
                        >
                            {capturedShortcut || 'Press shortcut'}
                        </button>
                        {#if shortcutConflict}
                            <div class="hotkey-warning">Already in use: {shortcutConflict}</div>
                        {/if}
                        <div class="hotkey-actions">
                            <button type="button" onclick={() => {
                                hotkeyTargetId = null;
                                capturedShortcut = '';
                            }}>
                                Cancel
                            </button>
                            <button type="button" class="primary" onclick={saveHotkey} disabled={!canSaveHotkey}>
                                Save
                            </button>
                        </div>
                    </div>
                </div>
            {/if}
        </div>
    </div>
{/if}

<style>
    .palette-overlay {
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

    .palette-panel {
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

    .hotkey-panel {
        position: fixed;
        inset: 0;
        z-index: 1230;
        display: flex;
        align-items: center;
        justify-content: center;
        background: color-mix(in srgb, var(--bg) 58%, transparent);
    }

    .hotkey-card {
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
        .palette-overlay {
            padding-top: 16px;
            align-items: flex-start;
        }

        .palette-panel {
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
