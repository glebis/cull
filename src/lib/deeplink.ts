import { onOpenUrl, getCurrent } from '@tauri-apps/plugin-deep-link';
import { listen } from '@tauri-apps/api/event';
import { get } from 'svelte/store';
import {
    viewMode,
    thumbnailSize,
    focusedIndex,
    images,
    gridGap,
    loupeScale,
    activeFolder,
    folders,
    windowName,
    windowLabel,
    navigateTo,
    showToast,
    importBatchFilter,
    importBatchImageIds,
    pinnedCollection,
    activeCollection,
    activeSmartCollection,
    activeDetectedClass,
    collections,
    type ViewMode,
} from './stores';
import { importFolder, importFiles, addToCollection, listCollections, getBatchImages, listFolders } from './api';
import { clearImageScope, loadAllImages, loadImagesForCurrentScope, loadImagesUntil, resetImagePaging } from './image-loading';

interface OpenParams {
    path?: string | null;
    paths?: string[] | null;
    folder?: string | null;
    view?: string | null;
    size?: number | null;
    zoom?: number | null;
    fullscreen?: boolean | null;
    focus?: number | null;
    gap?: number | null;
}

const VALID_VIEWS: ViewMode[] = ['grid', 'compare', 'loupe', 'canvas', 'lineage', 'embeddings', 'export'];

export async function handleParams(params: OpenParams) {
    console.log('[deep-link] handleParams called:', JSON.stringify(params));
    // Set view mode
    if (params.view && VALID_VIEWS.includes(params.view as ViewMode)) {
        navigateTo(params.view as ViewMode);
    }

    // Set thumbnail size
    if (params.size != null) {
        thumbnailSize.set(params.size);
    }

    // Set grid gap
    if (params.gap != null) {
        gridGap.set(params.gap);
    }

    // Set zoom / loupe scale
    if (params.zoom != null) {
        loupeScale.set(params.zoom / 100);
    }

    // Handle folder import
    if (params.folder) {
        try {
            const result = await importFolder(params.folder);
            activeSmartCollection.set(null);
            activeCollection.set(null);
            activeDetectedClass.set(null);
            activeFolder.set(params.folder);
            const folderName = params.folder.split('/').filter(Boolean).pop() ?? params.folder;
            await loadImagesForCurrentScope();
            focusedIndex.set(0);
            // Refresh folder list in sidebar
            const f = await listFolders();
            folders.set(f);
            const folderTotal = f.find(([path]) => path === params.folder)?.[1] ?? result.imported;
            if (result.imported > 0) {
                showToast(`Imported "${folderName}"`, {
                    detail: `+${result.imported} new, ${result.skipped} skipped · ${folderTotal} total in folder`,
                    type: 'success',
                    duration: 8000,
                });
            }
        } catch (e) {
            console.error('Deep link: failed to import folder', e);
            showToast('Import failed', { detail: String(e), type: 'error', duration: 10000 });
        }
    }

    // Handle single path import
    if (params.path) {
        try {
            const result = await importFiles([params.path]);
            const pinned = get(pinnedCollection);

            if (pinned && result.image_ids.length > 0) {
                await addToCollection(pinned, result.image_ids);
                const c = await listCollections();
                collections.set(c);
                showToast(`Image added to active collection`, { type: 'success', duration: 5000 });
            }

            if (pinned && get(activeCollection) === pinned) {
                await loadImagesForCurrentScope();
            } else {
                await loadAllImages();
            }
            const firstId = result.image_ids[0];
            if (firstId) {
                const idx = await loadImagesUntil((img) => img.image.id === firstId);
                if (idx >= 0) focusedIndex.set(idx);
            }
        } catch (e) {
            console.error('Deep link: failed to import path', e);
        }
    }

    // Handle multiple paths
    if (params.paths && params.paths.length > 0) {
        try {
            const result = await importFiles(params.paths);
            const pinned = get(pinnedCollection);

            if (pinned && result.image_ids.length > 0) {
                // Active collection exists — append silently
                await addToCollection(pinned, result.image_ids);
                const c = await listCollections();
                collections.set(c);

                activeCollection.set(pinned);
                activeSmartCollection.set(null);
                activeDetectedClass.set(null);
                activeFolder.set(null);
                await loadImagesForCurrentScope();
                const firstId = result.image_ids[0];
                if (firstId) {
                    const idx = await loadImagesUntil((img) => img.image.id === firstId);
                    focusedIndex.set(idx >= 0 ? idx : 0);
                } else {
                    focusedIndex.set(0);
                }

                const collName = c.find(([id]) => id === pinned)?.[1] ?? 'collection';
                showToast(`${result.imported} images added to "${collName}"`, {
                    type: 'success',
                    duration: 8000,
                });
            } else if (result.batch_id) {
                // No active collection — filter to batch
                const batchImgs = await getBatchImages(result.batch_id);
                clearImageScope();
                resetImagePaging();
                images.set(batchImgs);
                importBatchFilter.set(result.batch_id);
                importBatchImageIds.set(result.image_ids);
                focusedIndex.set(0);
            } else {
                await loadAllImages();
                focusedIndex.set(0);
            }
        } catch (e) {
            console.error('Deep link: failed to import paths', e);
        }
    }

    // Handle focus index
    if (params.focus != null) {
        focusedIndex.set(params.focus);
    }

    // Handle fullscreen
    if (params.fullscreen) {
        try {
            await document.documentElement.requestFullscreen();
        } catch (e) {
            console.error('Deep link: fullscreen request failed', e);
        }
    }
}

