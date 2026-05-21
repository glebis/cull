import { listen } from '@tauri-apps/api/event';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { importFolder, importFiles, redo, undo } from './api';
import {
    viewMode,
    focusedIndex,
    sidebarVisible,
    thumbnailSize,
    activeFolder,
    activeCollection,
    activeSmartCollection,
    activeDetectedClass,
    selectedIds,
    loupeScale,
    settingsOpen,
    navigateTo,
    showToast,
    type ViewMode,
} from './stores';
import { loadAllImages, loadImagesForCurrentScope, loadImagesUntil } from './image-loading';

const IMAGE_FILTERS = [
    { name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'tiff', 'tif', 'heic', 'heif', 'avif', 'svg'] },
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
        viewMode.set('loupe');
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

function handleMenuAction(action: string) {
    switch (action) {
        case 'open_file':
            handleOpenFile();
            break;
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
        case 'view_export':
            navigateTo('export' as ViewMode);
            break;
        case 'toggle_sidebar':
            sidebarVisible.update((v) => !v);
            break;
        case 'zoom_in':
            thumbnailSize.update((s) => Math.min(s + 40, 600));
            loupeScale.update((s) => Math.min(s * 1.25, 20));
            break;
        case 'zoom_out':
            thumbnailSize.update((s) => Math.max(s - 40, 40));
            loupeScale.update((s) => Math.max(s / 1.25, 0.1));
            break;
        case 'actual_size':
            loupeScale.set(1);
            break;
        case 'settings':
            settingsOpen.set(true);
            break;
        case 'help':
            openUrl('https://github.com/glebis/imageview#readme');
            break;
    }
}

export async function initMenu() {
    await listen<string>('menu-action', (event) => {
        handleMenuAction(event.payload);
    });
}
