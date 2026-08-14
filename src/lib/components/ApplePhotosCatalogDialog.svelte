<script lang="ts">
    import { onDestroy, onMount } from 'svelte';
    import ModalDialog from '$lib/components/ModalDialog.svelte';
    import ThumbnailScaleControl from '$lib/components/ThumbnailScaleControl.svelte';
    import {
        tauriApplePhotosCatalogClient,
        type ApplePhotosAlbum,
        type ApplePhotosAsset,
        type ApplePhotosAssetFilter,
        type ApplePhotosAssetSort,
        type ApplePhotosAuthorization,
        type ApplePhotosCatalogClient,
    } from '$lib/apple-photos';
    import {
        nudgeThumbnailSize,
        thumbnailSizeFromZoomPosition,
        zoomPositionFromThumbnailSize,
    } from '$lib/thumbnail-zoom';

    interface Props {
        onclose: () => void;
        client?: ApplePhotosCatalogClient;
    }

    let { onclose, client = tauriApplePhotosCatalogClient }: Props = $props();

    const PAGE_SIZE = 100;
    const MAX_CACHED_PREVIEWS = 96;
    const PREVIEW_SIZE_MIN = 88;
    const PREVIEW_SIZE_MAX = 220;
    const PREVIEW_SIZE_DEFAULT = 120;
    const PREVIEW_REQUEST_SIZE = 512;
    let authorization = $state<ApplePhotosAuthorization | null>(null);
    let albums = $state<ApplePhotosAlbum[]>([]);
    let albumsHasMore = $state(false);
    let selectedAlbumId = $state('');
    let assets = $state<ApplePhotosAsset[]>([]);
    let assetsTotal = $state(0);
    let assetsHasMore = $state(false);
    let assetsNextOffset = $state(0);
    let assetFilter = $state<ApplePhotosAssetFilter>('all');
    let assetSort = $state<ApplePhotosAssetSort>('newest');
    let checking = $state(true);
    let requesting = $state(false);
    let albumsLoading = $state(false);
    let assetsLoading = $state(false);
    let authorizationError = $state<string | null>(null);
    let albumsError = $state<string | null>(null);
    let assetsError = $state<string | null>(null);
    let importing = $state(false);
    let importError = $state<string | null>(null);
    let authorizationGeneration = 0;
    let albumsGeneration = 0;
    let assetsGeneration = 0;
    let selectedAssetIds = $state<Set<string>>(new Set());
    let previews = $state<Record<string, string | null | undefined>>({});
    let assetGridWidth = $state(0);
    let previewScalePosition = $state(
        zoomPositionFromThumbnailSize(PREVIEW_SIZE_DEFAULT, PREVIEW_SIZE_MIN, PREVIEW_SIZE_MAX),
    );
    const previewRequests = new Map<string, Promise<string | null>>();
    const previewAssets = new WeakMap<Element, ApplePhotosAsset>();
    let previewObserver: IntersectionObserver | null = null;
    let previewOrder: string[] = [];
    let previewGeneration = 0;

    const canBrowse = $derived(authorization === 'authorized' || authorization === 'limited');

    function messageFrom(errorValue: unknown): string {
        if (errorValue instanceof Error) return errorValue.message;
        if (typeof errorValue === 'string') return errorValue;
        if (
            typeof errorValue === 'object' &&
            errorValue !== null &&
            'message' in errorValue &&
            typeof errorValue.message === 'string'
        ) return errorValue.message;
        return 'Apple Photos could not be reached.';
    }

    async function loadAlbums(offset = 0, append = false) {
        const generation = ++albumsGeneration;
        albumsLoading = true;
        albumsError = null;
        try {
            const page = await client.listAlbums(offset, PAGE_SIZE);
            if (generation !== albumsGeneration) return;
            albums = append ? [...albums, ...page.items] : page.items;
            albumsHasMore = page.has_more;
        } catch (loadError) {
            if (generation !== albumsGeneration) return;
            albumsError = messageFrom(loadError);
        } finally {
            if (generation === albumsGeneration) albumsLoading = false;
        }
    }

    async function loadAssets(albumId: string | null, offset = 0, append = false) {
        const generation = ++assetsGeneration;
        assetsLoading = true;
        assetsError = null;
        try {
            const page = await client.listAssets(albumId, offset, PAGE_SIZE, assetFilter, assetSort);
            if (generation !== assetsGeneration) return;
            const existingIds = new Set(append ? assets.map(asset => asset.id) : []);
            const uniquePageItems = page.items.filter(asset => {
                if (existingIds.has(asset.id)) return false;
                existingIds.add(asset.id);
                return true;
            });
            assets = append ? [...assets, ...uniquePageItems] : uniquePageItems;
            assetsTotal = page.total;
            assetsHasMore = page.has_more;
            // The provider cursor advances by consumed rows, not rendered rows: PhotoKit
            // pages can overlap when assets share the same creation timestamp.
            assetsNextOffset = page.offset + page.items.length;
        } catch (loadError) {
            if (generation !== assetsGeneration) return;
            assetsError = messageFrom(loadError);
        } finally {
            if (generation === assetsGeneration) assetsLoading = false;
        }
    }

    function beginCatalogLoad() {
        void loadAlbums();
        void loadAssets(null);
    }

    async function checkAuthorization() {
        const generation = ++authorizationGeneration;
        checking = true;
        authorizationError = null;
        try {
            const status = await client.authorizationStatus();
            if (generation !== authorizationGeneration) return;
            authorization = status;
            if (status === 'authorized' || status === 'limited') beginCatalogLoad();
        } catch (statusError) {
            if (generation !== authorizationGeneration) return;
            authorizationError = messageFrom(statusError);
        } finally {
            if (generation === authorizationGeneration) checking = false;
        }
    }

    async function requestAccess() {
        const generation = ++authorizationGeneration;
        requesting = true;
        authorizationError = null;
        try {
            const status = await client.requestAuthorization();
            if (generation !== authorizationGeneration) return;
            authorization = status;
            if (status === 'authorized' || status === 'limited') beginCatalogLoad();
        } catch (requestError) {
            if (generation !== authorizationGeneration) return;
            authorizationError = messageFrom(requestError);
        } finally {
            if (generation === authorizationGeneration) requesting = false;
        }
    }

    function changeAlbum(event: Event) {
        selectedAlbumId = (event.currentTarget as HTMLSelectElement).value;
        resetAssets();
        void loadAssets(selectedAlbumId || null);
    }

    function selectAlbum(albumId: string) {
        selectedAlbumId = albumId;
        resetAssets();
        void loadAssets(albumId || null);
    }

    function resetAssets() {
        assets = [];
        assetsTotal = 0;
        assetsHasMore = false;
        assetsNextOffset = 0;
        assetsError = null;
        previews = {};
        previewOrder = [];
        previewGeneration += 1;
        selectedAssetIds = new Set();
        importError = null;
    }

    function changeFilter(event: Event) {
        assetFilter = (event.currentTarget as HTMLSelectElement).value as ApplePhotosAssetFilter;
        resetAssets();
        void loadAssets(selectedAlbumId || null);
    }

    function changeSort(event: Event) {
        assetSort = (event.currentTarget as HTMLSelectElement).value as ApplePhotosAssetSort;
        resetAssets();
        void loadAssets(selectedAlbumId || null);
    }

    function loadMore() {
        if (assetsLoading || !assetsHasMore) return;
        void loadAssets(selectedAlbumId || null, assetsNextOffset, true);
    }

    function handleAssetScroll(event: Event) {
        const list = event.currentTarget as HTMLElement;
        if (list.scrollHeight - list.scrollTop - list.clientHeight <= 240) loadMore();
    }

    function retryAssets() {
        if (assetsLoading) return;
        void loadAssets(selectedAlbumId || null, assets.length === 0 ? 0 : assetsNextOffset, assets.length > 0);
    }

    function toggleSelection(assetId: string) {
        const next = new Set(selectedAssetIds);
        if (next.has(assetId)) next.delete(assetId);
        else next.add(assetId);
        selectedAssetIds = next;
        importError = null;
    }

    async function startImport() {
        if (importing || selectedAssetIds.size === 0) return;
        const frozenAssetIds = [...selectedAssetIds];
        importing = true;
        importError = null;
        try {
            await client.startImport(frozenAssetIds, selectedAlbumId || null);
            onclose();
        } catch (startError) {
            importError = messageFrom(startError);
        } finally {
            importing = false;
        }
    }

    function handleDialogKeydown(event: KeyboardEvent) {
        if (event.defaultPrevented || event.key !== 'Enter' || importing || selectedAssetIds.size === 0) return;
        const target = event.target;
        if (target instanceof HTMLSelectElement) return;
        if (target instanceof HTMLInputElement && target.type === 'range') return;
        if (target instanceof HTMLButtonElement && !target.classList.contains('asset-tile')) return;
        event.preventDefault();
        event.stopPropagation();
        void startImport();
    }

    function observePreview(node: HTMLElement, asset: ApplePhotosAsset) {
        if (typeof IntersectionObserver === 'undefined') return {};
        previewAssets.set(node, asset);
        previewObserver ??= new IntersectionObserver(entries => {
            for (const entry of entries) {
                if (!entry.isIntersecting) continue;
                const visibleAsset = previewAssets.get(entry.target);
                if (!visibleAsset || previews[visibleAsset.id] !== undefined) continue;
                requestPreview(visibleAsset.id);
            }
        }, { root: node.closest('.asset-list'), rootMargin: '240px' });
        previewObserver.observe(node);
        return { destroy: () => previewObserver?.unobserve(node) };
    }

    function requestPreview(assetId: string) {
        const generation = previewGeneration;
        let request = previewRequests.get(assetId);
        if (!request) {
            request = client.loadPreview(assetId, PREVIEW_REQUEST_SIZE).catch(() => null);
            previewRequests.set(assetId, request);
            void request.finally(() => {
                if (previewRequests.get(assetId) === request) previewRequests.delete(assetId);
            });
        }
        void request.then(preview => cachePreview(assetId, preview, generation));
    }

    function cachePreview(assetId: string, preview: string | null, generation: number) {
        if (generation !== previewGeneration) return;
        previews[assetId] = preview;
        previewOrder = [...previewOrder.filter(id => id !== assetId), assetId];
        while (previewOrder.length > MAX_CACHED_PREVIEWS) {
            const evictedId = previewOrder.shift();
            if (evictedId) delete previews[evictedId];
        }
    }

    function observePagination(node: HTMLElement, _state: { offset: number; hasMore: boolean }) {
        if (typeof IntersectionObserver === 'undefined') return {};
        const observer = new IntersectionObserver(entries => {
            if (entries.some(entry => entry.isIntersecting)) loadMore();
        }, { root: node.closest('.asset-list'), rootMargin: '240px' });
        const observeAgain = () => {
            observer.disconnect();
            observer.observe(node);
        };
        observeAgain();
        return {
            update: (_nextState: { offset: number; hasMore: boolean }) => {
                observeAgain();
            },
            destroy: () => observer.disconnect(),
        };
    }

    function observeAssetListSize(node: HTMLElement) {
        const updateWidth = (width: number) => { assetGridWidth = Math.max(0, width); };
        updateWidth(node.clientWidth - 32);
        const sizeObserver = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(entries => {
            const entry = entries[0];
            if (entry) updateWidth(entry.contentRect.width);
        });
        sizeObserver?.observe(node);
        return {
            destroy: () => sizeObserver?.disconnect(),
        };
    }

    function groupIntrinsicHeight(itemCount: number): number {
        const minimumTileWidth = photoTileSize;
        const columns = Math.max(1, Math.floor((assetGridWidth + 8) / (minimumTileWidth + 8)));
        const tileWidth = (assetGridWidth - (columns - 1) * 8) / columns;
        const rows = Math.ceil(itemCount / columns);
        return Math.ceil(28 + rows * tileWidth + Math.max(0, rows - 1) * 8);
    }

    function setPreviewScale(position: number) {
        previewScalePosition = position;
    }

    function stepPreviewScale(direction: -1 | 1) {
        const nextSize = nudgeThumbnailSize(
            photoTileSize,
            direction,
            PREVIEW_SIZE_MIN,
            PREVIEW_SIZE_MAX,
        );
        previewScalePosition = zoomPositionFromThumbnailSize(
            nextSize,
            PREVIEW_SIZE_MIN,
            PREVIEW_SIZE_MAX,
        );
    }

    function dateLabel(createdAt: string | null): string {
        if (!createdAt) return 'Date unknown';
        return new Date(createdAt).toLocaleDateString(undefined, { dateStyle: 'long' });
    }

    function groupedAssets(items: ApplePhotosAsset[]) {
        const groups: { label: string; items: ApplePhotosAsset[] }[] = [];
        for (const asset of items) {
            const label = dateLabel(asset.created_at);
            const latest = groups.at(-1);
            if (latest?.label === label) latest.items.push(asset);
            else groups.push({ label, items: [asset] });
        }
        return groups;
    }

    function loadMoreAlbums() {
        if (albumsLoading || !albumsHasMore) return;
        void loadAlbums(albums.length, true);
    }

    onMount(() => {
        void checkAuthorization();
    });

    onDestroy(() => {
        authorizationGeneration += 1;
        albumsGeneration += 1;
        assetsGeneration += 1;
        previewGeneration += 1;
        previewObserver?.disconnect();
        previewObserver = null;
    });

    const smartAlbums = $derived(albums.filter(album => album.kind === 'smart'));
    const userAlbums = $derived(albums.filter(album => album.kind === 'user'));
    const favouriteAlbum = $derived(smartAlbums.find(album => album.role === 'favorites'));
    const screenshotsAlbum = $derived(smartAlbums.find(album => album.role === 'screenshots'));
    const selectedAlbumTitle = $derived(
        selectedAlbumId === ''
            ? 'All Photos'
            : albums.find(album => album.id === selectedAlbumId)?.title ?? 'Untitled album',
    );
    const assetGroups = $derived(groupedAssets(assets));
    const photoTileSize = $derived(
        thumbnailSizeFromZoomPosition(previewScalePosition, PREVIEW_SIZE_MIN, PREVIEW_SIZE_MAX),
    );
