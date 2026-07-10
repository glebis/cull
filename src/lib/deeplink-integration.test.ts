import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockState = vi.hoisted(() => ({
    images: [] as any[],
    viewMode: 'grid',
}));

vi.mock('./stores', () => ({
    viewMode: {
        set: vi.fn((value) => { mockState.viewMode = value; }),
        subscribe: vi.fn((run) => { run(mockState.viewMode); return vi.fn(); }),
    },
    thumbnailSize: { set: vi.fn() },
    focusedIndex: { set: vi.fn() },
    focusedImageOverride: { set: vi.fn() },
    images: {
        set: vi.fn((value) => { mockState.images = value; }),
        subscribe: vi.fn((run) => { run(mockState.images); return vi.fn(); }),
    },
    gridGap: { set: vi.fn() },
    loupeScale: { set: vi.fn() },
    activeFolder: { set: vi.fn() },
    folders: { set: vi.fn() },
    activeCollection: { set: vi.fn(), subscribe: vi.fn((run) => { run(null); return vi.fn(); }) },
    activeSmartCollection: { set: vi.fn() },
    activeDetectedClass: { set: vi.fn() },
    activeSession: { subscribe: vi.fn((run) => { run(null); return vi.fn(); }) },
    collections: { set: vi.fn() },
    windowName: { set: vi.fn() },
    windowLabel: { set: vi.fn() },
    navigateTo: vi.fn(),
    showToast: vi.fn(),
    pinnedCollection: { subscribe: vi.fn((run) => { run(null); return vi.fn(); }) },
    importBatchFilter: { set: vi.fn() },
    importBatchImageIds: { set: vi.fn() },
    embeddingViewState: { set: vi.fn(), subscribe: vi.fn(() => vi.fn()) },
}));

vi.mock('./api', () => ({
    importFolder: vi.fn(),
    importFiles: vi.fn(),
    addToCollection: vi.fn(),
    listCollections: vi.fn(),
    listFolders: vi.fn(),
    listImagesByFolder: vi.fn(),
    getBatchImages: vi.fn(),
    getImagesByIds: vi.fn(),
    getImageByPath: vi.fn(),
    drainPendingOpenParams: vi.fn(),
    openDeepLinkUrls: vi.fn(),
}));

vi.mock('./image-loading', () => ({
    clearImageScope: vi.fn(),
    invalidateImageCache: vi.fn(),
    loadAllImages: vi.fn(),
    loadImagesForCurrentScope: vi.fn(),
    loadImagesUntil: vi.fn(),
    resetImagePaging: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-deep-link', () => ({
    onOpenUrl: vi.fn(),
    getCurrent: vi.fn(),
}));

import { handleParams, initDeepLink } from './deeplink';
import { thumbnailSize, focusedIndex, focusedImageOverride, images, gridGap, loupeScale, activeFolder, navigateTo } from './stores';
import { importFolder, importFiles, getBatchImages, listFolders, listImagesByFolder, getImagesByIds, getImageByPath, drainPendingOpenParams, openDeepLinkUrls } from './api';
import { loadAllImages, loadImagesForCurrentScope, loadImagesUntil } from './image-loading';
import { listen } from '@tauri-apps/api/event';
import { onOpenUrl, getCurrent } from '@tauri-apps/plugin-deep-link';

beforeEach(() => {
    vi.clearAllMocks();
    mockState.images = [];
    mockState.viewMode = 'grid';
    vi.mocked(importFolder).mockResolvedValue({ imported: 0, skipped: 0, errors: [], batch_id: null, image_ids: [] } as never);
    vi.mocked(importFiles).mockResolvedValue({ imported: 0, skipped: 0, errors: [], batch_id: null, image_ids: [] } as never);
    vi.mocked(listFolders).mockResolvedValue([] as never);
    vi.mocked(listImagesByFolder).mockResolvedValue([] as never);
    vi.mocked(getImagesByIds).mockResolvedValue([] as never);
    vi.mocked(getImageByPath).mockResolvedValue(null as never);
    vi.mocked(drainPendingOpenParams).mockResolvedValue([] as never);
    vi.mocked(loadImagesUntil).mockResolvedValue(-1);
});

