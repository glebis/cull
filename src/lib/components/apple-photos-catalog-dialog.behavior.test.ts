// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ApplePhotosCatalogDialog from './ApplePhotosCatalogDialog.svelte';
import type {
    ApplePhotosAlbum,
    ApplePhotosAsset,
    ApplePhotosCatalogClient,
    ApplePhotosPage,
} from '$lib/apple-photos';

afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
});

function page<T>(items: T[], offset = 0, total = items.length): ApplePhotosPage<T> {
    return { items, offset, total, has_more: offset + items.length < total };
}

function album(id: string, title: string): ApplePhotosAlbum {
    return { id, title, kind: 'user', role: null };
}

function asset(id: string, filename: string): ApplePhotosAsset {
    return {
        id,
        filename,
        created_at: '2026-08-08T12:00:00Z',
        modified_at: null,
        pixel_width: 1200,
        pixel_height: 800,
        favorite: false,
        media_subtypes: 0,
    };
}

function client(overrides: Partial<ApplePhotosCatalogClient> = {}): ApplePhotosCatalogClient {
    return {
        authorizationStatus: vi.fn().mockResolvedValue('authorized'),
        requestAuthorization: vi.fn().mockResolvedValue('authorized'),
        listAlbums: vi.fn().mockResolvedValue(page([])),
        listAssets: vi.fn().mockResolvedValue(page([])),
        loadPreview: vi.fn().mockResolvedValue(null),
        ...overrides,
    };
}

