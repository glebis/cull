// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import type { ReferencedSource } from '$lib/api';

const apiMocks = vi.hoisted(() => ({
    listReferencedSources: vi.fn(),
    listSourceFolders: vi.fn().mockResolvedValue([]),
    openReferencedFolder: vi.fn(),
    cancelReferencedSourceJob: vi.fn().mockResolvedValue(true),
}));

vi.mock('$lib/api', () => apiMocks);
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('$lib/image-loading', () => ({ loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined) }));

import DevicesSection from './DevicesSection.svelte';
import { referencedSources } from '$lib/referenced-sources';
import { activeReferencedFolder } from '$lib/stores';

const connectedSource: ReferencedSource = {
    id: 'source-connected',
    platform_volume_id: 'volume-1',
    display_name: 'FUJIFILM SD',
    last_mount_path: '/Volumes/FUJIFILM SD',
    source_kind: 'sd_card',
    capacity_bytes: 64_000_000_000,
    recursive_default: false,
    settings_json: '{}',
    last_seen_at: '2026-08-30T10:00:00Z',
    offline_at: null,
};

afterEach(() => cleanup());
beforeEach(() => {
    vi.clearAllMocks();
    referencedSources.set([]);
    apiMocks.listReferencedSources.mockResolvedValue([]);
});

describe('DevicesSection visibility', () => {
    it('does not occupy sidebar space when no device is connected', async () => {
        render(DevicesSection);

        await waitFor(() => expect(apiMocks.listReferencedSources).toHaveBeenCalledOnce());
        expect(screen.queryByTestId('devices-section')).not.toBeInTheDocument();
    });

    it('does not show a remembered source while its device is offline', async () => {
        apiMocks.listReferencedSources.mockResolvedValue([
            { ...connectedSource, offline_at: '2026-08-30T10:05:00Z' },
        ]);
        render(DevicesSection);

        await waitFor(() => expect(apiMocks.listReferencedSources).toHaveBeenCalledOnce());
        expect(screen.queryByTestId('devices-section')).not.toBeInTheDocument();
    });

    it('shows a connected device without a redundant section heading', async () => {
        apiMocks.listReferencedSources.mockResolvedValue([connectedSource]);
        render(DevicesSection);

        expect(await screen.findByRole('button', { name: /^FUJIFILM SD/ })).toBeInTheDocument();
        expect(screen.getByTestId('devices-section')).toBeInTheDocument();
        expect(screen.queryByText('DEVICES')).not.toBeInTheDocument();
    });

    it('opens the folder actions from right-click or the visible affordance without importing on left-click', async () => {
        apiMocks.listReferencedSources.mockResolvedValue([connectedSource]);
        apiMocks.listSourceFolders.mockResolvedValue(['653_FUJI']);
        apiMocks.openReferencedFolder.mockImplementation(({ relative_path }: { relative_path: string }) => Promise.resolve({
            job_id: `job-${relative_path || 'root'}`,
            source_id: connectedSource.id,
            relative_path,
            requested_paths: [],
            image_ids: [],
            discovered_count: 0,
            next_cursor: null,
            indexing: false,
        }));
        const onimportfolder = vi.fn();
        const onrevealfolder = vi.fn();
        const oncopypath = vi.fn();
        render(DevicesSection, { onimportfolder, onrevealfolder, oncopypath });

        await fireEvent.click(await screen.findByRole('button', { name: /^FUJIFILM SD/ }));
        const folder = await screen.findByRole('button', { name: '653_FUJI' });
        expect(screen.getByRole('button', { name: 'Actions for 653_FUJI' })).toBeVisible();
        expect(onimportfolder).not.toHaveBeenCalled();

        await fireEvent.contextMenu(folder);
        expect(await screen.findByRole('menuitem', { name: 'Open Folder' })).toBeInTheDocument();
        expect(screen.getByRole('menuitem', { name: 'Reveal in Finder' })).toBeInTheDocument();
        expect(screen.getByRole('menuitem', { name: 'Import Folder…' })).toBeInTheDocument();
        expect(screen.getByRole('menuitem', { name: 'Copy Path' })).toBeInTheDocument();
        expect(onimportfolder).not.toHaveBeenCalled();

        await fireEvent.click(screen.getByRole('menuitem', { name: 'Open Folder' }));
        await waitFor(() => expect(get(activeReferencedFolder)?.relative_path).toBe('653_FUJI'));
        expect(onimportfolder).not.toHaveBeenCalled();

        await fireEvent.contextMenu(folder);
        await fireEvent.click(screen.getByRole('menuitem', { name: 'Import Folder…' }));
        expect(onimportfolder).toHaveBeenCalledWith('/Volumes/FUJIFILM SD/653_FUJI');
    });
});
