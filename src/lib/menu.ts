import { listen } from '@tauri-apps/api/event';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';
import { importFolder, importFiles, listImages, listImagesByFolder } from './api';
import {
    viewMode,
    images,
    focusedIndex,
    sidebarVisible,
    thumbnailSize,
    activeFolder,
    selectedIds,
    loupeScale,
    settingsOpen,
    type ViewMode,
} from './stores';

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
    await importFiles(paths);
    const allImgs = await listImages(100000, 0);
    images.set(allImgs);
    const idx = allImgs.findIndex((img) => img.path === paths[0]);
    focusedIndex.set(idx >= 0 ? idx : 0);
    if (paths.length === 1) {
        viewMode.set('loupe');
    }
}

async function handleOpenFolder() {
    const selected = await dialogOpen({ directory: true });
    if (!selected || Array.isArray(selected)) return;
    await importFolder(selected);
    activeFolder.set(selected);
    const imgs = await listImagesByFolder(selected, 100000, 0);
    images.set(imgs);
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
        case 'deselect_all':
            selectedIds.set(new Set());
            break;
        case 'view_grid':
            viewMode.set('grid');
            break;
        case 'view_compare':
            viewMode.set('compare');
            break;
        case 'view_loupe':
            viewMode.set('loupe');
            break;
        case 'view_canvas':
            viewMode.set('canvas' as ViewMode);
            break;
        case 'view_lineage':
            viewMode.set('lineage' as ViewMode);
            break;
        case 'view_embeddings':
            viewMode.set('embeddings');
            break;
        case 'view_export':
            viewMode.set('export' as ViewMode);
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
            settingsOpen.update(v => !v);
            break;
        case 'help':
            // Help not yet implemented
            break;
    }
}

export async function initMenu() {
    await listen<string>('menu-action', (event) => {
        handleMenuAction(event.payload);
    });
}
