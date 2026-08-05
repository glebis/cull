import { listen } from '@tauri-apps/api/event';
import { getCurrent } from '@tauri-apps/plugin-deep-link';
import { get } from 'svelte/store';
import {
    viewMode,
    thumbnailSize,
    focusedIndex,
    focusedImageOverride,
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
    activeSession,
    collections,
    type ViewMode,
} from './stores';
import { importFolder, importFiles, addToCollection, listCollections, getBatchImages, listFolders, listImagesByFolder, getImagesByIds, getImageByPath, drainPendingOpenParams, openDeepLinkUrls, completeDeepLinkNavigation, type ImageWithFile, type ImportResponse } from './api';
import { applyClipboardMonitorCollection } from './clipboard-monitor';
import { clearImageScope, invalidateImageCache, loadAllImages, loadImagesForCurrentScope, loadImagesUntil, resetImagePaging } from './image-loading';
import { openSettings, type SettingsTab } from './settings-navigation';

interface OpenParams {
    path?: string | null;
    paths?: string[] | null;
    folder?: string | null;
    settings_tab?: string | null;
    collection?: string | null;
    request_id?: string | null;
    view?: string | null;
    size?: number | null;
    zoom?: number | null;
    fullscreen?: boolean | null;
    focus?: number | string | null;
    image_id?: string | null;
    imageId?: string | null;
    gap?: number | null;
    drag_drop?: boolean | null;
    drop_x?: number | null;
    drop_y?: number | null;
}

const VALID_VIEWS: ViewMode[] = ['grid', 'compare', 'loupe', 'canvas', 'lineage', 'embeddings', 'export'];
const VALID_SETTINGS_TABS: SettingsTab[] = ['general', 'appearance', 'ai', 'agent-access', 'privacy', 'plugins'];
const FOLDER_IMAGE_PAGE_SIZE = 250;
const FOLDER_IMAGE_PAGE_LIMIT = 200;

interface CanvasImportDropDetail {
    images: ImageWithFile[];
    folder?: string | null;
    paths?: string[] | null;
    dropX: number;
    dropY: number;
    importResult: ImportResponse;
}

function focusIndex(index: number) {
    focusedImageOverride.set(null);
    focusedIndex.set(index);
}

function imageIdFromParams(params: OpenParams): string | null {
    if (params.image_id) return params.image_id;
    if (params.imageId) return params.imageId;
    if (typeof params.focus === 'string' && params.focus.trim() !== '') return params.focus;
    return null;
}

async function focusImageById(imageId: string): Promise<boolean> {
    const loaded = get(images) ?? [];
    const loadedIndex = loaded.findIndex(img => img.image.id === imageId);
    if (loadedIndex >= 0) {
        focusIndex(loadedIndex);
        return true;
    }

    const pagedIndex = await loadImagesUntil((img) => img.image.id === imageId);
    if (pagedIndex >= 0) {
        focusIndex(pagedIndex);
        return true;
    }

    try {
        const [img] = await getImagesByIds([imageId]);
        if (img) {
            focusedImageOverride.set(img);
            return true;
        }
    } catch (e) {
        console.error('Deep link: failed to load image by id', e);
    }

    showToast('Image not found', { detail: imageId, type: 'error', duration: 8000 });
    return false;
}

/**
 * Report the outcome of a navigation back to Rust. Display tools (show_image,
 * show_collection, navigate_to_folder) block on this ack, so every exit path of
 * handleParams must reach it — otherwise the tool reports a spurious timeout.
 * A failure to ack is logged and swallowed: it must never break the navigation
 * that already succeeded.
 */
async function ackNavigation(params: OpenParams, ok: boolean, error?: unknown): Promise<boolean> {
    if (!params.request_id) return true;
    const message = error == null ? null : (error instanceof Error ? error.message : String(error));
    try {
        await completeDeepLinkNavigation(params.request_id, ok, message);
        return true;
    } catch (e) {
        console.warn('[deep-link] Failed to ack navigation', e);
        return false;
    }
}

export async function handleParams(params: OpenParams) {
    let acked = false;
    // Only latch on a delivered ack. If the IPC call itself failed, a later
    // ack attempt should still get through rather than leaving the caller to
    // time out on a navigation that succeeded.
    const ack: AckFn = async (ok, error) => {
        if (acked) return;
        acked = await ackNavigation(params, ok, error);
    };

    try {
        const result = await handleParamsInner(params, ack);
        await ack(true);
        return result;
    } catch (e) {
        await ack(false, e);
        throw e;
    }
}

