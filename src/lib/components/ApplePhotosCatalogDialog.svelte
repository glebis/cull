<script lang="ts">
    import { onDestroy, onMount } from 'svelte';
    import ModalDialog from '$lib/components/ModalDialog.svelte';
    import {
        tauriApplePhotosCatalogClient,
        type ApplePhotosAlbum,
        type ApplePhotosAsset,
        type ApplePhotosAuthorization,
        type ApplePhotosCatalogClient,
    } from '$lib/apple-photos';

    interface Props {
        onclose: () => void;
        client?: ApplePhotosCatalogClient;
    }

    let { onclose, client = tauriApplePhotosCatalogClient }: Props = $props();

    const PAGE_SIZE = 100;
    let authorization = $state<ApplePhotosAuthorization | null>(null);
    let albums = $state<ApplePhotosAlbum[]>([]);
    let albumsHasMore = $state(false);
    let selectedAlbumId = $state('');
    let assets = $state<ApplePhotosAsset[]>([]);
    let assetsTotal = $state(0);
    let assetsHasMore = $state(false);
    let checking = $state(true);
    let requesting = $state(false);
    let albumsLoading = $state(false);
    let assetsLoading = $state(false);
    let authorizationError = $state<string | null>(null);
    let albumsError = $state<string | null>(null);
    let assetsError = $state<string | null>(null);
    let authorizationGeneration = 0;
    let albumsGeneration = 0;
    let assetsGeneration = 0;

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
            const page = await client.listAssets(albumId, offset, PAGE_SIZE);
            if (generation !== assetsGeneration) return;
            assets = append ? [...assets, ...page.items] : page.items;
            assetsTotal = page.total;
            assetsHasMore = page.has_more;
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
        assets = [];
        assetsTotal = 0;
        assetsHasMore = false;
        void loadAssets(selectedAlbumId || null);
    }

    function loadMore() {
        if (assetsLoading || !assetsHasMore) return;
        void loadAssets(selectedAlbumId || null, assets.length, true);
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
    });
</script>

<ModalDialog
    titleId="apple-photos-title"
    descriptionId="apple-photos-description"
    {onclose}
    panelClass="dialog apple-photos-dialog"
    overlayClass="apple-photos-overlay"
    initialFocus=".apple-photos-close"
>
    <header class="dialog-header">
        <div>
            <h2 id="apple-photos-title">Apple Photos</h2>
            <p id="apple-photos-description">Browse still-image metadata from your System Photo Library.</p>
        </div>
        <button class="close-btn apple-photos-close" onclick={onclose} aria-label="Close Apple Photos">×</button>
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
            {#if authorization === 'limited'}
                <p class="limited-note" role="status">Showing the photos you allowed Cull to access.</p>
            {/if}

            <label class="album-picker">
                <span>Album</span>
                <select onchange={changeAlbum} value={selectedAlbumId} disabled={albumsLoading}>
                    <option value="">All Photos</option>
                    {#each albums as album (album.id)}
                        <option value={album.id}>{album.title ?? 'Untitled album'}</option>
                    {/each}
                </select>
            </label>

            {#if albumsHasMore}
                <button class="btn" onclick={loadMoreAlbums} disabled={albumsLoading}>
                    {albumsLoading ? 'Loading albums…' : 'Load more albums'}
                </button>
            {/if}

            {#if albumsError}
                <div class="catalog-error" role="alert">Albums: {albumsError}</div>
            {/if}
            {#if assetsError}
                <div class="catalog-error" role="alert">Photos: {assetsError}</div>
            {/if}

            <div class="catalog-summary">
                <span>{assets.length} of {assetsTotal} still images</span>
                <span>Metadata only · no downloads</span>
            </div>

            <div class="asset-list" aria-busy={assetsLoading && assets.length === 0}>
                {#each assets as asset (asset.id)}
                    <article class="asset-row">
                        <div class="asset-name">{asset.filename ?? 'Untitled image'}</div>
                        <div class="asset-meta">
                            <span>{asset.pixel_width} × {asset.pixel_height}</span>
                            {#if asset.created_at}<span>{new Date(asset.created_at).toLocaleDateString()}</span>{/if}
                            {#if asset.favorite}<span>Favourite</span>{/if}
                        </div>
                    </article>
                {:else}
                    {#if assetsLoading}
                        <div class="state" role="status">Loading photo metadata…</div>
                    {:else}
                        <div class="state">No still images are visible in this album.</div>
                    {/if}
                {/each}
            </div>

            {#if assetsHasMore}
                <button class="btn" onclick={loadMore} disabled={assetsLoading}>
                    {assetsLoading ? 'Loading…' : 'Load more'}
                </button>
            {/if}
        {/if}
    </div>
</ModalDialog>

<style>
    :global(.apple-photos-overlay) {
        background: color-mix(in srgb, var(--bg) 82%, transparent);
    }

    :global(.apple-photos-dialog) {
        width: min(760px, calc(100vw - 32px));
        height: min(90vh, 840px);
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
        border: 0;
        background: transparent;
        color: var(--text-secondary);
        font: inherit;
        font-size: 20px;
        cursor: pointer;
    }

    .dialog-content {
        min-height: 0;
        display: flex;
        flex-direction: column;
        gap: 16px;
        padding: 16px;
        overflow: hidden;
    }

    .permission-state, .state {
        margin: auto;
        max-width: 520px;
        text-align: center;
    }

    .permission-state .btn { margin-top: 16px; }
    .error, .catalog-error { color: var(--red); }
    .limited-note { color: var(--orange); }

    .album-picker {
        display: grid;
        grid-template-columns: auto minmax(0, 1fr);
        align-items: center;
        gap: 16px;
    }

    select {
        min-width: 0;
        padding: 8px;
        color: var(--text);
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        font: inherit;
    }

    .catalog-summary, .asset-meta {
        display: flex;
        justify-content: space-between;
        gap: 16px;
        color: var(--text-secondary);
        font-size: 12px;
    }

    .asset-list {
        min-height: 0;
        flex: 1;
        overflow: auto;
        border: 1px solid var(--border);
    }

    .asset-row {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        align-items: center;
        gap: 16px;
        min-height: 48px;
        padding: 8px 16px;
        border-bottom: 1px solid var(--border);
    }

    .asset-row:last-child { border-bottom: 0; }
    .asset-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .asset-meta { justify-content: flex-end; }
</style>