export function parseDeepLinkUrl(url: string): OpenParams {
    try {
        const parsed = new URL(url);
        const p = parsed.searchParams;

        const pathsRaw = p.get('paths');
        return {
            path: p.get('path'),
            paths: pathsRaw ? pathsRaw.split(',') : null,
            folder: p.get('folder'),
            view: p.get('view') ?? inferViewFromAction(parsed.hostname || parsed.pathname.replace(/^\/+/, '')),
            size: p.has('size') ? parseInt(p.get('size')!) : null,
            zoom: p.has('zoom') ? parseInt(p.get('zoom')!) : null,
            fullscreen: p.get('fullscreen') === 'true',
            focus: p.has('focus') ? parseInt(p.get('focus')!) : null,
            gap: p.has('gap') ? parseInt(p.get('gap')!) : null,
        };
    } catch (e) {
        console.error('Deep link: failed to parse URL', url, e);
        return {};
    }
}

export function inferViewFromAction(action: string): string | null {
    if (['grid', 'loupe', 'compare'].includes(action)) {
        return action;
    }
    return null;
}

let lastHandledKey = '';
let lastHandledAt = 0;

function deduplicatedHandleParams(params: OpenParams, source: string) {
    const key = JSON.stringify(params);
    const now = Date.now();
    if (key === lastHandledKey && now - lastHandledAt < 5000) {
        console.log(`[deep-link] Skipping duplicate from ${source}`);
        return;
    }
    lastHandledKey = key;
    lastHandledAt = now;
    console.log(`[deep-link] Handling from ${source}:`, key);
    handleParams(params);
}

export async function initDeepLink() {
    // Listen for window name assignment from Rust
    await listen<{ label: string; name: string }>('set-window-name', (event) => {
        windowLabel.set(event.payload.label);
        windowName.set(event.payload.name);
    });

    // Listen for open-with-params events (from Rust deep link handler + open_with_params command)
    await listen<OpenParams>('open-with-params', (event) => {
        deduplicatedHandleParams(event.payload, 'open-with-params');
    });

    // Listen for deep link URLs opened while app is running (macOS)
    try {
        await onOpenUrl((urls) => {
            for (const url of urls) {
                const params = parseDeepLinkUrl(url);
                deduplicatedHandleParams(params, 'onOpenUrl');
            }
        });
    } catch (e) {
        console.warn('Deep link: onOpenUrl not available', e);
    }

    // Check if app was launched via deep link
    try {
        const current = await getCurrent();
        if (current && current.length > 0) {
            for (const url of current) {
                const params = parseDeepLinkUrl(url);
                deduplicatedHandleParams(params, 'getCurrent');
            }
        }
    } catch (e) {
        console.warn('Deep link: getCurrent not available', e);
    }
}