</script>

<ModalDialog
    titleId="apple-photos-title"
    descriptionId="apple-photos-description"
    {onclose}
    panelClass="dialog apple-photos-dialog"
    overlayClass="apple-photos-overlay"
    initialFocus=".apple-photos-close"
    onkeydown={handleDialogKeydown}
>
    <header class="dialog-header">
        <div>
            <h2 id="apple-photos-title">Apple Photos</h2>
            <p id="apple-photos-description">Browse your System Photo Library.</p>
        </div>
        <button class="close-btn apple-photos-close" onclick={onclose} aria-label="Close Apple Photos">Close</button>
    </header>

    <div class="dialog-content">
        {#if checking}
            <div class="state" role="status">Checking Photos access…</div>
        {:else if authorizationError && !canBrowse}
            <div class="state error" role="alert">{authorizationError}</div>
        {:else if authorization === 'not_determined'}
            <section class="permission-state">
                <h3>Allow access when you are ready</h3>
                <p>Cull needs permission before it can browse your Photos library.</p>
                <button class="btn primary" onclick={requestAccess} disabled={requesting}>
                    {requesting ? 'Requesting…' : 'Allow Photos Access'}
                </button>
            </section>
        {:else if authorization === 'denied' || authorization === 'restricted'}
            <section class="permission-state">
                <h3>Photos access is unavailable</h3>
                <p>Enable Cull in System Settings → Privacy &amp; Security → Photos, then reopen this dialog.</p>
            </section>
        {:else if authorization === 'unsupported'}
            <section class="permission-state">
                <h3>Apple Photos is available on macOS</h3>
                <p>This catalog source is not supported on the current platform.</p>
            </section>
        {:else if canBrowse}
            <div class="catalog-shell">
                <aside class="catalog-sidebar" aria-label="Apple Photos library">
                    <nav aria-label="Library views">
                        <button class:active={selectedAlbumId === ''} aria-current={selectedAlbumId === '' ? 'page' : undefined} onclick={() => selectAlbum('')}>All Photos</button>
                        {#if favouriteAlbum}
                            <button class:active={selectedAlbumId === favouriteAlbum.id} aria-current={selectedAlbumId === favouriteAlbum.id ? 'page' : undefined} onclick={() => selectAlbum(favouriteAlbum.id)}>Favourites</button>
                        {/if}
                        {#if screenshotsAlbum}
                            <button class:active={selectedAlbumId === screenshotsAlbum.id} aria-current={selectedAlbumId === screenshotsAlbum.id ? 'page' : undefined} onclick={() => selectAlbum(screenshotsAlbum.id)}>Screenshots</button>
                        {/if}
                    </nav>

                    {#if smartAlbums.some(album => album.id !== favouriteAlbum?.id && album.id !== screenshotsAlbum?.id)}
                        <section>
                            <h3>Smart albums</h3>
                            {#each smartAlbums.filter(album => album.id !== favouriteAlbum?.id && album.id !== screenshotsAlbum?.id) as album (album.id)}
                                <button class:active={selectedAlbumId === album.id} aria-current={selectedAlbumId === album.id ? 'page' : undefined} onclick={() => selectAlbum(album.id)}>{album.title ?? 'Untitled album'}</button>
                            {/each}
                        </section>
                    {/if}

                    {#if userAlbums.length > 0}
                        <section>
                            <h3>Albums</h3>
                            {#each userAlbums as album (album.id)}
                                <button class:active={selectedAlbumId === album.id} aria-current={selectedAlbumId === album.id ? 'page' : undefined} onclick={() => selectAlbum(album.id)}>{album.title ?? 'Untitled album'}</button>
                            {/each}
                        </section>
                    {/if}

                    {#if albumsHasMore}
                        <button class="load-albums" onclick={loadMoreAlbums} disabled={albumsLoading}>
                            {albumsLoading ? 'Loading albums…' : 'Load more albums'}
                        </button>
                    {/if}
                </aside>

                <main class="catalog-main">
                    <div class="catalog-top">
                        {#if authorization === 'limited'}
                            <p class="limited-note" role="status">Showing the photos you allowed Cull to access.</p>
                        {/if}
                        {#if albumsError}<div class="catalog-error" role="alert">Albums: {albumsError}</div>{/if}

                        <div class="catalog-toolbar">
                            <h3>{selectedAlbumTitle}</h3>
                            <label class="mobile-album-picker">
                                <span>Album</span>
                                <select onchange={changeAlbum} value={selectedAlbumId} disabled={albumsLoading}>
                                    <option value="">All Photos</option>
                                    {#each albums as album (album.id)}
                                        <option value={album.id}>{album.title ?? 'Untitled album'}</option>
                                    {/each}
                                </select>
                            </label>
                            <ThumbnailScaleControl
                                position={previewScalePosition}
                                size={photoTileSize}
                                minSize={PREVIEW_SIZE_MIN}
                                maxSize={PREVIEW_SIZE_MAX}
                                groupLabel="Photo preview scale"
                                sliderLabel="Preview size"
                                outLabel="Zoom photo previews out"
                                inLabel="Zoom photo previews in"
                                onposition={setPreviewScale}
                                onstep={stepPreviewScale}
                            />
                            <div class="toolbar-actions">
                                <label>
                                    <span class="sr-only">Filter photos</span>
                                    <select aria-label="Filter photos" value={assetFilter} onchange={changeFilter}>
                                        <option value="all">All</option>
                                        <option value="favorites">Favourites</option>
                                    </select>
                                </label>
                                <label>
                                    <span class="sr-only">Sort photos</span>
                                    <select aria-label="Sort photos" value={assetSort} onchange={changeSort}>
                                        <option value="newest">Newest</option>
                                        <option value="oldest">Oldest</option>
                                    </select>
                                </label>
                            </div>
                            <span class="catalog-count">{assets.length} of {assetsTotal}</span>
                        </div>
                    </div>

                    <div
                        class="asset-list"
                        role="list"
                        aria-label="Apple Photos assets"
                        aria-busy={assetsLoading}
                        onscroll={handleAssetScroll}
                        use:observeAssetListSize
                    >
                        {#each assetGroups as group (group.label)}
                            <section
                                class="date-group"
                                class:contained={assetGridWidth > 0}
                                style:contain-intrinsic-size={assetGridWidth > 0 ? `${groupIntrinsicHeight(group.items.length)}px` : undefined}
                                aria-labelledby={`date-${group.label.replace(/[^a-z0-9]/gi, '-')}`}
                            >
                                <h3 id={`date-${group.label.replace(/[^a-z0-9]/gi, '-')}`}>{group.label}</h3>
                                <div class="asset-grid" style:--photo-tile-size={`${photoTileSize}px`}>
                                    {#each group.items as asset (asset.id)}
                                        <div role="listitem">
                                            <button
                                                class="asset-tile"
                                                class:selected={selectedAssetIds.has(asset.id)}
                                                type="button"
                                                aria-label={asset.filename ? `Select ${asset.filename}` : 'Select photo'}
                                                aria-pressed={selectedAssetIds.has(asset.id)}
                                                onclick={() => toggleSelection(asset.id)}
                                                use:observePreview={asset}
                                            >
                                                {#if previews[asset.id]}
                                                    <img src={previews[asset.id] ?? ''} alt="" />
                                                {:else}
                                                    <span class="preview-placeholder" aria-label="Preview unavailable locally">☁</span>
                                                {/if}
                                                {#if asset.favorite}<span class="favourite" aria-label="Favourite">★</span>{/if}
                                                {#if selectedAssetIds.has(asset.id)}<span class="selection" aria-hidden="true">✓</span>{/if}
                                            </button>
                                        </div>
                                    {/each}
                                </div>
                            </section>
                        {:else}
                            {#if assetsLoading}
                                <div class="state" role="status">Loading photo metadata…</div>
                            {:else if assetsError}
                                <div class="state error" role="alert">Photos: {assetsError}</div>
                            {:else}
                                <div class="state">No still images are visible in this album.</div>
                            {/if}
                        {/each}

                        <div class="pagination-state" role="status" aria-live="polite">
                            {#if assets.length > 0 && assetsLoading}
                                Loading more photos…
                            {:else if assets.length > 0 && assetsError}
                                <span>More photos could not be loaded.</span>
                                <button onclick={retryAssets}>Retry</button>
                            {:else if assets.length > 0 && !assetsHasMore}
                                {#if assets.length < assetsTotal}
                                    End of library reached · {assets.length} unique photos shown.
                                {:else}
                                    All photos loaded.
                                {/if}
                            {/if}
                        </div>
                        <div
                            class="pagination-sentinel"
                            aria-hidden="true"
                            use:observePagination={{ offset: assetsNextOffset, hasMore: assetsHasMore }}
                        ></div>
                    </div>

                    <footer>
                        <span>Local previews when available · iCloud downloads enabled for selected imports</span>
                        <div class="import-actions">
                            {#if importError}<span class="import-error" role="alert">{importError}</span>{/if}
                            <button
                                class="import-btn"
                                type="button"
                                disabled={selectedAssetIds.size === 0 || importing}
                                aria-label={importing
                                    ? 'Starting import…'
                                    : `Import ${selectedAssetIds.size} ${selectedAssetIds.size === 1 ? 'photo' : 'photos'}`}
                                onclick={startImport}
                            >
                                {importing
                                    ? 'Starting import…'
                                    : `Import ${selectedAssetIds.size} ${selectedAssetIds.size === 1 ? 'photo' : 'photos'}`}
                            </button>
                        </div>
                    </footer>
                </main>
            </div>
        {/if}
    </div>
</ModalDialog>

<style>
    :global(.apple-photos-overlay) {
        background: color-mix(in srgb, var(--bg) 82%, transparent);
        --modal-align-items: flex-start;
        padding: var(--macos-titlebar-safe-area) 12px 12px;
    }

    :global(.dialog.apple-photos-dialog) {
        width: 100%;
        max-width: none;
        height: calc(100vh - var(--macos-titlebar-safe-area) - 12px);
        max-height: none;
        display: grid;
        grid-template-rows: auto minmax(0, 1fr);
        overflow: hidden;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
    }

    .dialog-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 16px;
        padding: 16px;
        border-bottom: 1px solid var(--border);
    }

    h2, h3, p { margin: 0; }
    h2 { font-size: 16px; }
    h3 { font-size: 14px; }
    .dialog-header p, .permission-state p { margin-top: 8px; color: var(--text-secondary); }

    .close-btn {
        padding: 6px 10px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text-secondary);
        font: inherit;
        font-size: 12px;
        cursor: pointer;
    }

    .dialog-content {
        min-height: 0;
        display: block;
        overflow: hidden;
    }

    .permission-state, .state {
        margin: auto;
        max-width: 520px;
        text-align: center;
    }

    .permission-state .btn { margin-top: 16px; }
    .error, .catalog-error { color: var(--red); }
    .limited-note { padding: 8px 16px; color: var(--orange); border-bottom: 1px solid var(--border); }

    .catalog-shell {
        height: 100%;
        min-height: 0;
        display: grid;
        grid-template-columns: 208px minmax(0, 1fr);
    }

    .catalog-sidebar {
        min-height: 0;
        padding: 12px 8px;
        overflow-y: auto;
        border-right: 1px solid var(--border);
        background: var(--bg);
    }

    .catalog-sidebar nav, .catalog-sidebar section { display: grid; gap: 2px; }
    .catalog-sidebar section { margin-top: 18px; }
    .catalog-sidebar h3 {
        padding: 0 8px 6px;
        color: var(--text-secondary);
        font-size: 10px;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }
    .catalog-sidebar button {
        min-width: 0;
        padding: 7px 8px;
        overflow: hidden;
        border: 0;
        border-radius: var(--radius);
        background: transparent;
        color: var(--text-secondary);
        font: inherit;
        text-align: left;
        text-overflow: ellipsis;
        white-space: nowrap;
        cursor: pointer;
    }
    .catalog-sidebar button:hover, .catalog-sidebar button.active {
        color: var(--text);
        background: var(--surface);
    }
    .catalog-sidebar .load-albums { margin-top: 16px; color: var(--blue); }

    .catalog-main {
        min-width: 0;
        min-height: 0;
        display: grid;
        grid-template-rows: auto minmax(0, 1fr) auto;
    }

    .catalog-toolbar {
        min-height: 48px;
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 8px 16px;
        border-bottom: 1px solid var(--border);
    }
    .catalog-toolbar > h3 { margin-right: auto; color: var(--text); }
    .toolbar-actions { display: flex; align-items: center; gap: 8px; }
    .toolbar-actions select, .pagination-state button {
        padding: 6px 10px;
        color: var(--text-secondary);
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        font: inherit;
    }
    .catalog-count { color: var(--text-secondary); font-size: 11px; }
    .mobile-album-picker { display: none; }

    select {
        min-width: 0;
        padding: 8px;
        color: var(--text);
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        font: inherit;
    }

    .asset-list {
        min-height: 0;
        overflow: auto;
        padding: 16px;
        scrollbar-gutter: stable;
    }

    .date-group.contained { content-visibility: auto; }
    .date-group + .date-group { margin-top: 24px; }
    .date-group > h3 { margin-bottom: 10px; color: var(--text-secondary); font-size: 11px; font-weight: 500; }
    .asset-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(var(--photo-tile-size), 1fr));
        gap: 8px;
    }
    .asset-grid > [role='listitem'] { min-width: 0; }

    .asset-tile {
        position: relative;
        min-width: 0;
        aspect-ratio: 1;
        padding: 0;
        overflow: hidden;
        border: 2px solid transparent;
        border-radius: var(--radius);
        background: var(--bg);
        cursor: pointer;
        width: 100%;
    }
    .asset-tile.selected { border-color: var(--blue); }
    .asset-tile img { width: 100%; height: 100%; display: block; object-fit: cover; }
    .preview-placeholder {
        width: 100%; height: 100%; display: grid; place-items: center;
        color: var(--text-secondary);
        background: color-mix(in srgb, var(--surface) 86%, var(--border));
        font-size: 18px;
    }
    .favourite, .selection {
        position: absolute;
        top: 7px;
        color: var(--text);
        filter: drop-shadow(0 1px 2px var(--bg));
    }
    .favourite { left: 8px; }
    .selection {
        right: 7px;
        width: 20px;
        height: 20px;
        display: grid;
        place-items: center;
        border-radius: 50%;
        background: var(--blue);
        color: var(--bg);
        font-weight: 700;
    }
    .pagination-state {
        min-height: 44px;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 10px;
        color: var(--text-secondary);
        font-size: 11px;
    }
    .pagination-sentinel { height: 1px; }
    footer {
        padding: 10px 16px;
        border-top: 1px solid var(--border);
        color: var(--text-secondary);
        font-size: 10px;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
    }
    .import-actions { display: flex; align-items: center; gap: 12px; }
    .import-error { color: var(--red); }
    .import-btn {
        min-width: 132px;
        padding: 7px 14px;
        border: 1px solid var(--green);
        border-radius: var(--radius);
        background: color-mix(in srgb, var(--green) 16%, var(--surface));
        color: var(--green);
        font: inherit;
        cursor: pointer;
    }
    .import-btn:hover:not(:disabled) { background: color-mix(in srgb, var(--green) 24%, var(--surface)); }
    .import-btn:disabled { cursor: default; opacity: 0.45; }
    .sr-only {
        position: absolute;
        width: 1px;
        height: 1px;
        padding: 0;
        margin: -1px;
        overflow: hidden;
        clip: rect(0, 0, 0, 0);
        white-space: nowrap;
        border: 0;
    }

    @media (max-width: 720px) {
        .catalog-shell { grid-template-columns: 1fr; }
        .catalog-sidebar { display: none; }
        .mobile-album-picker { display: flex; align-items: center; gap: 8px; }
    }
</style>