describe('handleParams', () => {
    it('sets view mode via navigateTo for valid views', async () => {
        await handleParams({ view: 'loupe' });
        expect(navigateTo).toHaveBeenCalledWith('loupe');
    });

    it('ignores invalid view modes', async () => {
        await handleParams({ view: 'invalid' });
        expect(navigateTo).not.toHaveBeenCalled();
    });

    it('sets thumbnail size', async () => {
        await handleParams({ size: 200 });
        expect(thumbnailSize.set).toHaveBeenCalledWith(200);
    });

    it('sets grid gap', async () => {
        await handleParams({ gap: 8 });
        expect(gridGap.set).toHaveBeenCalledWith(8);
    });

    it('sets loupe scale from zoom percentage', async () => {
        await handleParams({ zoom: 200 });
        expect(loupeScale.set).toHaveBeenCalledWith(2);
    });

    it('imports folder and updates stores', async () => {
        vi.mocked(importFolder).mockResolvedValue({ imported: 1, skipped: 0, errors: [], batch_id: null, image_ids: ['1'] } as never);
        vi.mocked(listFolders).mockResolvedValue([['/test', 1]] as never);

        await handleParams({ folder: '/test' });

        expect(importFolder).toHaveBeenCalledWith('/test');
        expect(activeFolder.set).toHaveBeenCalledWith('/test');
        expect(loadImagesForCurrentScope).toHaveBeenCalled();
        expect(focusedIndex.set).toHaveBeenCalledWith(0);
    });

    it('routes dropped folders to canvas placement without switching to grid', async () => {
        const folderImages = [
            { image: { id: 'existing-1' }, path: '/test/a.png' },
            { image: { id: 'new-1' }, path: '/test/b.png' },
        ];
        vi.mocked(importFolder).mockResolvedValue({ imported: 1, skipped: 1, errors: [], batch_id: 'batch-1', image_ids: ['new-1'] } as never);
        vi.mocked(listImagesByFolder)
            .mockResolvedValueOnce(folderImages as never)
            .mockResolvedValueOnce([] as never);
        vi.mocked(listFolders).mockResolvedValue([['/test', 2]] as never);
        const dispatchSpy = vi.fn();
        vi.stubGlobal('window', { dispatchEvent: dispatchSpy });

        mockState.viewMode = 'canvas';
        await handleParams({ folder: '/test', view: 'grid', drag_drop: true, drop_x: 140, drop_y: 220 });

        expect(navigateTo).not.toHaveBeenCalledWith('grid');
        expect(importFolder).toHaveBeenCalledWith('/test', null);
        expect(listImagesByFolder).toHaveBeenCalledWith('/test', 250, 0);
        expect(dispatchSpy).toHaveBeenCalledWith(expect.objectContaining({
            type: 'canvas-import-drop',
            detail: expect.objectContaining({
                images: folderImages,
                folder: '/test',
                dropX: 140,
                dropY: 220,
            }),
        }));
        vi.unstubAllGlobals();
    });

    it('routes dropped files to canvas placement and includes already imported paths', async () => {
        const image = { image: { id: 'existing-1' }, path: '/test/a.png' };
        vi.mocked(importFiles).mockResolvedValue({ imported: 0, skipped: 1, errors: [], batch_id: null, image_ids: [] } as never);
        vi.mocked(getImageByPath).mockResolvedValue(image as never);
        const dispatchSpy = vi.fn();
        vi.stubGlobal('window', { dispatchEvent: dispatchSpy });

        mockState.viewMode = 'canvas';
        await handleParams({ path: '/test/a.png', view: 'loupe', drag_drop: true, drop_x: 10, drop_y: 20 });

        expect(navigateTo).not.toHaveBeenCalledWith('loupe');
        expect(importFiles).toHaveBeenCalledWith(['/test/a.png'], null);
        expect(dispatchSpy).toHaveBeenCalledWith(expect.objectContaining({
            type: 'canvas-import-drop',
            detail: expect.objectContaining({
                images: [image],
                paths: ['/test/a.png'],
                dropX: 10,
                dropY: 20,
            }),
        }));
        vi.unstubAllGlobals();
    });

    it('imports single path and focuses the imported image', async () => {
        vi.mocked(importFiles).mockResolvedValue({ imported: 1, skipped: 0, errors: [], batch_id: null, image_ids: ['2'] } as never);
        vi.mocked(loadImagesUntil).mockResolvedValue(1);

        await handleParams({ path: '/target.jpg' });

        expect(importFiles).toHaveBeenCalledWith(['/target.jpg']);
        expect(loadAllImages).toHaveBeenCalled();
        expect(focusedIndex.set).toHaveBeenCalledWith(1);
    });

    it('focuses an already-imported single path by resolving the path after import skip', async () => {
        const image = { image: { id: 'existing-1' }, path: '/target.jpg' };
        vi.mocked(importFiles).mockResolvedValue({ imported: 0, skipped: 1, errors: [], batch_id: null, image_ids: [] } as never);
        vi.mocked(getImageByPath).mockResolvedValue(image as never);
        vi.mocked(loadImagesUntil).mockResolvedValue(4);

        await handleParams({ path: '/target.jpg' });

        expect(getImageByPath).toHaveBeenCalledWith('/target.jpg');
        expect(loadImagesUntil).toHaveBeenCalled();
        expect(focusedIndex.set).toHaveBeenCalledWith(4);
    });

    it('focuses explicit image_id when the image is already loaded', async () => {
        mockState.images = [
            { image: { id: 'img-1' }, path: '/one.jpg' },
            { image: { id: 'img-2' }, path: '/two.jpg' },
        ];

        await handleParams({ image_id: 'img-2', view: 'loupe' });

        expect(navigateTo).toHaveBeenCalledWith('loupe');
        expect(focusedImageOverride.set).toHaveBeenCalledWith(null);
        expect(focusedIndex.set).toHaveBeenCalledWith(1);
        expect(getImagesByIds).not.toHaveBeenCalled();
    });

    it('focuses explicit image_id with an override when outside the loaded list', async () => {
        const image = { image: { id: 'img-outside' }, path: '/outside.jpg' };
        vi.mocked(getImagesByIds).mockResolvedValue([image] as never);

        await handleParams({ image_id: 'img-outside', view: 'loupe' });

        expect(navigateTo).toHaveBeenCalledWith('loupe');
        expect(getImagesByIds).toHaveBeenCalledWith(['img-outside']);
        expect(focusedImageOverride.set).toHaveBeenCalledWith(image);
    });

    it('does not set focusedIndex if imported image not found in list', async () => {
        vi.mocked(importFiles).mockResolvedValue({ imported: 1, skipped: 0, errors: [], batch_id: null, image_ids: ['missing'] } as never);
        vi.mocked(loadImagesUntil).mockResolvedValue(-1);

        await handleParams({ path: '/missing.jpg' });

        expect(focusedIndex.set).not.toHaveBeenCalled();
    });

    it('imports multiple paths', async () => {
        const fakeImages = [{ path: '/a.jpg' }, { path: '/b.jpg' }];
        vi.mocked(importFiles).mockResolvedValue({ imported: 2, skipped: 0, image_ids: ['1', '2'], batch_id: 'b1' } as never);
        vi.mocked(getBatchImages).mockResolvedValue(fakeImages as never);

        await handleParams({ paths: ['/a.jpg', '/b.jpg'] });

        expect(importFiles).toHaveBeenCalledWith(['/a.jpg', '/b.jpg']);
        expect(images.set).toHaveBeenCalledWith(fakeImages);
        expect(focusedIndex.set).toHaveBeenCalledWith(0);
    });

    it('skips empty paths array', async () => {
        await handleParams({ paths: [] });
        expect(importFiles).not.toHaveBeenCalled();
    });

    it('sets focus index', async () => {
        await handleParams({ focus: 5 });
        expect(focusedIndex.set).toHaveBeenCalledWith(5);
    });

    it('handles folder import failure gracefully', async () => {
        vi.mocked(importFolder).mockRejectedValue(new Error('fail'));
        const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

        await handleParams({ folder: '/bad' });

        expect(consoleSpy).toHaveBeenCalled();
        expect(images.set).not.toHaveBeenCalled();
        consoleSpy.mockRestore();
    });

    it('handles path import failure gracefully', async () => {
        vi.mocked(importFiles).mockRejectedValue(new Error('fail'));
        const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

        await handleParams({ path: '/bad.jpg' });

        expect(consoleSpy).toHaveBeenCalled();
        consoleSpy.mockRestore();
    });

    it('does nothing for empty params', async () => {
        await handleParams({});
        expect(navigateTo).not.toHaveBeenCalled();
        expect(thumbnailSize.set).not.toHaveBeenCalled();
        expect(importFolder).not.toHaveBeenCalled();
        expect(importFiles).not.toHaveBeenCalled();
    });

    it('handles multiple params together', async () => {
        vi.mocked(importFolder).mockResolvedValue({ imported: 1, skipped: 0, errors: [], batch_id: null, image_ids: ['1'] } as never);
        vi.mocked(listFolders).mockResolvedValue([['/test', 1]] as never);

        await handleParams({
            view: 'grid',
            size: 120,
            gap: 4,
            zoom: 150,
            folder: '/test',
            focus: 3,
        });

        expect(navigateTo).toHaveBeenCalledWith('grid');
        expect(thumbnailSize.set).toHaveBeenCalledWith(120);
        expect(gridGap.set).toHaveBeenCalledWith(4);
        expect(loupeScale.set).toHaveBeenCalledWith(1.5);
        expect(importFolder).toHaveBeenCalledWith('/test');
        expect(focusedIndex.set).toHaveBeenCalledWith(3);
    });
});

