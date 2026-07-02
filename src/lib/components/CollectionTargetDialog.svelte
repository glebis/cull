<script lang="ts">
    import { tick } from 'svelte';
    import {
        collectionTargetDialog,
        resolveCollectionTargetDialog,
        type CollectionTargetDialogResult,
    } from '$lib/stores';

    type Mode = 'existing' | 'new';

    let mode = $state<Mode>('new');
    let selectedId = $state('');
    let newName = $state('');
    let query = $state('');
    let error = $state('');
    let currentId = $state(0);
    let searchInputEl: HTMLInputElement | undefined = $state();
    let nameInputEl: HTMLInputElement | undefined = $state();

    let filteredCollections = $derived(
        ($collectionTargetDialog?.collections ?? []).filter(([, name]) =>
            name.toLowerCase().includes(query.trim().toLowerCase())
        )
    );

    $effect(() => {
        const request = $collectionTargetDialog;
        if (!request) {
            currentId = 0;
            return;
        }
        if (request.id === currentId) return;

        currentId = request.id;
        mode = request.collections.length > 0 ? 'existing' : 'new';
        selectedId = request.collections[0]?.[0] ?? '';
        newName = request.initialName ?? '';
        query = '';
        error = '';
    });

    $effect(() => {
        const request = $collectionTargetDialog;
        if (!request) return;

        if (mode === 'existing') {
            const visibleIds = filteredCollections.map(([id]) => id);
            if (visibleIds.length > 0 && !visibleIds.includes(selectedId)) {
                selectedId = visibleIds[0];
            }
            tick().then(() => searchInputEl?.focus());
        } else {
            tick().then(() => nameInputEl?.focus());
        }
    });

    function cancel() {
        resolveCollectionTargetDialog(null);
    }

    function submit() {
        const request = $collectionTargetDialog;
        if (!request) return;

        let result: CollectionTargetDialogResult | null = null;
        if (mode === 'existing') {
            if (!selectedId) {
                error = 'Choose a collection or create a new one';
                return;
            }
            result = { type: 'existing', collectionId: selectedId };
        } else {
            const trimmed = newName.trim();
            if (!trimmed) {
                error = 'Collection name is required';
                tick().then(() => nameInputEl?.focus());
                return;
            }
            result = { type: 'new', name: trimmed };
        }

        resolveCollectionTargetDialog(result);
    }

    function handleKeydown(e: KeyboardEvent) {
        e.stopPropagation();
        if (e.key === 'Escape') {
            e.preventDefault();
            cancel();
        }
        if (e.key === 'Enter') {
            e.preventDefault();
            submit();
        }
    }
</script>

