<script lang="ts">
    import { onMount, tick } from 'svelte';
    import { open as openDialog } from '@tauri-apps/plugin-dialog';
    import { setRating, setDecision, listCollections, addToCollection, removeFromCollection, createCollection, trashImages, moveImage, renameImage, listFolders, shareImages, openImagesWithApplication, listOpenWithApplications } from '$lib/api';
    import { loadSimilarImages } from '$lib/similarity';
    import type { ImageWithFile, OpenWithApplication } from '$lib/api';
    import { images, focusedIndex, selectedIds, activeCollection, activeSession, collections, folders, showToast, requestTextInput } from '$lib/stores';
    import { invalidateImageCache, loadImagesForCurrentScope } from '$lib/image-loading';
    import { clampFloatingPosition } from '$lib/floating-position';
    import { filterMoveFolders, folderDisplayName, folderParentPath } from '$lib/move-menu-utils';
    import { withDecision, withRating, type ImageDecision } from '$lib/selection-updates';

    interface Props {
        image: ImageWithFile;
        x: number;
        y: number;
        onclose: () => void;
    }

    let { image, x, y, onclose }: Props = $props();

    let menuEl: HTMLDivElement | undefined = $state();
    let openSubmenu = $state<string | null>(null);
    let collectionList = $state<[string, string, number][]>([]);
    let folderList = $state<[string, number][]>([]);
    let openWithApps = $state<OpenWithApplication[]>([]);
    let openWithLoading = $state(false);
    let openWithLoadedFor = $state<string | null>(null);
    let folderSearch = $state('');
    let menuX = $state(0);
    let menuY = $state(0);
    let menuReady = $state(false);
    let activeIndex = $state(0);
    let placementRun = 0;

    let currentRating = $derived(image.selection?.star_rating ?? 0);
    let currentDecision = $derived(image.selection?.decision ?? 'undecided');
    let filteredFolderList = $derived(filterMoveFolders(folderList, folderSearch));

    let targetIds = $derived(
        $selectedIds.size > 0 && $selectedIds.has(image.image.id)
            ? [...$selectedIds]
            : [image.image.id]
    );
    let multiCount = $derived(targetIds.length);
    let inCollection = $derived($activeCollection !== null);

    let flatItems = $derived(
        menuEl
            ? Array.from(menuEl.querySelectorAll<HTMLButtonElement>('button[data-menu-index]'))
            : []
    );

    async function placeMenu(anchorX: number, anchorY: number) {
        const run = ++placementRun;
        menuReady = false;
        menuX = anchorX;
        menuY = anchorY;
        await tick();

        if (run !== placementRun || !menuEl) return;

        const rect = menuEl.getBoundingClientRect();
        const next = clampFloatingPosition(
            { x: anchorX, y: anchorY },
            { width: rect.width, height: rect.height },
            { width: window.innerWidth, height: window.innerHeight },
        );

        menuX = next.x;
        menuY = next.y;
        await tick();

        if (run === placementRun && menuEl) {
            menuReady = true;
            if (!menuEl.contains(document.activeElement)) {
                menuEl.focus();
            }
        }
    }

    $effect(() => {
        if (!menuEl) return;
        void placeMenu(x, y);
    });

    onMount(() => {
        function handleClickOutside(e: MouseEvent) {
            if (menuEl && !menuEl.contains(e.target as Node)) onclose();
        }
        function handleResize() {
            void placeMenu(x, y);
        }
        setTimeout(() => {
            window.addEventListener('click', handleClickOutside);
            window.addEventListener('contextmenu', handleClickOutside);
            window.addEventListener('resize', handleResize);
        });
        return () => {
            window.removeEventListener('click', handleClickOutside);
            window.removeEventListener('contextmenu', handleClickOutside);
            window.removeEventListener('resize', handleResize);
        };
    });

    async function revealInFinder(path: string) {
        const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
        await revealItemInDir(path);
    }

    $effect(() => {
        if (menuEl) {
            flatItems[activeIndex]?.focus();
        }
    });

    function handleMenuKeydown(e: KeyboardEvent) {
        const items = flatItems;
        const count = items.length;
        if (count === 0) return;

        if (e.key === 'ArrowDown') {
            e.preventDefault();
            activeIndex = (activeIndex + 1) % count;
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            activeIndex = (activeIndex - 1 + count) % count;
        } else if (e.key === 'ArrowRight') {
            e.preventDefault();
            // Find which submenu-parent the active button belongs to
            const activeBtn = items[activeIndex];
            if (activeBtn?.classList.contains('has-submenu')) {
                const parentEl = activeBtn.closest('.submenu-parent');
                if (parentEl) {
                    // Determine submenu key from data attribute or order
                    const key = activeBtn.dataset.submenuKey;
                    if (key === 'rate') openSubmenu = 'rate';
                    else if (key === 'collections') { loadCollections(); }
                    else if (key === 'copy') openSubmenu = 'copy';
                    else if (key === 'openwith') { loadOpenWithApps(); }
                    else if (key === 'moveto') { loadFolders(); }
                }
            }
        } else if (e.key === 'ArrowLeft' || e.key === 'Escape') {
            e.preventDefault();
            if (openSubmenu !== null) {
                openSubmenu = null;
            } else {
                onclose();
            }
        } else if (e.key === 'Enter') {
            e.preventDefault();
            const activeBtn = items[activeIndex];
            activeBtn?.click();
        }
    }

    function act(fn: () => void | Promise<void>) {
        return async () => {
            onclose();
            await fn();
        };
    }

    async function handleRate(n: number) {
        onclose();
        await setRating(image.image.id, n, $activeSession?.id ?? null);
        invalidateImageCache();
        image.selection = withRating(image, n).selection;
        images.update(all => all.map(item => item.image.id === image.image.id ? withRating(item, n) : item));
    }

    async function handleDecision(d: ImageDecision) {
        onclose();
        await setDecision(image.image.id, d, $activeSession?.id ?? null);
        invalidateImageCache();
        image.selection = withDecision(image, d).selection;
        images.update(all => all.map(item => item.image.id === image.image.id ? withDecision(item, d) : item));
    }

    async function loadCollections() {
        openSubmenu = 'collections';
        collectionList = await listCollections();
    }

    async function handleAddToCollection(colId: string) {
        onclose();
        await addToCollection(colId, targetIds);
        invalidateImageCache();
        const c = await listCollections();
        collections.set(c);
    }

    async function handleNewCollection() {
        onclose();
        const imageLabel = targetIds.length === 1 ? '1 image' : `${targetIds.length} images`;
        const name = await requestTextInput({
            title: 'New Collection',
            label: 'Collection name',
            description: `${imageLabel} will be added.`,
            placeholder: 'Collection name',
            confirmLabel: 'Create and Add',
        });
        if (!name?.trim()) return;
        const colId = await createCollection(name.trim());
        await addToCollection(colId, targetIds);
        invalidateImageCache();
        const c = await listCollections();
        collections.set(c);
    }

    async function handleRemoveFromCollection() {
        const colId = $activeCollection;
        if (!colId) return;
        onclose();
        await removeFromCollection(colId, targetIds);
        await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        focusedIndex.update(i => Math.max(0, Math.min(i, $images.length - 1)));
        const c = await listCollections();
        collections.set(c);
    }

    async function handleFindSimilar() {
        onclose();
        try {
            await loadSimilarImages(image.image.id, 20);
        } catch {
            await navigator.clipboard.writeText(image.image.id);
        }
    }

    async function handleCopyPath() {
        onclose();
        await navigator.clipboard.writeText(targetImages().map(img => img.path).join('\n'));
    }

    async function handleCopyFilename() {
        onclose();
        await navigator.clipboard.writeText(targetImages().map(img => img.path.split('/').pop() ?? img.path).join('\n'));
    }

    function toFileUrl(path: string) {
        return `file://${path.split('/').map((part, index) => index === 0 ? '' : encodeURIComponent(part)).join('/')}`;
    }

    async function handleCopyFileUrl() {
        onclose();
        await navigator.clipboard.writeText(targetImages().map(img => toFileUrl(img.path)).join('\n'));
    }

    function targetImages() {
        const byId = new Map($images.map(img => [img.image.id, img]));
        return targetIds.map(id => byId.get(id) ?? (id === image.image.id ? image : null)).filter((img): img is ImageWithFile => img !== null);
    }

    async function handleShare() {
        onclose();
        try {
            await shareImages([...new Set(targetIds)]);
        } catch (e) {
            showToast('Share failed', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function openInDefaultApp(path: string) {
        const { openPath } = await import('@tauri-apps/plugin-opener');
        await openPath(path);
    }

    async function handleOpenWith() {
        onclose();
        const selected = await openDialog({
            title: 'Open With',
            directory: true,
            multiple: false,
            defaultPath: '/Applications',
            fileAccessMode: 'scoped',
        });
        if (!selected || Array.isArray(selected)) return;

        await handleOpenWithApp(selected);
    }

    async function loadOpenWithApps() {
        openSubmenu = 'openwith';
        if (openWithLoadedFor === image.image.id) return;

        openWithLoading = true;
        try {
            openWithApps = await listOpenWithApplications(image.image.id);
            openWithLoadedFor = image.image.id;
        } catch (e) {
            openWithApps = [];
            showToast('Open With app list unavailable', { detail: String(e), type: 'warning', duration: 8000 });
        } finally {
            openWithLoading = false;
        }
    }

    async function handleOpenWithApp(appPath: string) {
        onclose();
        try {
            await openImagesWithApplication([image.image.id], appPath);
        } catch (e) {
            showToast('Open With failed', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    async function handleTrash() {
        onclose();
        const ids = new Set(targetIds);
        await trashImages([...ids]);
        const remainingLoadedCount = $images.filter(img => !ids.has(img.image.id)).length;
        await loadImagesForCurrentScope({
            resetFocus: false,
            force: true,
            invalidateCache: true,
            minItems: remainingLoadedCount,
        });
        if ($focusedIndex >= $images.length) focusedIndex.set(Math.max(0, $images.length - 1));
    }

    async function handleRename() {
        onclose();
        const currentName = image.path.split('/').pop() ?? '';
        const newName = await requestTextInput({
            title: 'Rename File',
            label: 'File name',
            initialValue: currentName,
            confirmLabel: 'Rename',
        });
        if (!newName?.trim() || newName.trim() === currentName) return;
        try {
            await renameImage(image.image.id, newName.trim());
            await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
            showToast(`Renamed to ${newName.trim()}`, { type: 'success' });
        } catch (e) {
            showToast(`Rename failed: ${e}`, { type: 'error' });
        }
    }

    async function loadFolders() {
        openSubmenu = 'moveto';
        folderList = await listFolders();
    }

    function currentFolderPath() {
        const parts = image.path.split('/');
        parts.pop();
        return parts.join('/') || undefined;
    }

    async function refreshAfterMove() {
        await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        try {
            folders.set(await listFolders());
        } catch (e) {
            console.error('Failed to refresh folders after move:', e);
        }
    }

    async function moveImagesToFolder(ids: string[], folder: string) {
        let moved = 0;
        try {
            for (const id of ids) {
                await moveImage(id, folder);
                moved += 1;
            }
        } catch (e) {
            if (moved > 0) {
                await refreshAfterMove();
            }
            showToast('Move incomplete', {
                detail: `${moved}/${ids.length} moved. ${String(e)}`,
                type: 'error',
                duration: 10000,
            });
            return;
        }

        await refreshAfterMove();
        const folderName = folderDisplayName(folder);
        const movedLabel = moved === 1 ? '1 image' : `${moved} images`;
        showToast(`Moved ${movedLabel} to ${folderName}`, { type: 'success' });
    }

    async function handleMoveTo(folder: string) {
        const ids = [...new Set(targetIds)];
        onclose();
        await moveImagesToFolder(ids, folder);
    }

    async function handleChooseMoveFolder() {
        const ids = [...new Set(targetIds)];
        const defaultPath = currentFolderPath();
        onclose();

        const selected = await openDialog({
            title: ids.length === 1 ? 'Move Image to Folder' : `Move ${ids.length} Images to Folder`,
            directory: true,
            multiple: false,
            defaultPath,
            canCreateDirectories: true,
            fileAccessMode: 'scoped',
        });
        if (!selected || Array.isArray(selected)) return;

        await moveImagesToFolder(ids, selected);
    }

    function handleFolderSearchKeydown(e: KeyboardEvent) {
        e.stopPropagation();
        if (e.key === 'Escape') {
            e.preventDefault();
            folderSearch = '';
        }
    }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
    class="context-menu"
    style="left: {menuX}px; top: {menuY}px; visibility: {menuReady ? 'visible' : 'hidden'};"
    role="menu"
    tabindex="-1"
    bind:this={menuEl}
    onkeydown={handleMenuKeydown}
>
    {#if multiCount > 1}
        <div class="context-menu-header">{multiCount} images selected</div>
        <div class="separator"></div>
    {/if}

    <!-- Rate -->
    <div class="submenu-parent"
        onmouseenter={() => openSubmenu = 'rate'}
        onmouseleave={() => { if (openSubmenu === 'rate') openSubmenu = null; }}
    >
        <button
            class="context-menu-item has-submenu"
            role="menuitem"
            data-menu-index="0"
            data-submenu-key="rate"
            tabindex={activeIndex === 0 ? 0 : -1}
        >
            <span>Rate</span>
            <span class="current-value">{currentRating > 0 ? '★'.repeat(currentRating) : '—'}</span>
        </button>
        {#if openSubmenu === 'rate'}
            <div class="submenu" role="menu">
                <button class="context-menu-item" class:active={currentRating === 0} onclick={() => handleRate(0)} role="menuitem" tabindex="-1">☆ Unrated</button>
                {#each [1, 2, 3, 4, 5] as n}
                    <button class="context-menu-item" class:active={currentRating === n} onclick={() => handleRate(n)} role="menuitem" tabindex="-1">{'★'.repeat(n)} {n} Star{n > 1 ? 's' : ''}</button>
                {/each}
            </div>
        {/if}
    </div>

    <div class="separator"></div>

    <!-- Decision -->
    <button
        class="context-menu-item"
        class:active={currentDecision === 'accept'}
        onclick={() => handleDecision('accept')}
        role="menuitem"
        data-menu-index="1"
        tabindex={activeIndex === 1 ? 0 : -1}
    >
        <span>Accept</span>
        {#if currentDecision === 'accept'}<span class="check">✓</span>{/if}
    </button>
    <button
        class="context-menu-item"
        class:active={currentDecision === 'reject'}
        onclick={() => handleDecision('reject')}
        role="menuitem"
        data-menu-index="2"
        tabindex={activeIndex === 2 ? 0 : -1}
    >
        <span>Reject</span>
        {#if currentDecision === 'reject'}<span class="check">✓</span>{/if}
    </button>
    <button
        class="context-menu-item"
        onclick={() => handleDecision('undecided')}
        role="menuitem"
        data-menu-index="3"
        tabindex={activeIndex === 3 ? 0 : -1}
    >Clear Decision</button>

    <div class="separator"></div>

    <!-- Collections -->
    <div class="submenu-parent"
        onmouseenter={loadCollections}
        onmouseleave={() => { if (openSubmenu === 'collections') openSubmenu = null; }}
    >
        <button
            class="context-menu-item has-submenu"
            role="menuitem"
            data-menu-index="4"
            data-submenu-key="collections"
            tabindex={activeIndex === 4 ? 0 : -1}
        >
            <span>Add to Collection</span>
            <span class="arrow">►</span>
        </button>
        {#if openSubmenu === 'collections'}
            <div class="submenu" role="menu">
                {#each collectionList as [id, name, count]}
                    <button class="context-menu-item" onclick={() => handleAddToCollection(id)} role="menuitem" tabindex="-1">
                        {name} <span class="count">({count})</span>
                    </button>
                {/each}
                <div class="separator"></div>
                <button class="context-menu-item" onclick={handleNewCollection} role="menuitem" tabindex="-1">+ New Collection...</button>
            </div>
        {/if}
    </div>

    {#if inCollection}
        <button
            class="context-menu-item danger"
            onclick={handleRemoveFromCollection}
            role="menuitem"
            data-menu-index="5"
            tabindex={activeIndex === 5 ? 0 : -1}
        >Remove from Collection{multiCount > 1 ? ` (${multiCount})` : ''}</button>
    {/if}

    <div class="separator"></div>

    <!-- Search -->
    <button
        class="context-menu-item"
        onclick={handleFindSimilar}
        role="menuitem"
        data-menu-index="6"
        tabindex={activeIndex === 6 ? 0 : -1}
    >Find Similar</button>

    <div class="separator"></div>

    <!-- Copy -->
    <div class="submenu-parent"
        onmouseenter={() => openSubmenu = 'copy'}
        onmouseleave={() => { if (openSubmenu === 'copy') openSubmenu = null; }}
    >
        <button
            class="context-menu-item has-submenu"
            role="menuitem"
            data-menu-index="7"
            data-submenu-key="copy"
            tabindex={activeIndex === 7 ? 0 : -1}
        >
            <span>Copy</span>
            <span class="arrow">►</span>
        </button>
        {#if openSubmenu === 'copy'}
            <div class="submenu" role="menu">
                <button class="context-menu-item" onclick={handleCopyPath} role="menuitem" tabindex="-1">Copy Path{multiCount > 1 ? 's' : ''}</button>
                <button class="context-menu-item" onclick={handleCopyFilename} role="menuitem" tabindex="-1">Copy Filename{multiCount > 1 ? 's' : ''}</button>
                <button class="context-menu-item" onclick={handleCopyFileUrl} role="menuitem" tabindex="-1">Copy File URL{multiCount > 1 ? 's' : ''}</button>
            </div>
        {/if}
    </div>

    <button
        class="context-menu-item"
        onclick={handleShare}
        role="menuitem"
        data-menu-index="8"
        tabindex={activeIndex === 8 ? 0 : -1}
    >Share{multiCount > 1 ? ` (${multiCount})` : ''}...</button>

    <!-- File actions -->
    <button
        class="context-menu-item"
        onclick={act(() => revealInFinder(image.path))}
        role="menuitem"
        data-menu-index="9"
        tabindex={activeIndex === 9 ? 0 : -1}
    >Reveal in Finder</button>

    {#if multiCount === 1}
        <button
            class="context-menu-item"
            onclick={act(() => openInDefaultApp(image.path))}
            role="menuitem"
            data-menu-index="10"
            tabindex={activeIndex === 10 ? 0 : -1}
        >Open in Default App</button>
        <div class="submenu-parent"
            onmouseenter={loadOpenWithApps}
            onmouseleave={() => { if (openSubmenu === 'openwith') openSubmenu = null; }}
        >
            <button
                class="context-menu-item has-submenu"
                role="menuitem"
                data-menu-index="11"
                data-submenu-key="openwith"
                tabindex={activeIndex === 11 ? 0 : -1}
            >
                <span>Open With</span>
                <span class="arrow">►</span>
            </button>
            {#if openSubmenu === 'openwith'}
                <div class="submenu open-with-submenu" role="menu">
                    {#if openWithLoading}
                        <div class="context-menu-item empty-menu-item">Loading...</div>
                    {:else}
                        {#each openWithApps as app}
                            <button class="context-menu-item" onclick={() => handleOpenWithApp(app.path)} role="menuitem" tabindex="-1" title={app.path}>
                                <span>{app.name}</span>
                                {#if app.is_default}<span class="count">Default</span>{/if}
                            </button>
                        {/each}
                        {#if openWithApps.length > 0}
                            <div class="separator"></div>
                        {/if}
                        <button class="context-menu-item" onclick={handleOpenWith} role="menuitem" tabindex="-1">
                            Choose Application...
                        </button>
                    {/if}
                </div>
            {/if}
        </div>
    {/if}

    <button
        class="context-menu-item"
        onclick={handleRename}
        role="menuitem"
        data-menu-index="12"
        tabindex={activeIndex === 12 ? 0 : -1}
    >Rename...</button>

    <!-- Move to -->
    <div class="submenu-parent"
        onmouseenter={loadFolders}
        onmouseleave={() => { if (openSubmenu === 'moveto') openSubmenu = null; }}
    >
        <button
            class="context-menu-item has-submenu"
            role="menuitem"
            data-menu-index="13"
            data-submenu-key="moveto"
            tabindex={activeIndex === 13 ? 0 : -1}
        >
            <span>Move to...</span>
            <span class="arrow">►</span>
        </button>
        {#if openSubmenu === 'moveto'}
            <div class="submenu move-submenu" role="menu">
                <button class="context-menu-item" onclick={handleChooseMoveFolder} role="menuitem" tabindex="-1">
                    Choose Folder...
                </button>
                <div class="separator"></div>
                <div class="folder-search-row">
                    <input
                        class="folder-search"
                        type="search"
                        placeholder="Search folders"
                        aria-label="Search folders"
                        bind:value={folderSearch}
                        onkeydown={handleFolderSearchKeydown}
                    />
                </div>
                <div class="folder-list">
                    {#each filteredFolderList as [folder, count]}
                        <button class="context-menu-item folder-item" onclick={() => handleMoveTo(folder)} role="menuitem" tabindex="-1" title={folder}>
                            <span class="folder-text">
                                <span class="folder-name">{folderDisplayName(folder)}</span>
                                <span class="folder-path">{folderParentPath(folder)}</span>
                            </span>
                            <span class="count">({count})</span>
                        </button>
                    {/each}
                    {#if filteredFolderList.length === 0}
                        <div class="context-menu-item empty-menu-item">
                            {folderList.length === 0 ? 'No folders' : 'No matching folders'}
                        </div>
                    {/if}
                </div>
            </div>
        {/if}
    </div>

    <div class="separator"></div>

    <!-- Destructive -->
    <button
        class="context-menu-item danger"
        onclick={handleTrash}
        role="menuitem"
        data-menu-index="14"
        tabindex={activeIndex === 14 ? 0 : -1}
    >Trash{multiCount > 1 ? ` (${multiCount})` : ''}</button>
</div>

<style>
    .context-menu {
        position: fixed;
        z-index: var(--z-context-menu);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 4px 0;
        min-width: 200px;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
        font-size: 13px;
        font-family: inherit;
        outline: none;
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
    .context-menu-item:hover,
    .context-menu-item:focus {
        background: var(--blue);
        color: var(--bg);
        outline: none;
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
    .context-menu-item:hover .count,
    .context-menu-item:focus .count,
    .context-menu-item:hover .folder-path,
    .context-menu-item:focus .folder-path {
        color: var(--bg);
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
    .move-submenu {
        display: flex;
        flex-direction: column;
        min-width: 340px;
        max-width: min(460px, calc(100vw - 32px));
        max-height: min(420px, calc(100vh - 24px));
        overflow: hidden;
    }
    .open-with-submenu {
        min-width: 240px;
        max-width: min(360px, calc(100vw - 32px));
    }
    .folder-search-row {
        padding: 4px 8px 6px;
    }
    .folder-search {
        box-sizing: border-box;
        width: 100%;
        padding: 6px 8px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
        font: inherit;
        outline: none;
    }
    .folder-search:focus {
        border-color: var(--blue);
    }
    .folder-list {
        min-height: 0;
        overflow-y: auto;
    }
    .folder-item {
        align-items: flex-start;
    }
    .folder-text {
        display: flex;
        flex-direction: column;
        min-width: 0;
        gap: 2px;
    }
    .folder-name,
    .folder-path {
        max-width: 260px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .folder-path {
        color: var(--text-secondary);
        font-size: 10px;
    }
    .empty-menu-item {
        color: var(--text-secondary);
        cursor: default;
    }
    .empty-menu-item:hover,
    .empty-menu-item:focus {
        background: none;
        color: var(--text-secondary);
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
    .context-menu-header {
        padding: 4px 12px;
        font-size: 11px;
        color: var(--blue);
        font-weight: 600;
    }
</style>
