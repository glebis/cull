import { onOpenUrl, getCurrent } from '@tauri-apps/plugin-deep-link';
import { listen } from '@tauri-apps/api/event';
import {
    viewMode,
    thumbnailSize,
    focusedIndex,
    images,
    gridGap,
    loupeScale,
    activeFolder,
    windowName,
    windowLabel,
    navigateTo,
    type ViewMode,
} from './stores';
import { importFolder, importFiles, listImagesByFolder, listImages } from './api';

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
            await importFolder(params.folder);
            activeFolder.set(params.folder);
            const imgs = await listImagesByFolder(params.folder, 100000, 0);
            images.set(imgs);
            focusedIndex.set(0);
        } catch (e) {
            console.error('Deep link: failed to import folder', e);
        }
    }

    // Handle single path import
    if (params.path) {
        try {
            await importFiles([params.path]);
            const allImgs = await listImages(100000, 0);
            images.set(allImgs);
            // Focus the imported image
            const idx = allImgs.findIndex((img) => img.path === params.path);
            if (idx >= 0) {
                focusedIndex.set(idx);
            }
        } catch (e) {
            console.error('Deep link: failed to import path', e);
        }
    }

    // Handle multiple paths
    if (params.paths && params.paths.length > 0) {
        try {
            await importFiles(params.paths);
            const allImgs = await listImages(100000, 0);
            images.set(allImgs);
            focusedIndex.set(0);
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

export async function initDeepLink() {
    // Listen for window name assignment from Rust
    await listen<{ label: string; name: string }>('set-window-name', (event) => {
        windowLabel.set(event.payload.label);
        windowName.set(event.payload.name);
    });

    // Listen for open-with-params events (from Rust deep link handler + open_with_params command)
    await listen<OpenParams>('open-with-params', (event) => {
        handleParams(event.payload);
    });

    // Listen for deep link URLs opened while app is running (macOS)
    try {
        await onOpenUrl((urls) => {
            for (const url of urls) {
                const params = parseDeepLinkUrl(url);
                handleParams(params);
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
                handleParams(params);
            }
        }
    } catch (e) {
        console.warn('Deep link: getCurrent not available', e);
    }
}