{#if $collectionTargetDialog}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-overlay" onclick={cancel} onkeydown={handleKeydown}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <dialog
        open
        class="dialog"
        aria-modal="true"
        aria-labelledby="collection-target-dialog-title"
        aria-describedby={$collectionTargetDialog.description ? 'collection-target-dialog-description' : undefined}
        onclick={(e: MouseEvent) => e.stopPropagation()}
        onkeydown={handleKeydown}
    >
        <div class="dialog-header">
            <h3 id="collection-target-dialog-title">{$collectionTargetDialog.title}</h3>
            <button class="close-btn" onclick={cancel} aria-label="Close">&times;</button>
        </div>

        <div class="dialog-body">
            {#if $collectionTargetDialog.description}
                <p id="collection-target-dialog-description" class="description">{$collectionTargetDialog.description}</p>
            {/if}

            <div class="mode-switch" role="tablist" aria-label="Collection target">
                <button
                    class:active={mode === 'existing'}
                    disabled={$collectionTargetDialog.collections.length === 0}
                    onclick={() => { mode = 'existing'; error = ''; }}
                    role="tab"
                    aria-selected={mode === 'existing'}
                >
                    Existing
                </button>
                <button
                    class:active={mode === 'new'}
                    onclick={() => { mode = 'new'; error = ''; }}
                    role="tab"
                    aria-selected={mode === 'new'}
                >
                    New
                </button>
            </div>

            {#if mode === 'existing'}
                <input
                    bind:this={searchInputEl}
                    bind:value={query}
                    class="search-input"
                    type="search"
                    placeholder="Filter collections"
                    aria-label="Filter collections"
                    autocomplete="off"
                    spellcheck="false"
                />

                <div class="collection-list" role="listbox" aria-label="Collections">
                    {#each filteredCollections as [id, name, count]}
                        <button
                            class="collection-option"
                            class:active={selectedId === id}
                            onclick={() => { selectedId = id; error = ''; }}
                            role="option"
                            aria-selected={selectedId === id}
                        >
                            <span class="collection-name">{name}</span>
                            <span class="count">{count}</span>
                        </button>
                    {/each}

                    {#if filteredCollections.length === 0}
                        <div class="empty-state">
                            {$collectionTargetDialog.collections.length === 0 ? 'No collections yet' : 'No matching collections'}
                        </div>
                    {/if}
                </div>
            {:else}
                <label class="field-label" for="collection-target-name">Collection name</label>
                <input
                    id="collection-target-name"
                    bind:this={nameInputEl}
                    bind:value={newName}
                    class="name-input"
                    type="text"
                    placeholder="Collection name"
                    autocomplete="off"
                    spellcheck="false"
                    aria-invalid={error ? 'true' : 'false'}
                    aria-describedby={error ? 'collection-target-error' : undefined}
                />
            {/if}

            {#if error}
                <div id="collection-target-error" class="error" role="alert">{error}</div>
            {/if}
        </div>

        <div class="dialog-footer">
            <button class="btn secondary" onclick={cancel}>{$collectionTargetDialog.cancelLabel ?? 'Cancel'}</button>
            <button
                class="btn primary"
                onclick={submit}
                disabled={mode === 'existing' ? !selectedId : !newName.trim()}
            >
                {$collectionTargetDialog.confirmLabel ?? 'Start'}
            </button>
        </div>
    </dialog>
</div>
{/if}

<style>
    .dialog-overlay {
        position: fixed;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: calc(var(--spacing) * 2);
        background: color-mix(in srgb, var(--bg) 78%, transparent);
        z-index: var(--z-modal);
    }

    .dialog {
        position: static;
        display: block;
        width: min(460px, 100%);
        margin: 0;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        box-shadow: 0 18px 60px color-mix(in srgb, var(--bg) 82%, transparent);
        outline: none;
    }

    .dialog-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
        border-bottom: 1px solid var(--border);
    }

    h3 {
        margin: 0;
        color: var(--text);
        font-size: 14px;
        font-weight: 700;
    }

    .close-btn {
        border: none;
        background: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-family: var(--font);
        font-size: 18px;
        line-height: 1;
        padding: 0 4px;
    }

    .close-btn:hover {
        color: var(--text);
    }

    .dialog-body {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
    }

    .description {
        margin: 0 0 2px;
        color: var(--text-secondary);
        font-size: 12px;
    }

    .mode-switch {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 4px;
        padding: 3px;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
    }

    .mode-switch button {
        border: none;
        border-radius: var(--radius);
        background: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-family: var(--font);
        font-size: 12px;
        padding: 5px 8px;
    }

    .mode-switch button.active {
        background: color-mix(in srgb, var(--blue) 16%, var(--surface));
        color: var(--blue);
    }

    .mode-switch button:disabled {
        cursor: not-allowed;
        opacity: 0.45;
    }

    .field-label {
        color: var(--blue);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }

    .search-input,
    .name-input {
        width: 100%;
        min-width: 0;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font-family: var(--font);
        font-size: 13px;
        padding: 8px 10px;
    }

    .search-input:focus,
    .name-input:focus {
        border-color: var(--blue);
        outline: none;
    }

    .collection-list {
        display: flex;
        flex-direction: column;
        gap: 2px;
        max-height: min(260px, 40vh);
        overflow: auto;
        padding: 2px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
    }

    .collection-option {
        display: flex;
        align-items: center;
        gap: var(--spacing);
        width: 100%;
        min-height: 32px;
        border: none;
        border-radius: var(--radius);
        background: none;
        color: var(--text);
        cursor: pointer;
        font-family: var(--font);
        font-size: 12px;
        padding: 6px 8px;
        text-align: left;
    }

    .collection-option:hover,
    .collection-option.active {
        background: color-mix(in srgb, var(--blue) 14%, var(--surface));
        color: var(--blue);
    }

    .collection-name {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .count {
        margin-left: auto;
        color: var(--text-secondary);
        font-size: 11px;
    }

    .empty-state {
        color: var(--text-secondary);
        font-size: 12px;
        padding: 12px 8px;
        text-align: center;
    }

    .error {
        color: var(--red);
        font-size: 11px;
    }

    .dialog-footer {
        display: flex;
        justify-content: flex-end;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
        border-top: 1px solid var(--border);
    }

    .btn {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        cursor: pointer;
        font-family: var(--font);
        font-size: 12px;
        padding: 6px 14px;
    }

    .btn:disabled {
        cursor: not-allowed;
        opacity: 0.5;
    }

    .btn.secondary {
        background: var(--surface);
        color: var(--text-secondary);
    }

    .btn.secondary:hover {
        border-color: var(--text-secondary);
        color: var(--text);
    }

    .btn.primary {
        background: color-mix(in srgb, var(--green) 16%, var(--surface));
        border-color: var(--green);
        color: var(--green);
    }

    .btn.primary:hover:not(:disabled) {
        background: color-mix(in srgb, var(--green) 24%, var(--surface));
    }
</style>
