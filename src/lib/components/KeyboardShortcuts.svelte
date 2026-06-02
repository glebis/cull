<script lang="ts">
    import { untrack } from 'svelte';
    import { shortcutsOpen } from '$lib/stores';
    import {
        canAssignCommandHotkey,
        getCommandPaletteItems,
        getShortcutConflict,
        listCommandShortcuts,
        readCommandHotkeys,
        resetCommandHotkeys,
        setCommandHotkey,
        shortcutFromKeyboardEvent,
        type CommandShortcutRow,
    } from '$lib/command-palette';

    let query = $state('');
    let hotkeys = $state<Record<string, string>>({});
    let rows = $state<CommandShortcutRow[]>([]);
    let captureId = $state<string | null>(null);
    let capturedShortcut = $state('');

    function refresh() {
        hotkeys = readCommandHotkeys();
        rows = listCommandShortcuts(getCommandPaletteItems('all'), hotkeys);
    }

    // Only depend on $shortcutsOpen — untrack the resets/refresh so reading
    // state inside refresh() doesn't make this effect re-run and freeze the
    // component's store reactivity (which would stop the panel from closing).
    $effect(() => {
        if ($shortcutsOpen) {
            untrack(() => {
                query = '';
                captureId = null;
                capturedShortcut = '';
                refresh();
            });
        }
    });

    let filtered = $derived(
        rows.filter(row => {
            const q = query.trim().toLowerCase();
            if (!q) return true;
            return (
                row.title.toLowerCase().includes(q) ||
                row.category.toLowerCase().includes(q) ||
                (row.shortcut ?? '').toLowerCase().includes(q)
            );
        })
    );

    let captureConflict = $derived(
        captureId && capturedShortcut
            ? getShortcutConflict(capturedShortcut, captureId, getCommandPaletteItems('all'), hotkeys)
            : null
    );
    let canSave = $derived(
        Boolean(captureId) &&
        canAssignCommandHotkey(capturedShortcut, captureId ?? '', getCommandPaletteItems('all'), hotkeys)
    );

    function startCapture(row: CommandShortcutRow) {
        captureId = row.id;
        capturedShortcut = row.shortcut ?? '';
    }

    function handleCaptureKeydown(event: KeyboardEvent) {
        event.preventDefault();
        event.stopPropagation();
        if (event.key === 'Escape') {
            captureId = null;
            capturedShortcut = '';
            return;
        }
        const shortcut = shortcutFromKeyboardEvent(event);
        if (shortcut) capturedShortcut = shortcut;
    }

    function saveCapture() {
        if (!captureId || !canSave) return;
        setCommandHotkey(captureId, capturedShortcut || null);
        captureId = null;
        capturedShortcut = '';
        refresh();
    }

    function clearBinding(row: CommandShortcutRow) {
        setCommandHotkey(row.id, null);
        refresh();
    }

    function resetAll() {
        resetCommandHotkeys();
        captureId = null;
        capturedShortcut = '';
        refresh();
    }

    function close() {
        shortcutsOpen.set(false);
    }

    function handleBackdropKeydown(event: KeyboardEvent) {
        if (event.key === 'Escape') close();
    }
</script>

