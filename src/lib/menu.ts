import { emit, listen } from '@tauri-apps/api/event';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';
import { openPath, openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { get } from 'svelte/store';
import {
    importFolder,
    importFiles,
    redo,
    undo,
    moveImage,
    listOpenWithApplications,
    openImagesWithApplication,
    renameImage,
    shareImages,
    trashImages,
    listCollections,
    listFolders,
    updateMenuState,
    getPreviewDisplayWebStreamStatus,
    type ImageWithFile,
    type OpenWithApplication,
} from './api';
import {
    images,
    viewMode,
    focusedIndex,
    focusedImage,
    sidebarVisible,
    showRejected,
    thumbnailSize,
    showLoupeHistogram,
    activeFolder,
    activeCollection,
    activeSmartCollection,
    activeDetectedClass,
    selectedIds,
    requestLoupeActualSize,
    requestLoupeFitIn,
    requestLoupeZoomIn,
    requestLoupeZoomOut,
    staticPublishingEnabled,
    pluginsEnabled,
    activePluginIds,
    aboutOpen,
    agentSkillsOpen,
    navigateTo,
    showToast,
    requestTextInput,
    collections,
    folders,
    undoHistoryOpen,
    type ViewMode,
} from './stores';
import { openSettings } from './settings-navigation';
import {
    previewDisplayAlwaysOnTop,
    previewDisplayBlanked,
    previewDisplayFrozen,
    previewDisplayLayout,
    previewDisplayMode,
    previewDisplayOverlay,
    previewDisplayWebStreamStatus,
    setPreviewDisplayWebStreamStatus,
} from './preview-display-store';
import { tabRegistry } from './plugins/tab-registry';

/** Publish is plugin-only now: it is reachable iff the bundled cull-publish
 * plugin has registered its tab in the tab registry. */
function publishTabAvailable(): boolean {
    return get(tabRegistry).some(t => t.id === 'publish');
}
import {
    copyPreviewDisplayWebStreamUrl,
    handleOpenPreviewDisplay,
    handlePreviewDisplayAlwaysOnTop,
    handlePreviewDisplayBlank,
    handlePreviewDisplayField,
    handlePreviewDisplayFreeze,
    handlePreviewDisplayFullscreen,
    handlePreviewDisplayLayout,
    handlePreviewDisplayMoveMonitor,
    handlePreviewDisplayPreset,
    handlePreviewDisplayRailSide,
    handlePreviewDisplayRailTextSize,
    handlePreviewDisplayRailWidth,
    handlePreviewDisplayStartWebStream,
    handlePreviewDisplayStopWebStream,
    requestPreviewDisplayCapture,
} from './preview-display-actions';
import { invalidateImageCache, loadAllImages, loadImagesForCurrentScope, loadImagesUntil } from './image-loading';
import { folderDisplayName } from './move-menu-utils';
import { openCommandPalette } from './command-palette';
import { checkForUpdates } from './update-manager';

type UnlistenFn = () => void;

export interface MenuInitOptions {
    listenTimeoutMs?: number;
    retryDelayMs?: number;
    stateUpdateTimeoutMs?: number;
}

const DEFAULT_LISTEN_TIMEOUT_MS = 5000;
const DEFAULT_RETRY_DELAY_MS = 1000;
const DEFAULT_STATE_UPDATE_TIMEOUT_MS = 5000;
const GITHUB_WIKI_URL = 'https://github.com/glebis/cull/wiki';

const IMAGE_FILTERS = [
    {
        name: 'Images',
        extensions: [
            'jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'tiff', 'tif', 'heic', 'heif',
            'avif', 'svg', 'jxl', 'ico', 'psd', 'cr2', 'cr3', 'nef', 'arw', 'dng',
            'orf', 'raf', 'rw2',
        ],
    },
];

async function handleOpenFile() {
    const selected = await dialogOpen({
        multiple: true,
        filters: IMAGE_FILTERS,
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length === 0) return;
    const result = await importFiles(paths);
    await loadAllImages({ force: true, invalidateCache: true });
    const firstId = result.image_ids[0];
    if (firstId) {
        const idx = await loadImagesUntil((img) => img.image.id === firstId);
        focusedIndex.set(idx >= 0 ? idx : 0);
    } else {
        focusedIndex.set(0);
    }
    if (paths.length === 1) {
        navigateTo('loupe');
    }
}

async function handleOpenFolder() {
    const selected = await dialogOpen({ directory: true });
    if (!selected || Array.isArray(selected)) return;
    await importFolder(selected);
    activeSmartCollection.set(null);
    activeCollection.set(null);
    activeDetectedClass.set(null);
    activeFolder.set(selected);
    await loadImagesForCurrentScope({ force: true, invalidateCache: true });
    focusedIndex.set(0);
    viewMode.set('grid');
}

function currentMenuTarget(options: { single?: boolean } = {}): ImageWithFile[] {
    const focused = get(focusedImage);
    if (!focused) return [];
    if (options.single) return [focused];

    const selected = get(selectedIds);
    if (selected.size > 0 && selected.has(focused.image.id)) {
        const byId = new Map(get(images).map((img) => [img.image.id, img]));
        const selectedImages = [...selected]
            .map((id) => byId.get(id))
            .filter((img): img is ImageWithFile => img !== undefined);
        if (selectedImages.length > 0) return selectedImages;
    }

    return [focused];
}

function currentMenuTargetIds(options: { single?: boolean } = {}): string[] {
    return [...new Set(currentMenuTarget(options).map((img) => img.image.id))];
}

function currentFolderPath(img: ImageWithFile): string | undefined {
    const parts = img.path.split('/');
    parts.pop();
    return parts.join('/') || undefined;
}

async function reloadAfterImageRemoval(ids: string[]) {
    const removed = new Set(ids);
    const remainingLoadedCount = get(images).filter((img) => !removed.has(img.image.id)).length;
    await loadImagesForCurrentScope({
        resetFocus: false,
        force: true,
        invalidateCache: true,
        minItems: remainingLoadedCount,
    });
    if (get(focusedIndex) >= get(images).length) {
        focusedIndex.set(Math.max(0, get(images).length - 1));
    }
    collections.set(await listCollections(get(showRejected)));
}

async function handleImageShare() {
    const ids = currentMenuTargetIds();
    if (ids.length === 0) {
        showToast('No image selected', { type: 'warning' });
        return;
    }
    try {
        await shareImages(ids);
    } catch (e) {
        showToast('Share failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

async function handleImageOpenDefault() {
    const [img] = currentMenuTarget({ single: true });
    if (!img) {
        showToast('No image selected', { type: 'warning' });
        return;
    }
    try {
        await openPath(img.path);
    } catch (e) {
        showToast('Open failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

async function chooseOpenWithApplication(img: ImageWithFile) {
    const selected = await dialogOpen({
        title: 'Open With',
        directory: true,
        multiple: false,
        defaultPath: '/Applications',
        fileAccessMode: 'scoped',
    });
    if (!selected || Array.isArray(selected)) return;
    await openImageWithApplication(img, selected);
}

async function openImageWithApplication(img: ImageWithFile, appPath: string) {
    try {
        await openImagesWithApplication([img.image.id], appPath);
    } catch (e) {
        showToast('Open With failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

function showOpenWithToast(img: ImageWithFile, apps: OpenWithApplication[]) {
    const filename = img.path.split('/').pop() ?? img.path;
    showToast('Open With', {
        detail: filename,
        duration: 12000,
        actions: [
            ...apps.slice(0, 3).map((app) => ({
                label: app.is_default ? `${app.name} (Default)` : app.name,
                onclick: () => openImageWithApplication(img, app.path),
            })),
            {
                label: 'Choose...',
                onclick: () => chooseOpenWithApplication(img),
            },
        ],
    });
}

async function handleImageOpenWith() {
    const [img] = currentMenuTarget({ single: true });
    if (!img) {
        showToast('No image selected', { type: 'warning' });
        return;
    }

    try {
        const apps = await listOpenWithApplications(img.image.id);
        if (apps.length > 0) {
            showOpenWithToast(img, apps);
        } else {
            await chooseOpenWithApplication(img);
        }
    } catch (e) {
        showToast('Open With app list unavailable', { detail: String(e), type: 'warning', duration: 8000 });
        await chooseOpenWithApplication(img);
    }
}

async function handleImageReveal() {
    const targets = currentMenuTarget();
    if (targets.length === 0) {
        showToast('No image selected', { type: 'warning' });
        return;
    }
    try {
        await revealItemInDir(targets.map((img) => img.path));
    } catch (e) {
        showToast('Reveal failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

async function handleImageRename() {
    const [img] = currentMenuTarget({ single: true });
    if (!img) {
        showToast('No image selected', { type: 'warning' });
        return;
    }

    const currentName = img.path.split('/').pop() ?? '';
    const newName = await requestTextInput({
        title: 'Rename File',
        label: 'File name',
        initialValue: currentName,
        confirmLabel: 'Rename',
    });
    if (!newName?.trim() || newName.trim() === currentName) return;

    try {
        await renameImage(img.image.id, newName.trim());
        await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        showToast(`Renamed to ${newName.trim()}`, { type: 'success' });
    } catch (e) {
        showToast('Rename failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

async function moveMenuImagesToFolder(ids: string[], folder: string) {
    let moved = 0;
    try {
        for (const id of ids) {
            await moveImage(id, folder);
            moved += 1;
        }
    } catch (e) {
        if (moved > 0) {
            await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        }
        showToast('Move incomplete', {
            detail: `${moved}/${ids.length} moved. ${String(e)}`,
            type: 'error',
            duration: 10000,
        });
        return;
    }

    await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
    try {
        folders.set(await listFolders(get(showRejected)));
    } catch (e) {
        console.error('Failed to refresh folders after move:', e);
    }
    const movedLabel = moved === 1 ? '1 image' : `${moved} images`;
    showToast(`Moved ${movedLabel} to ${folderDisplayName(folder)}`, { type: 'success' });
}

async function handleImageMoveTo() {
    const targets = currentMenuTarget();
    const ids = [...new Set(targets.map((img) => img.image.id))];
    if (ids.length === 0) {
        showToast('No image selected', { type: 'warning' });
        return;
    }

    const selected = await dialogOpen({
        title: ids.length === 1 ? 'Move Image to Folder' : `Move ${ids.length} Images to Folder`,
        directory: true,
        multiple: false,
        defaultPath: currentFolderPath(targets[0]),
        canCreateDirectories: true,
        fileAccessMode: 'scoped',
    });
    if (!selected || Array.isArray(selected)) return;

    await moveMenuImagesToFolder(ids, selected);
}

async function handleImageTrash() {
    const ids = currentMenuTargetIds();
    if (ids.length === 0) {
        showToast('No image selected', { type: 'warning' });
        return;
    }

    try {
        await trashImages(ids);
        await reloadAfterImageRemoval(ids);
    } catch (e) {
        showToast('Trash failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

async function handleGitHubWiki() {
    try {
        await openUrl(GITHUB_WIKI_URL);
    } catch (e) {
        showToast('Could not open GitHub Wiki', { detail: String(e), type: 'error', duration: 8000 });
    }
}

function handleMenuAction(action: string) {
    switch (action) {
        case 'about':
            aboutOpen.set(true);
            break;
        case 'agent_skills':
            agentSkillsOpen.set(true);
            break;
        case 'open_file':
            handleOpenFile();
            break;
        case 'import_folder':
        case 'open_folder':
            handleOpenFolder();
            break;
        case 'undo':
            undo().then(label => {
                if (!label) return;
                showToast(`Undone: ${label}`, { type: 'info', duration: 4000 });
                window.dispatchEvent(new CustomEvent('reload-images'));
            });
            break;
        case 'redo':
            redo().then(label => {
                if (!label) return;
                showToast(`Redone: ${label}`, { type: 'info', duration: 4000 });
                window.dispatchEvent(new CustomEvent('reload-images'));
            });
            break;
        case 'deselect_all':
            selectedIds.set(new Set());
            break;
        case 'command_palette':
            openCommandPalette('commands');
            break;
        case 'undo_history':
            undoHistoryOpen.set(true);
            break;
        case 'image_share':
            handleImageShare();
            break;
        case 'image_open_default':
            handleImageOpenDefault();
            break;
        case 'image_open_with':
            handleImageOpenWith();
            break;
        case 'image_reveal':
            handleImageReveal();
            break;
        case 'image_rename':
            handleImageRename();
            break;
        case 'image_move_to':
            handleImageMoveTo();
            break;
        case 'image_trash':
            handleImageTrash();
            break;
        case 'view_grid':
            navigateTo('grid');
            break;
        case 'view_compare':
            navigateTo('compare');
            break;
        case 'view_loupe':
            navigateTo('loupe');
            break;
        case 'view_canvas':
            navigateTo('canvas' as ViewMode);
            break;
        case 'view_lineage':
            navigateTo('lineage' as ViewMode);
            break;
        case 'view_embeddings':
            navigateTo('embeddings');
            break;
        case 'view_publish':
            // Publish is plugin-only: reachable iff the bundled cull-publish
            // plugin registered its tab.
            if (publishTabAvailable()) navigateTo('publish' as ViewMode);
            break;
        case 'view_export':
            navigateTo('export' as ViewMode);
            break;
        case 'view_tinder':
            viewMode.set('tinder' as ViewMode);
            break;
        case 'toggle_sidebar':
            sidebarVisible.update((v) => !v);
            break;
        case 'view_show_rejected':
            showRejected.update((visible) => !visible);
            invalidateImageCache();
            loadImagesForCurrentScope({ resetFocus: true, force: true, invalidateCache: true }).catch((e) => {
                showToast('Failed to reload images', { detail: String(e), type: 'error', duration: 8000 });
            });
            break;
        case 'view_loupe_histogram':
            showLoupeHistogram.update((visible) => !visible);
            break;
        case 'view_preview_display':
            handleOpenPreviewDisplay();
            break;
        case 'preview_display_move_monitor':
            handlePreviewDisplayMoveMonitor();
            break;
        case 'preview_display_fullscreen':
            handlePreviewDisplayFullscreen();
            break;
        case 'preview_display_always_on_top':
            handlePreviewDisplayAlwaysOnTop();
            break;
        case 'preview_display_start_web_stream':
            handlePreviewDisplayStartWebStream('127.0.0.1');
            break;
        case 'preview_display_start_lan_web_stream':
            handlePreviewDisplayStartWebStream('0.0.0.0');
            break;
        case 'preview_display_copy_web_stream_url':
            copyPreviewDisplayWebStreamUrl();
            break;
        case 'preview_display_stop_web_stream':
            handlePreviewDisplayStopWebStream();
            break;
        case 'preview_display_freeze':
            handlePreviewDisplayFreeze();
            break;
        case 'preview_display_blank':
            handlePreviewDisplayBlank();
            break;
        case 'preview_display_preset_image_only':
            handlePreviewDisplayPreset('image_only');
            break;
        case 'preview_display_preset_client_review':
            handlePreviewDisplayPreset('client_review');
            break;
        case 'preview_display_preset_metadata_review':
            handlePreviewDisplayPreset('metadata_review');
            break;
        case 'preview_display_layout_single':
            handlePreviewDisplayLayout('single');
            break;
        case 'preview_display_layout_compare':
            handlePreviewDisplayLayout('compare');
            break;
        case 'preview_display_layout_grid':
            handlePreviewDisplayLayout('grid');
            break;
        case 'preview_display_copy_to_clipboard':
            requestPreviewDisplayCapture('clipboard');
            break;
        case 'preview_display_export_png':
            requestPreviewDisplayCapture('png');
            break;
        case 'preview_display_field_filename':
            handlePreviewDisplayField('showFilename');
            break;
        case 'preview_display_field_rating':
            handlePreviewDisplayField('showRating');
            break;
        case 'preview_display_field_decision':
            handlePreviewDisplayField('showDecision');
            break;
        case 'preview_display_field_dimensions':
            handlePreviewDisplayField('showDimensions');
            break;
        case 'preview_display_field_format':
            handlePreviewDisplayField('showFormat');
            break;
        case 'preview_display_field_source':
            handlePreviewDisplayField('showSource');
            break;
        case 'preview_display_field_prompt':
            handlePreviewDisplayField('showPrompt');
            break;
        case 'preview_display_field_tags':
            handlePreviewDisplayField('showTags');
            break;
        case 'preview_display_field_histogram':
            handlePreviewDisplayField('showHistogram');
            break;
        case 'preview_display_rail_left':
            handlePreviewDisplayRailSide('left');
            break;
        case 'preview_display_rail_right':
            handlePreviewDisplayRailSide('right');
            break;
        case 'preview_display_rail_width_narrow':
            handlePreviewDisplayRailWidth('narrow');
            break;
        case 'preview_display_rail_width_medium':
            handlePreviewDisplayRailWidth('medium');
            break;
        case 'preview_display_rail_width_wide':
            handlePreviewDisplayRailWidth('wide');
            break;
        case 'preview_display_text_small':
            handlePreviewDisplayRailTextSize('small');
            break;
        case 'preview_display_text_medium':
            handlePreviewDisplayRailTextSize('medium');
            break;
        case 'preview_display_text_large':
            handlePreviewDisplayRailTextSize('large');
            break;
        case 'zoom_in':
            if (get(viewMode) === 'loupe') {
                requestLoupeZoomIn();
            } else {
                thumbnailSize.update((s) => Math.min(s + 40, 600));
            }
            break;
        case 'zoom_out':
            if (get(viewMode) === 'loupe') {
                requestLoupeZoomOut();
            } else {
                thumbnailSize.update((s) => Math.max(s - 40, 40));
            }
            break;
        case 'actual_size':
            requestLoupeActualSize();
            break;
        case 'fit_in':
            requestLoupeFitIn();
            break;
        case 'settings':
            openSettings('general');
            break;
        case 'check_update':
            void checkForUpdates('manual');
            break;
        case 'github_wiki':
            handleGitHubWiki();
            break;
    }
}

let menuOptions: Required<MenuInitOptions> = {
    listenTimeoutMs: DEFAULT_LISTEN_TIMEOUT_MS,
    retryDelayMs: DEFAULT_RETRY_DELAY_MS,
    stateUpdateTimeoutMs: DEFAULT_STATE_UPDATE_TIMEOUT_MS,
};

let menuStateSubscriptionsStarted = false;
let menuStateQueued = false;
let menuStateDirty = false;
let menuStateUpdateInFlight = false;
let menuStateRetryTimer: ReturnType<typeof setTimeout> | null = null;
let menuActionUnlisten: UnlistenFn | null = null;
let menuActionListenInFlight = false;
let menuActionRetryTimer: ReturnType<typeof setTimeout> | null = null;
let menuActionListenGeneration = 0;

function withTimeout<T>(
    promise: Promise<T>,
    timeoutMs: number,
    label: string,
    onLateResolve?: (value: T) => void
): Promise<T> {
    if (timeoutMs <= 0) return promise;

    let settled = false;
    let timedOut = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    return new Promise<T>((resolve, reject) => {
        timer = setTimeout(() => {
            timedOut = true;
            settled = true;
            reject(new Error(`${label} timed out after ${timeoutMs}ms`));
        }, timeoutMs);

        promise.then(
            (value) => {
                if (timedOut) {
                    onLateResolve?.(value);
                    return;
                }
                if (settled) return;
                settled = true;
                if (timer) clearTimeout(timer);
                resolve(value);
            },
            (error) => {
                if (settled) return;
                settled = true;
                if (timer) clearTimeout(timer);
                reject(error);
            }
        );
    });
}

function currentMenuStatePayload() {
    return {
        viewMode: get(viewMode),
        sidebarVisible: get(sidebarVisible),
        showRejected: get(showRejected),
        hasFocusedImage: get(focusedImage) !== null,
        selectedCount: get(selectedIds).size,
        staticPublishingEnabled: publishTabAvailable(),
        showLoupeHistogram: get(showLoupeHistogram),
        previewDisplayFrozen: get(previewDisplayFrozen),
        previewDisplayBlanked: get(previewDisplayBlanked),
        previewDisplayAlwaysOnTop: get(previewDisplayAlwaysOnTop),
        previewDisplayMode: get(previewDisplayMode),
        previewDisplayLayout: get(previewDisplayLayout),
        previewDisplayOverlay: get(previewDisplayOverlay),
        previewDisplayWebStreamActive: get(previewDisplayWebStreamStatus).active,
    };
}

function queueMenuStateUpdate() {
    menuStateDirty = true;
    if (menuStateQueued || menuStateUpdateInFlight) return;
    menuStateQueued = true;
    queueMicrotask(() => {
        menuStateQueued = false;
        flushMenuStateUpdate();
    });
}

function scheduleMenuStateRetry() {
    if (menuStateRetryTimer) return;
    menuStateRetryTimer = setTimeout(() => {
        menuStateRetryTimer = null;
        queueMenuStateUpdate();
    }, menuOptions.retryDelayMs);
}

function flushMenuStateUpdate() {
    if (!menuStateDirty || menuStateUpdateInFlight) return;

    menuStateDirty = false;
    menuStateUpdateInFlight = true;
    withTimeout(
        updateMenuState(currentMenuStatePayload()),
        menuOptions.stateUpdateTimeoutMs,
        'Native menu state update'
    )
        .catch((e) => {
            menuStateDirty = true;
            console.debug('Failed to update native menu state:', e);
            scheduleMenuStateRetry();
        })
        .finally(() => {
            menuStateUpdateInFlight = false;
            if (menuStateDirty && !menuStateRetryTimer) {
                queueMenuStateUpdate();
            }
        });
}

function startMenuStateSubscriptions() {
    if (menuStateSubscriptionsStarted) return;
    menuStateSubscriptionsStarted = true;

    viewMode.subscribe(queueMenuStateUpdate);
    sidebarVisible.subscribe(queueMenuStateUpdate);
    showRejected.subscribe(queueMenuStateUpdate);
    focusedImage.subscribe(queueMenuStateUpdate);
    selectedIds.subscribe(queueMenuStateUpdate);
    staticPublishingEnabled.subscribe(queueMenuStateUpdate);
    pluginsEnabled.subscribe(queueMenuStateUpdate);
    activePluginIds.subscribe(queueMenuStateUpdate);
    showLoupeHistogram.subscribe(queueMenuStateUpdate);
    previewDisplayFrozen.subscribe(queueMenuStateUpdate);
    previewDisplayBlanked.subscribe(queueMenuStateUpdate);
    previewDisplayAlwaysOnTop.subscribe(queueMenuStateUpdate);
    previewDisplayMode.subscribe(queueMenuStateUpdate);
    previewDisplayLayout.subscribe(queueMenuStateUpdate);
    previewDisplayOverlay.subscribe(queueMenuStateUpdate);
    previewDisplayWebStreamStatus.subscribe(queueMenuStateUpdate);
    queueMenuStateUpdate();
}

function cleanupLateMenuListener(unlisten: UnlistenFn) {
    try {
        unlisten();
    } catch (e) {
        console.debug('Failed to clean up late native menu listener:', e);
    }
}

function scheduleMenuActionListenerRestart() {
    if (menuActionRetryTimer) return;
    menuActionRetryTimer = setTimeout(() => {
        menuActionRetryTimer = null;
        startMenuActionListener();
    }, menuOptions.retryDelayMs);
}

function startMenuActionListener() {
    if (menuActionUnlisten || menuActionListenInFlight) return;

    menuActionListenInFlight = true;
    const generation = ++menuActionListenGeneration;

    withTimeout(
        listen<string>('menu-action', (event) => {
            handleMenuAction(event.payload);
        }),
        menuOptions.listenTimeoutMs,
        'Native menu listener setup',
        cleanupLateMenuListener
    )
        .then((unlisten) => {
            if (generation !== menuActionListenGeneration) {
                cleanupLateMenuListener(unlisten);
                return;
            }
            menuActionUnlisten = unlisten;
        })
        .catch((e) => {
            if (generation !== menuActionListenGeneration) return;
            console.debug('Failed to start native menu listener:', e);
            scheduleMenuActionListenerRestart();
        })
        .finally(() => {
            if (generation === menuActionListenGeneration) {
                menuActionListenInFlight = false;
            }
        });
}

export function restartMenuActionListener() {
    menuActionListenGeneration += 1;
    if (menuActionRetryTimer) {
        clearTimeout(menuActionRetryTimer);
        menuActionRetryTimer = null;
    }
    if (menuActionUnlisten) {
        cleanupLateMenuListener(menuActionUnlisten);
        menuActionUnlisten = null;
    }
    menuActionListenInFlight = false;
    startMenuActionListener();
}

export async function initMenu(options: MenuInitOptions = {}) {
    menuOptions = {
        listenTimeoutMs: options.listenTimeoutMs ?? DEFAULT_LISTEN_TIMEOUT_MS,
        retryDelayMs: options.retryDelayMs ?? DEFAULT_RETRY_DELAY_MS,
        stateUpdateTimeoutMs: options.stateUpdateTimeoutMs ?? DEFAULT_STATE_UPDATE_TIMEOUT_MS,
    };

    startMenuStateSubscriptions();
    getPreviewDisplayWebStreamStatus()
        .then(setPreviewDisplayWebStreamStatus)
        .catch((e) => console.debug('Failed to read Preview Display web stream status:', e));
    startMenuActionListener();
    queueMenuStateUpdate();
}