describe('Apple Photos catalog dialog', () => {
    it('requests access only after an explicit user action, then lists the limited catalog', async () => {
        const catalog = client({
            authorizationStatus: vi.fn().mockResolvedValue('not_determined'),
            requestAuthorization: vi.fn().mockResolvedValue('limited'),
            listAlbums: vi.fn().mockResolvedValue(page([album('album-1', 'Favourites')])),
            listAssets: vi.fn().mockResolvedValue(page([asset('asset-1', 'IMG_0001.HEIC')])),
        });
        const user = userEvent.setup();
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        expect(await screen.findByText('Cull needs permission before it can browse your Photos library.')).toBeInTheDocument();
        expect(catalog.requestAuthorization).not.toHaveBeenCalled();

        await user.click(screen.getByRole('button', { name: 'Allow Photos Access' }));

        expect(await screen.findByText('Showing the photos you allowed Cull to access.')).toBeInTheDocument();
        expect(await screen.findByRole('option', { name: 'Favourites' })).toBeInTheDocument();
        expect(await screen.findByRole('button', { name: 'Select IMG_0001.HEIC' })).toBeInTheDocument();
        expect(catalog.listAssets).toHaveBeenCalledWith(null, 0, 100);
    });

    it('pins favourites and screenshots by stable PhotoKit role instead of localized title', async () => {
        const catalog = client({
            listAlbums: vi.fn().mockResolvedValue(page([
                { id: 'fav-id', title: 'Favoriten', kind: 'smart', role: 'favorites' },
                { id: 'shots-id', title: 'Bildschirmfotos', kind: 'smart', role: 'screenshots' },
            ])),
        });
        const user = userEvent.setup();
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        await user.click(await screen.findByRole('button', { name: 'Favourites' }));
        expect(catalog.listAssets).toHaveBeenLastCalledWith('fav-id', 0, 100);
        expect(screen.getByRole('button', { name: 'Favourites' })).toHaveAttribute('aria-current', 'page');
        expect(screen.getByRole('button', { name: 'Screenshots' })).toBeInTheDocument();
    });

    it('offers a bounded grid-style control for changing photo preview scale', async () => {
        const catalog = client({
            listAssets: vi.fn().mockResolvedValue(page([
                asset('asset-1', 'One.jpg'),
                asset('asset-2', 'Two.jpg'),
                asset('asset-3', 'Three.jpg'),
            ])),
        });
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        const slider = await screen.findByRole('slider', { name: 'Preview size' });
        expect(screen.getByRole('group', { name: 'Photo preview scale' })).toContainElement(slider);
        expect(slider).toHaveAttribute('aria-valuetext', '120 pixel previews');
        expect(screen.getByRole('button', { name: 'Zoom photo previews out' })).toBeEnabled();
        expect(screen.getByRole('button', { name: 'Zoom photo previews in' })).toBeEnabled();

        await fireEvent.input(slider, { target: { value: '100' } });
        expect(slider).toHaveAttribute('aria-valuetext', '220 pixel previews');
    });

    it('switches albums, ignores a stale response, and loads the next bounded page on scroll', async () => {
        let resolveAll!: (value: ApplePhotosPage<ApplePhotosAsset>) => void;
        const allPending = new Promise<ApplePhotosPage<ApplePhotosAsset>>(resolve => { resolveAll = resolve; });
        const catalog = client({
            listAlbums: vi.fn().mockResolvedValue(page([
                album('album-a', 'A'),
                album('album-b', 'B'),
            ])),
            listAssets: vi.fn()
                .mockImplementationOnce(() => allPending)
                .mockResolvedValueOnce(page([asset('b-1', 'B-1.jpg')], 0, 2))
                .mockResolvedValueOnce(page([asset('b-2', 'B-2.jpg')], 1, 2)),
        });
        const user = userEvent.setup();
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        await user.click(await screen.findByRole('button', { name: 'B' }));
        expect(await screen.findByRole('button', { name: 'Select B-1.jpg' })).toBeInTheDocument();

        resolveAll(page([asset('stale', 'STALE.jpg')]));
        await waitFor(() => expect(screen.queryByRole('button', { name: 'Select STALE.jpg' })).not.toBeInTheDocument());

        await fireEvent.scroll(screen.getByRole('list', { name: 'Apple Photos assets' }));
        expect(await screen.findByRole('button', { name: 'Select B-2.jpg' })).toBeInTheDocument();
        expect(catalog.listAssets).toHaveBeenLastCalledWith('album-b', 1, 100);
        expect(screen.getByText('All photos loaded.')).toBeInTheDocument();
    });

    it('deduplicates overlapping pages, advances by consumed results, and prevents concurrent scroll loads', async () => {
        let resolveSecondPage!: (value: ApplePhotosPage<ApplePhotosAsset>) => void;
        const secondPage = new Promise<ApplePhotosPage<ApplePhotosAsset>>(resolve => { resolveSecondPage = resolve; });
        const catalog = client({
            listAssets: vi.fn()
                .mockResolvedValueOnce({
                    items: [asset('asset-1', 'One.jpg'), asset('asset-2', 'Two.jpg')],
                    offset: 0,
                    total: 5,
                    has_more: true,
                })
                .mockImplementationOnce(() => secondPage)
                .mockResolvedValueOnce({
                    items: [asset('asset-4', 'Four.jpg')],
                    offset: 4,
                    total: 5,
                    has_more: false,
                }),
        });
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        expect(await screen.findByRole('button', { name: 'Select Two.jpg' })).toBeInTheDocument();
        const list = screen.getByRole('list', { name: 'Apple Photos assets' });
        await fireEvent.scroll(list);
        await fireEvent.scroll(list);

        expect(screen.getByText('Loading more photos…')).toBeInTheDocument();
        expect(catalog.listAssets).toHaveBeenCalledTimes(2);

        resolveSecondPage({
            items: [asset('asset-2', 'Two.jpg'), asset('asset-3', 'Three.jpg')],
            offset: 2,
            total: 5,
            has_more: true,
        });
        expect(await screen.findByRole('button', { name: 'Select Three.jpg' })).toBeInTheDocument();
        expect(screen.getAllByRole('button', { name: 'Select Two.jpg' })).toHaveLength(1);

        await fireEvent.scroll(list);
        expect(await screen.findByRole('button', { name: 'Select Four.jpg' })).toBeInTheDocument();
        expect(catalog.listAssets).toHaveBeenLastCalledWith(null, 4, 100);
        expect(screen.getByText('End of library reached · 4 unique photos shown.')).toBeInTheDocument();
    });

    it('retries a failed incremental page without clearing the visible grid', async () => {
        const catalog = client({
            listAssets: vi.fn()
                .mockResolvedValueOnce({ items: [asset('asset-1', 'One.jpg')], offset: 0, total: 2, has_more: true })
                .mockRejectedValueOnce(new Error('Photos unavailable'))
                .mockResolvedValueOnce({ items: [asset('asset-2', 'Two.jpg')], offset: 1, total: 2, has_more: false }),
        });
        const user = userEvent.setup();
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        expect(await screen.findByRole('button', { name: 'Select One.jpg' })).toBeInTheDocument();
        await fireEvent.scroll(screen.getByRole('list', { name: 'Apple Photos assets' }));
        expect(await screen.findByText('More photos could not be loaded.')).toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'Select One.jpg' })).toBeInTheDocument();

        await user.click(screen.getByRole('button', { name: 'Retry' }));
        expect(await screen.findByRole('button', { name: 'Select Two.jpg' })).toBeInTheDocument();
        expect(catalog.listAssets).toHaveBeenLastCalledWith(null, 1, 100);
    });

    it('loads a local preview only when its fixed-size tile becomes visible', async () => {
        const visibilityCallbacks: IntersectionObserverCallback[] = [];
        vi.stubGlobal('IntersectionObserver', class {
            root = null;
            rootMargin = '240px';
            thresholds = [0];

            constructor(callback: IntersectionObserverCallback) {
                visibilityCallbacks.push(callback);
            }

            observe() {}
            unobserve() {}
            disconnect() {}
            takeRecords(): IntersectionObserverEntry[] { return []; }
        });
        const preview = 'data:image/png;base64,cHJldmlldw==';
        const catalog = client({
            listAssets: vi.fn().mockResolvedValue(page([asset('asset-1', 'One.jpg')])),
            loadPreview: vi.fn().mockResolvedValue(preview),
        });
        const view = render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        expect(await screen.findByLabelText('Preview unavailable locally')).toBeInTheDocument();
        expect(catalog.loadPreview).not.toHaveBeenCalled();
        const tile = screen.getByRole('button', { name: 'Select One.jpg' });
        for (const callback of visibilityCallbacks) {
            callback(
                [{ isIntersecting: true, target: tile } as unknown as IntersectionObserverEntry],
                {} as IntersectionObserver,
            );
        }

        await waitFor(() => expect(view.container.querySelector('img')).not.toBeNull());
        const image = view.container.querySelector('img');
        expect(image).toHaveAttribute('src', preview);
        expect(catalog.loadPreview).toHaveBeenCalledWith('asset-1', 512);
    });

    it('continues from the end sentinel when an overlapping page adds no grid rows', async () => {
        const visibilityCallbacks: IntersectionObserverCallback[] = [];
        vi.stubGlobal('IntersectionObserver', class {
            root = null;
            rootMargin = '240px';
            thresholds = [0];

            constructor(callback: IntersectionObserverCallback) {
                visibilityCallbacks.push(callback);
            }

            observe() {}
            unobserve() {}
            disconnect() {}
            takeRecords(): IntersectionObserverEntry[] { return []; }
        });
        const catalog = client({
            listAssets: vi.fn()
                .mockResolvedValueOnce({ items: [asset('asset-1', 'One.jpg')], offset: 0, total: 3, has_more: true })
                .mockResolvedValueOnce({ items: [asset('asset-1', 'One.jpg')], offset: 1, total: 3, has_more: true })
                .mockResolvedValueOnce({ items: [asset('asset-3', 'Three.jpg')], offset: 2, total: 3, has_more: false }),
        });
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        expect(await screen.findByRole('button', { name: 'Select One.jpg' })).toBeInTheDocument();
        const revealEnd = () => {
            const tile = screen.getByRole('button', { name: 'Select One.jpg' });
            for (const callback of visibilityCallbacks) {
                callback(
                    [{ isIntersecting: true, target: tile } as unknown as IntersectionObserverEntry],
                    {} as IntersectionObserver,
                );
            }
        };

        revealEnd();
        await waitFor(() => expect(catalog.listAssets).toHaveBeenCalledTimes(2));
        expect(screen.getAllByRole('button', { name: 'Select One.jpg' })).toHaveLength(1);

        revealEnd();
        expect(await screen.findByRole('button', { name: 'Select Three.jpg' })).toBeInTheDocument();
        expect(catalog.listAssets).toHaveBeenLastCalledWith(null, 2, 100);
        expect(screen.getByText('End of library reached · 2 unique photos shown.')).toBeInTheDocument();
    });

    it('shares an in-flight preview when the same asset appears after an album switch', async () => {
        const visibilityCallbacks: IntersectionObserverCallback[] = [];
        vi.stubGlobal('IntersectionObserver', class {
            root = null;
            rootMargin = '240px';
            thresholds = [0];

            constructor(callback: IntersectionObserverCallback) {
                visibilityCallbacks.push(callback);
            }

            observe() {}
            unobserve() {}
            disconnect() {}
            takeRecords(): IntersectionObserverEntry[] { return []; }
        });
        let resolvePreview!: (value: string) => void;
        const previewRequest = new Promise<string>(resolve => { resolvePreview = resolve; });
        const catalog = client({
            listAlbums: vi.fn().mockResolvedValue(page([album('shared', 'Shared album')])),
            listAssets: vi.fn().mockResolvedValue(page([asset('same-asset', 'Same.jpg')])),
            loadPreview: vi.fn().mockReturnValue(previewRequest),
        });
        const user = userEvent.setup();
        const view = render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        const reveal = (target: Element) => {
            for (const callback of visibilityCallbacks) {
                callback(
                    [{ isIntersecting: true, target } as unknown as IntersectionObserverEntry],
                    {} as IntersectionObserver,
                );
            }
        };
        reveal(await screen.findByRole('button', { name: 'Select Same.jpg' }));
        expect(catalog.loadPreview).toHaveBeenCalledOnce();

        await user.click(screen.getByRole('button', { name: 'Shared album' }));
        await waitFor(() => expect(catalog.listAssets).toHaveBeenCalledTimes(2));
        reveal(screen.getByRole('button', { name: 'Select Same.jpg' }));
        expect(catalog.loadPreview).toHaveBeenCalledOnce();

        resolvePreview('data:image/png;base64,c2hhcmVk');
        await waitFor(() => expect(view.container.querySelector('img')).not.toBeNull());
    });

    it('reserves measured grid height from the active preview scale', async () => {
        let resize!: (width: number) => void;
        vi.stubGlobal('ResizeObserver', class {
            constructor(callback: ResizeObserverCallback) {
                resize = (width: number) => callback(
                    [{ contentRect: { width } } as unknown as ResizeObserverEntry],
                    this as unknown as ResizeObserver,
                );
            }

            observe() {}
            unobserve() {}
            disconnect() {}
        });
        const catalog = client({
            listAssets: vi.fn().mockResolvedValue(page(
                Array.from({ length: 12 }, (_, index) => asset(`asset-${index}`, `${index}.jpg`)),
            )),
        });
        const view = render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        expect(await screen.findByRole('button', { name: 'Select 11.jpg' })).toBeInTheDocument();
        resize(528);

        await waitFor(() => {
            expect(view.container.querySelector('.date-group')).toHaveStyle(
                'contain-intrinsic-size: 422px',
            );
        });

        await fireEvent.input(screen.getByRole('slider', { name: 'Preview size' }), { target: { value: '100' } });
        await waitFor(() => {
            expect(view.container.querySelector('.date-group')).toHaveStyle(
                'contain-intrinsic-size: 1628px',
            );
        });
    });

    it('shows recovery guidance for denied access and closes through the shared modal', async () => {
        const onclose = vi.fn();
        const catalog = client({ authorizationStatus: vi.fn().mockResolvedValue('denied') });
        const user = userEvent.setup();
        render(ApplePhotosCatalogDialog, { onclose, client: catalog });

        expect(await screen.findByText(/System Settings.*Privacy & Security.*Photos/i)).toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Close Apple Photos' }));
        expect(onclose).toHaveBeenCalledOnce();
    });

    it.each(['restricted', 'unsupported'] as const)('renders the %s platform state', async (status) => {
        const catalog = client({ authorizationStatus: vi.fn().mockResolvedValue(status) });
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        if (status === 'restricted') {
            expect(await screen.findByText(/System Settings.*Privacy & Security.*Photos/i)).toBeInTheDocument();
        } else {
            expect(await screen.findByText('This catalog source is not supported on the current platform.')).toBeInTheDocument();
        }
        expect(catalog.requestAuthorization).not.toHaveBeenCalled();
    });

    it('loads another bounded album page when the library has more than 100 albums', async () => {
        const firstAlbums = Array.from({ length: 100 }, (_, index) => album(`album-${index}`, `Album ${index}`));
        const catalog = client({
            listAlbums: vi.fn()
                .mockResolvedValueOnce(page(firstAlbums, 0, 101))
                .mockResolvedValueOnce(page([album('album-last', 'Last album')], 100, 101)),
        });
        const user = userEvent.setup();
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        await user.click(await screen.findByRole('button', { name: 'Load more albums' }));

        expect(await screen.findByRole('option', { name: 'Last album' })).toBeInTheDocument();
        expect(catalog.listAlbums).toHaveBeenLastCalledWith(100, 100);
    });

    it('keeps an asset error visible when a later album page succeeds', async () => {
        const firstAlbums = Array.from({ length: 100 }, (_, index) => album(`album-${index}`, `Album ${index}`));
        const catalog = client({
            listAlbums: vi.fn()
                .mockResolvedValueOnce(page(firstAlbums, 0, 101))
                .mockResolvedValueOnce(page([album('album-last', 'Last album')], 100, 101)),
            listAssets: vi.fn().mockRejectedValue(new Error('Asset catalog unavailable')),
        });
        const user = userEvent.setup();
        render(ApplePhotosCatalogDialog, { onclose: vi.fn(), client: catalog });

        expect(await screen.findByText('Photos: Asset catalog unavailable')).toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Load more albums' }));

        expect(await screen.findByRole('option', { name: 'Last album' })).toBeInTheDocument();
        expect(screen.getByText('Photos: Asset catalog unavailable')).toBeInTheDocument();
    });

    it('ignores authorization completion after the dialog has closed', async () => {
        let resolveAuthorization!: (status: 'authorized') => void;
        const authorization = new Promise<'authorized'>(resolve => { resolveAuthorization = resolve; });
        const catalog = client({ authorizationStatus: vi.fn(() => authorization) });
        const onclose = vi.fn();
        const user = userEvent.setup();
        const view = render(ApplePhotosCatalogDialog, { onclose, client: catalog });

        await user.click(screen.getByRole('button', { name: 'Close Apple Photos' }));
        view.unmount();
        resolveAuthorization('authorized');
        await Promise.resolve();
        await Promise.resolve();

        expect(catalog.listAlbums).not.toHaveBeenCalled();
        expect(catalog.listAssets).not.toHaveBeenCalled();
    });
});