type AckFn = (ok: boolean, error?: unknown) => Promise<void>;

async function handleParamsInner(params: OpenParams, ack: AckFn) {
    console.log('[deep-link] handleParams called:', JSON.stringify(params));
    if (params.settings_tab) {
        const tab = VALID_SETTINGS_TABS.includes(params.settings_tab as SettingsTab)
            ? params.settings_tab as SettingsTab
            : 'general';
        openSettings(tab);
    }
    if (params.drag_drop && get(viewMode) === 'canvas' && (params.folder || params.path || (params.paths && params.paths.length > 0))) {
        await handleCanvasDropImport(params);
        return;
    }

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

    // Handle collection navigation. Comes through the same buffered
    // `open-with-params` channel, so a collection shown while the webview is not
    // yet listening is replayed on init instead of being lost.
    if (params.collection) {
        await applyClipboardMonitorCollection(params.collection);
        return;
    }

    // Handle folder import
    if (params.folder) {
        try {
            // Switch scope to the target folder FIRST, then ack, then import.
            // The ack must mean "the app is now on this folder", which is only
            // true once activeFolder is set. Acking after importFolder instead
            // would make the caller wait on unbounded scan/thumbnail/detection
            // work that routinely outlasts its timeout, reporting a timeout for
            // a navigation that visibly succeeded.
            activeSmartCollection.set(null);
            activeCollection.set(null);
            activeDetectedClass.set(null);
            activeFolder.set(params.folder);
            await ack(true);
            const result = await importFolder(params.folder);
            const folderName = params.folder.split('/').filter(Boolean).pop() ?? params.folder;
            await loadImagesForCurrentScope({ force: true, invalidateCache: true });
            focusIndex(0);
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
                await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
            } else {
                await loadAllImages({ force: true, invalidateCache: true });
            }
            const firstId = result.image_ids[0];
            let targetImage = firstId ? null : await getImageByPath(params.path);
            const targetId = firstId ?? targetImage?.image.id;
            if (targetId) {
                const idx = await loadImagesUntil((img) => img.image.id === targetId);
                if (idx >= 0) {
                    focusIndex(idx);
                } else {
                    targetImage = targetImage ?? await getImageByPath(params.path);
                    if (targetImage) focusedImageOverride.set(targetImage);
                }
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
                await loadImagesForCurrentScope({ force: true, invalidateCache: true });
                const firstId = result.image_ids[0];
                if (firstId) {
                    const idx = await loadImagesUntil((img) => img.image.id === firstId);
                    focusIndex(idx >= 0 ? idx : 0);
                } else {
                    focusIndex(0);
                }

                const collName = c.find(([id]) => id === pinned)?.[1] ?? 'collection';
                showToast(`${result.imported} images added to "${collName}"`, {
                    type: 'success',
                    duration: 8000,
                });
            } else if (result.batch_id) {
                // No active collection — filter to batch
                const batchImgs = await getBatchImages(result.batch_id);
                invalidateImageCache();
                clearImageScope();
                resetImagePaging();
                images.set(batchImgs);
                importBatchFilter.set(result.batch_id);
                importBatchImageIds.set(result.image_ids);
                focusIndex(0);
            } else {
                await loadAllImages({ force: true, invalidateCache: true });
                focusIndex(0);
            }
        } catch (e) {
            console.error('Deep link: failed to import paths', e);
        }
    }

    // Handle explicit image-id focus from MCP/display integrations, or numeric index focus from URLs.
    const imageId = imageIdFromParams(params);
    if (imageId) {
        // focusImageById returns false for an image that could not be found or
        // loaded. Acking success there would tell show_image's caller the image
        // is on screen when the app is showing whatever it was showing before.
        if (!await focusImageById(imageId)) {
            await ack(false, `Image not found: ${imageId}`);
        }
    } else if (typeof params.focus === 'number' && Number.isFinite(params.focus)) {
        focusIndex(params.focus);
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

async function handleCanvasDropImport(params: OpenParams) {
    if (params.folder) {
        await handleCanvasFolderDrop(params);
        return;
    }

    const droppedPaths = params.paths?.length ? params.paths : (params.path ? [params.path] : []);
    if (droppedPaths.length === 0) return;

    try {
        const result = await importFiles(droppedPaths, get(activeSession)?.id ?? null);
        const imagesForPaths = await Promise.all(droppedPaths.map(path => getImageByPath(path)));
        emitCanvasImportDrop({
            images: uniqueImages(imagesForPaths.filter((image): image is ImageWithFile => image !== null)),
            paths: droppedPaths,
            dropX: params.drop_x ?? 0,
            dropY: params.drop_y ?? 0,
            importResult: result,
        });
    } catch (e) {
        console.error('Deep link: failed to import dropped canvas files', e);
        showToast('Canvas import failed', { detail: String(e), type: 'error', duration: 10000 });
    }
}

async function handleCanvasFolderDrop(params: OpenParams) {
    if (!params.folder) return;
    try {
        const result = await importFolder(params.folder, get(activeSession)?.id ?? null);
        const folderImages = await listAllImagesByFolder(params.folder);
        const f = await listFolders();
        folders.set(f);
        emitCanvasImportDrop({
            images: folderImages,
            folder: params.folder,
            dropX: params.drop_x ?? 0,
            dropY: params.drop_y ?? 0,
            importResult: result,
        });
    } catch (e) {
        console.error('Deep link: failed to import dropped canvas folder', e);
        showToast('Canvas folder import failed', { detail: String(e), type: 'error', duration: 10000 });
    }
}

async function listAllImagesByFolder(folder: string): Promise<ImageWithFile[]> {
    const allImages: ImageWithFile[] = [];
    for (let page = 0; page < FOLDER_IMAGE_PAGE_LIMIT; page++) {
        const offset = page * FOLDER_IMAGE_PAGE_SIZE;
        const batch = await listImagesByFolder(folder, FOLDER_IMAGE_PAGE_SIZE, offset);
        allImages.push(...batch);
        if (batch.length < FOLDER_IMAGE_PAGE_SIZE) break;
    }
    return uniqueImages(allImages);
}

function emitCanvasImportDrop(detail: CanvasImportDropDetail) {
    window.dispatchEvent(new CustomEvent<CanvasImportDropDetail>('canvas-import-drop', { detail }));
}

function uniqueImages(input: ImageWithFile[]): ImageWithFile[] {
    const seen = new Set<string>();
    const images: ImageWithFile[] = [];
    for (const image of input) {
        if (seen.has(image.image.id)) continue;
        seen.add(image.image.id);
        images.push(image);
    }
    return images;
}

let lastHandledKey = '';
let lastHandledAt = 0;

function deduplicatedHandleParams(params: OpenParams, source: string) {
    const key = JSON.stringify(params);
    const now = Date.now();
    if (key === lastHandledKey && now - lastHandledAt < 5000) {
        console.log(`[deep-link] Skipping duplicate from ${source}`);
        // Deliberately do NOT ack here. An identical key means a first delivery
        // is already handling these params and will ack its real outcome; acking
        // success on the duplicate would report success for work still in
        // flight, and would mask a failure the first handler is about to report.
        return;
    }
    lastHandledKey = key;
    lastHandledAt = now;
    console.log(`[deep-link] Handling from ${source}:`, key);
    void handleParams(params).catch((e) => {
        console.error(`[deep-link] Failed to handle params from ${source}`, e);
    });
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

    await listen<{ collection_id: string }>('navigate-collection', async (event) => {
        await applyClipboardMonitorCollection(event.payload.collection_id);
    });

    try {
        const pending = await drainPendingOpenParams<OpenParams>();
        for (const params of pending) {
            deduplicatedHandleParams(params, 'pending-open-params');
        }
    } catch (e) {
        console.warn('Deep link: pending open params not available', e);
    }

    try {
        const currentUrls = await getCurrent();
        if (currentUrls && currentUrls.length > 0) {
            await openDeepLinkUrls(currentUrls);
        }
    } catch (e) {
        console.warn('Deep link: startup URLs not available', e);
    }

    // Raw cull:// URLs are parsed and validated in Rust before they reach the UI.
    // The frontend may ferry startup URLs from the deep-link plugin back to Rust,
    // but it only consumes structured OpenParams after filesystem path validation.
}
