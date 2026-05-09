<script lang="ts">
    import { onMount } from 'svelte';
    import { revealItemInDir } from '@tauri-apps/plugin-opener';
    import { setRating, setDecision, listCollections, addToCollection, createCollection, findSimilarImages, trashImages } from '$lib/api';
    import type { ImageWithFile } from '$lib/api';
    import { images, focusedIndex } from '$lib/stores';

    interface Props {
        image: ImageWithFile;
        x: number;
        y: number;
        onclose: () => void;
    }

    let { image, x, y, onclose }: Props = $props();

    let menuEl: HTMLDivElement | undefined = $state();
    let openSubmenu = $state<string | null>(null);
    let collections = $state<[string, string, number][]>([]);
    let menuX = $state(x);
    let menuY = $state(y);

    let currentRating = $derived(image.selection?.star_rating ?? 0);
    let currentDecision = $derived(image.selection?.decision ?? 'undecided');

    onMount(() => {
        if (menuEl) {
            const rect = menuEl.getBoundingClientRect();
            if (rect.right > window.innerWidth) menuX = window.innerWidth - rect.width - 8;
            if (rect.bottom > window.innerHeight) menuY = window.innerHeight - rect.height - 8;
            if (menuX < 0) menuX = 8;
            if (menuY < 0) menuY = 8;
        }

        function handleClickOutside(e: MouseEvent) {
            if (menuEl && !menuEl.contains(e.target as Node)) onclose();
        }
        function handleEscape(e: KeyboardEvent) {
            if (e.key === 'Escape') onclose();
        }
        setTimeout(() => {
            window.addEventListener('click', handleClickOutside);
            window.addEventListener('contextmenu', handleClickOutside);
            window.addEventListener('keydown', handleEscape);
        });
        return () => {
            window.removeEventListener('click', handleClickOutside);
            window.removeEventListener('contextmenu', handleClickOutside);
            window.removeEventListener('keydown', handleEscape);
        };
    });

    function act(fn: () => void | Promise<void>) {
        return async () => {
            onclose();
            await fn();
        };
    }

    async function handleRate(n: number) {
        onclose();
        await setRating(image.image.id, n);
        if (image.selection) image.selection.star_rating = n;
    }

    async function handleDecision(d: string) {
        onclose();
        await setDecision(image.image.id, d);
        if (image.selection) image.selection.decision = d;
    }

    async function loadCollections() {
        openSubmenu = 'collections';
        collections = await listCollections();
    }

    async function handleAddToCollection(colId: string) {
        onclose();
        await addToCollection(colId, [image.image.id]);
    }

    async function handleNewCollection() {
        onclose();
        const name = window.prompt('Collection name:');
        if (!name?.trim()) return;
        const colId = await createCollection(name.trim());
        await addToCollection(colId, [image.image.id]);
    }

    async function handleFindSimilar() {
        onclose();
        try {
            const results = await findSimilarImages(image.image.id, 20);
            const similarIds = results.map(([id]) => id);
            const allImages = [...$images];
            const similar = allImages.filter(img => similarIds.includes(img.image.id));
            if (similar.length > 0) {
                images.set(similar);
                focusedIndex.set(0);
            }
        } catch {
            await navigator.clipboard.writeText(image.image.id);
        }
    }

    async function handleCopyPath() {
        onclose();
        await navigator.clipboard.writeText(image.path);
    }

    async function handleTrash() {
        onclose();
        await trashImages([image.image.id]);
        const allImages = $images.filter(img => img.image.id !== image.image.id);
        images.set(allImages);
    }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
    class="context-menu"
    style="left: {menuX}px; top: {menuY}px;"
    role="menu"
    bind:this={menuEl}