describe('initDeepLink', () => {
    it('registers listeners for window, open params, and collection navigation', async () => {
        vi.mocked(listen).mockResolvedValue(vi.fn() as never);
        vi.mocked(onOpenUrl).mockResolvedValue(undefined as never);
        vi.mocked(getCurrent).mockResolvedValue([]);

        await initDeepLink();

        expect(listen).toHaveBeenCalledTimes(3);
        expect(vi.mocked(listen).mock.calls[0][0]).toBe('set-window-name');
        expect(vi.mocked(listen).mock.calls[1][0]).toBe('open-with-params');
        expect(vi.mocked(listen).mock.calls[2][0]).toBe('navigate-collection');
    });

    it('does not register raw frontend deep-link event handlers', async () => {
        vi.mocked(listen).mockResolvedValue(vi.fn() as never);
        vi.mocked(onOpenUrl).mockResolvedValue(undefined as never);
        vi.mocked(getCurrent).mockResolvedValue([]);

        await initDeepLink();

        expect(onOpenUrl).not.toHaveBeenCalled();
        expect(getCurrent).toHaveBeenCalled();
    });

    it('sends raw launch URLs from getCurrent to Rust validation', async () => {
        vi.mocked(listen).mockResolvedValue(vi.fn() as never);
        vi.mocked(onOpenUrl).mockResolvedValue(undefined as never);
        vi.mocked(getCurrent).mockResolvedValue(['cull://loupe?size=200']);

        await initDeepLink();

        expect(openDeepLinkUrls).toHaveBeenCalledWith(['cull://loupe?size=200']);
        expect(navigateTo).not.toHaveBeenCalled();
        expect(thumbnailSize.set).not.toHaveBeenCalled();
    });

    it('handles startup URL retrieval failure gracefully', async () => {
        vi.mocked(listen).mockResolvedValue(vi.fn() as never);
        vi.mocked(getCurrent).mockRejectedValue(new Error('not available'));

        const consoleSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
        await initDeepLink();

        expect(consoleSpy).toHaveBeenCalled();
        expect(openDeepLinkUrls).not.toHaveBeenCalled();
        consoleSpy.mockRestore();
    });

    it('handles Rust startup URL validation failure gracefully', async () => {
        vi.mocked(listen).mockResolvedValue(vi.fn() as never);
        vi.mocked(getCurrent).mockResolvedValue(['cull://open?path=/bad%ZZ']);
        vi.mocked(openDeepLinkUrls).mockRejectedValue(new Error('rejected') as never);

        const consoleSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
        await initDeepLink();

        expect(openDeepLinkUrls).toHaveBeenCalledWith(['cull://open?path=/bad%ZZ']);
        expect(consoleSpy).toHaveBeenCalled();
        consoleSpy.mockRestore();
    });
});
