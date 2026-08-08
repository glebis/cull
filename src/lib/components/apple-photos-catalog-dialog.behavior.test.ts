// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ApplePhotosCatalogDialog from './ApplePhotosCatalogDialog.svelte';
import type {
    ApplePhotosAlbum,
    ApplePhotosAsset,
    ApplePhotosCatalogClient,
    ApplePhotosPage,
} from '$lib/apple-photos';

afterEach(() => cleanup());

function page<T>(items: T[], offset = 0, total = items.length): ApplePhotosPage<T> {
    return { items, offset, total, has_more: offset + items.length < total };
}

function album(id: string, title: string): ApplePhotosAlbum {
    return { id, title, kind: 'user' };
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
        expect(await screen.findByText('IMG_0001.HEIC')).toBeInTheDocument();
        expect(catalog.listAssets).toHaveBeenCalledWith(null, 0, 100);
    });

    it('switches albums, ignores a stale response, and appends a bounded next page', async () => {
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

        const selector = await screen.findByRole('combobox', { name: 'Album' });
        await user.selectOptions(selector, 'album-b');
        expect(await screen.findByText('B-1.jpg')).toBeInTheDocument();

        resolveAll(page([asset('stale', 'STALE.jpg')]));
        await waitFor(() => expect(screen.queryByText('STALE.jpg')).not.toBeInTheDocument());

        await user.click(screen.getByRole('button', { name: 'Load more' }));
        expect(await screen.findByText('B-2.jpg')).toBeInTheDocument();
        expect(catalog.listAssets).toHaveBeenLastCalledWith('album-b', 1, 100);
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