>
    <!-- Rate -->
    <div class="submenu-parent"
        onmouseenter={() => openSubmenu = 'rate'}
        onmouseleave={() => { if (openSubmenu === 'rate') openSubmenu = null; }}
    >
        <button class="context-menu-item has-submenu" role="menuitem">
            <span>Rate</span>
            <span class="current-value">{currentRating > 0 ? '★'.repeat(currentRating) : '—'}</span>
        </button>
        {#if openSubmenu === 'rate'}
            <div class="submenu" role="menu">
                <button class="context-menu-item" class:active={currentRating === 0} onclick={() => handleRate(0)} role="menuitem">☆ Unrated</button>
                {#each [1, 2, 3, 4, 5] as n}
                    <button class="context-menu-item" class:active={currentRating === n} onclick={() => handleRate(n)} role="menuitem">{'★'.repeat(n)} {n} Star{n > 1 ? 's' : ''}</button>
                {/each}
            </div>
        {/if}
    </div>

    <div class="separator"></div>

    <!-- Decision -->
    <button class="context-menu-item" class:active={currentDecision === 'accept'} onclick={() => handleDecision('accept')} role="menuitem">
        <span>Select</span>
        {#if currentDecision === 'accept'}<span class="check">✓</span>{/if}
    </button>
    <button class="context-menu-item" class:active={currentDecision === 'reject'} onclick={() => handleDecision('reject')} role="menuitem">
        <span>Reject</span>
        {#if currentDecision === 'reject'}<span class="check">✓</span>{/if}
    </button>
    <button class="context-menu-item" onclick={() => handleDecision('undecided')} role="menuitem">Clear Decision</button>

    <div class="separator"></div>

    <!-- Collections -->
    <div class="submenu-parent"
        onmouseenter={loadCollections}
        onmouseleave={() => { if (openSubmenu === 'collections') openSubmenu = null; }}
    >
        <button class="context-menu-item has-submenu" role="menuitem">
            <span>Add to Collection</span>
            <span class="arrow">►</span>
        </button>
        {#if openSubmenu === 'collections'}
            <div class="submenu" role="menu">
                {#each collections as [id, name, count]}
                    <button class="context-menu-item" onclick={() => handleAddToCollection(id)} role="menuitem">
                        {name} <span class="count">({count})</span>
                    </button>
                {/each}
                <div class="separator"></div>
                <button class="context-menu-item" onclick={handleNewCollection} role="menuitem">+ New Collection...</button>
            </div>
        {/if}
    </div>

    <div class="separator"></div>

    <!-- Search -->
    <button class="context-menu-item" onclick={handleFindSimilar} role="menuitem">Find Similar</button>

    <div class="separator"></div>

    <!-- Copy -->
    <div class="submenu-parent"
        onmouseenter={() => openSubmenu = 'copy'}
        onmouseleave={() => { if (openSubmenu === 'copy') openSubmenu = null; }}
    >
        <button class="context-menu-item has-submenu" role="menuitem">
            <span>Copy</span>
            <span class="arrow">►</span>
        </button>
        {#if openSubmenu === 'copy'}
            <div class="submenu" role="menu">
                <button class="context-menu-item" onclick={handleCopyPath} role="menuitem">Copy Path</button>
            </div>
        {/if}
    </div>

    <!-- File actions -->
    <button class="context-menu-item" onclick={act(() => revealItemInDir(image.path))} role="menuitem">Reveal in Finder</button>

    <div class="separator"></div>

    <!-- Destructive -->
    <button class="context-menu-item danger" onclick={handleTrash} role="menuitem">Trash</button>
</div>

<style>
    .context-menu {
        position: fixed;
        z-index: 10000;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 4px 0;
        min-width: 200px;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
        font-size: 13px;
        font-family: inherit;
    }
    .context-menu-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        width: 100%;
        padding: 6px 12px;
        background: none;
        border: none;
        color: var(--text);
        font-family: inherit;
        font-size: inherit;
        cursor: pointer;
        text-align: left;
        gap: 12px;
    }
    .context-menu-item:hover {
        background: var(--blue);
        color: var(--bg);
    }
    .context-menu-item.active {
        color: var(--blue);
    }
    .context-menu-item.active:hover {
        color: var(--bg);
    }
    .context-menu-item.danger:hover {
        background: var(--red);
    }
    .context-menu-item.has-submenu {
        padding-right: 8px;
    }
    .separator {
        height: 1px;
        background: var(--border);
        margin: 4px 0;
    }
    .submenu-parent {
        position: relative;
    }
    .submenu {
        position: absolute;
        left: 100%;
        top: -4px;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 4px 0;
        min-width: 180px;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
    }
    .current-value {
        font-size: 11px;
        color: var(--orange);
    }
    .arrow {
        font-size: 9px;
        color: var(--text-secondary);
    }
    .check {
        font-size: 12px;
        color: var(--green);
    }
    .count {
        font-size: 11px;
        color: var(--text-secondary);
    }
</style>