{#if $shortcutsOpen}
    <div
        class="shortcuts-backdrop"
        role="dialog"
        aria-modal="true"
        aria-label="Keyboard shortcuts"
        tabindex="-1"
        onclick={close}
        onkeydown={handleBackdropKeydown}
    >
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="shortcuts-panel" role="presentation" onclick={(e) => e.stopPropagation()}>
            <div class="shortcuts-head">
                <span class="shortcuts-title">Keyboard Shortcuts</span>
                <button class="shortcuts-reset" type="button" onclick={resetAll}>Reset to defaults</button>
                <button class="shortcuts-close" type="button" onclick={close} aria-label="Close">×</button>
            </div>
            <input
                class="shortcuts-search"
                type="text"
                placeholder="Search commands or shortcuts…"
                bind:value={query}
            />
            <div class="shortcuts-list">
                {#each filtered as row (row.id)}
                    <div class="shortcuts-row" class:conflict={row.conflict}>
                        <div class="shortcuts-cell-title">
                            <span class="shortcuts-name">{row.title}</span>
                            <span class="shortcuts-category">{row.category}</span>
                        </div>
                        <div class="shortcuts-cell-key">
                            {#if captureId === row.id}
                                <button
                                    class="shortcuts-capture"
                                    type="button"
                                    onkeydown={handleCaptureKeydown}
                                >
                                    {capturedShortcut || 'Press keys…'}
                                </button>
                                {#if captureConflict}
                                    <span class="shortcuts-warn">In use: {captureConflict}</span>
                                {/if}
                                <button class="shortcuts-mini" type="button" disabled={!canSave} onclick={saveCapture}>Save</button>
                                <button class="shortcuts-mini" type="button" onclick={() => { captureId = null; }}>Cancel</button>
                            {:else}
                                {#if row.shortcut}
                                    <kbd class="shortcuts-kbd" class:custom={row.isCustom}>{row.shortcut}</kbd>
                                {:else}
                                    <span class="shortcuts-unset">—</span>
                                {/if}
                                {#if row.conflict}<span class="shortcuts-warn">conflict</span>{/if}
                                <button class="shortcuts-mini" type="button" onclick={() => startCapture(row)}>Set</button>
                                {#if row.isCustom}
                                    <button class="shortcuts-mini" type="button" onclick={() => clearBinding(row)}>Clear</button>
                                {/if}
                            {/if}
                        </div>
                    </div>
                {/each}
                {#if filtered.length === 0}
                    <div class="shortcuts-empty">No commands match “{query}”.</div>
                {/if}
            </div>
        </div>
    </div>
{/if}

<style>
    .shortcuts-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        align-items: flex-start;
        justify-content: center;
        padding-top: 8vh;
        z-index: 1210;
    }
    .shortcuts-panel {
        width: min(680px, 92vw);
        max-height: 78vh;
        display: flex;
        flex-direction: column;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 18px 60px rgba(0, 0, 0, 0.5);
        overflow: hidden;
    }
    .shortcuts-head {
        display: flex;
        align-items: center;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 1.5);
        border-bottom: 1px solid var(--border);
    }
    .shortcuts-title {
        font-weight: 600;
        color: var(--text);
        flex: 1;
    }
    .shortcuts-reset,
    .shortcuts-close,
    .shortcuts-mini {
        background: transparent;
        border: 1px solid var(--border);
        color: var(--text-secondary);
        border-radius: var(--radius);
        padding: 2px 8px;
        cursor: pointer;
        font-family: var(--font, monospace);
        font-size: 12px;
    }
    .shortcuts-close {
        border: none;
        font-size: 18px;
        line-height: 1;
    }
    .shortcuts-reset:hover,
    .shortcuts-close:hover,
    .shortcuts-mini:hover {
        color: var(--text);
        border-color: var(--blue);
    }
    .shortcuts-mini:disabled {
        opacity: 0.4;
        cursor: default;
    }
    .shortcuts-search {
        margin: var(--spacing);
        padding: var(--spacing);
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font-family: var(--font, monospace);
    }
    .shortcuts-list {
        overflow-y: auto;
        padding: 0 var(--spacing) var(--spacing);
    }
    .shortcuts-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--spacing);
        padding: 6px var(--spacing);
        border-bottom: 1px solid var(--border);
    }
    .shortcuts-row.conflict {
        background: rgba(247, 118, 142, 0.08);
    }
    .shortcuts-cell-title {
        display: flex;
        flex-direction: column;
    }
    .shortcuts-name {
        color: var(--text);
    }
    .shortcuts-category {
        color: var(--text-secondary);
        font-size: 11px;
    }
    .shortcuts-cell-key {
        display: flex;
        align-items: center;
        gap: 6px;
    }
    .shortcuts-kbd {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 2px 6px;
        color: var(--text-secondary);
        font-size: 12px;
    }
    .shortcuts-kbd.custom {
        color: var(--blue);
        border-color: var(--blue);
    }
    .shortcuts-unset {
        color: var(--text-secondary);
    }
    .shortcuts-warn {
        color: var(--red);
        font-size: 11px;
    }
    .shortcuts-capture {
        background: var(--bg);
        border: 1px solid var(--blue);
        border-radius: var(--radius);
        padding: 2px 8px;
        color: var(--text);
        font-family: var(--font, monospace);
        font-size: 12px;
        cursor: pointer;
    }
    .shortcuts-empty {
        color: var(--text-secondary);
        padding: var(--spacing);
        text-align: center;
    }
</style>
