// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import type { ReferencedSource } from '$lib/api';

const apiMocks = vi.hoisted(() => ({
    getAppSetting: vi.fn().mockResolvedValue(null),
    listReferencedSources: vi.fn(),
    listSourceFolders: vi.fn().mockResolvedValue([]),
    openReferencedFolder: vi.fn(),
    cancelReferencedSourceJob: vi.fn().mockResolvedValue(true),
    setAppSetting: vi.fn().mockResolvedValue(undefined),
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
    apiMocks.getAppSetting.mockResolvedValue(null);
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

    it('opens folder actions from right-click or keyboard without visible action buttons', async () => {
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
        expect(screen.queryByRole('button', { name: /Actions for/ })).not.toBeInTheDocument();
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

        await fireEvent.keyDown(folder, { key: 'F10', shiftKey: true });
        expect(await screen.findByRole('menuitem', { name: 'Open Folder' })).toBeInTheDocument();
    });

    it('hides dotfolders by default and toggles them with Cmd+Shift+Period outside text input', async () => {
        let showHidden = false;
        apiMocks.listReferencedSources.mockResolvedValue([connectedSource]);
        apiMocks.listSourceFolders.mockImplementation(async () => showHidden ? ['.Trashes', '653_FUJI'] : ['653_FUJI']);
        apiMocks.openReferencedFolder.mockResolvedValue({
            job_id: 'job-root', source_id: connectedSource.id, relative_path: '', requested_paths: [],
            image_ids: [], discovered_count: 0, next_cursor: null, indexing: false,
        });
        apiMocks.setAppSetting.mockImplementation(async (key: string, value: string) => {
            if (key === 'show_hidden_files') showHidden = value === 'true';
        });
        render(DevicesSection);

        await fireEvent.click(await screen.findByRole('button', { name: /^FUJIFILM SD/ }));
        expect(await screen.findByRole('button', { name: '653_FUJI' })).toBeVisible();
        expect(screen.queryByRole('button', { name: '.Trashes' })).not.toBeInTheDocument();

        const input = document.createElement('input');
        document.body.append(input);
        await fireEvent.keyDown(input, { key: '.', code: 'Period', metaKey: true, shiftKey: true });
        expect(apiMocks.setAppSetting).not.toHaveBeenCalledWith('show_hidden_files', 'true');

        await fireEvent.keyDown(window, { key: '.', code: 'Period', metaKey: true, shiftKey: true });
        await waitFor(() => expect(apiMocks.setAppSetting).toHaveBeenCalledWith('show_hidden_files', 'true'));
        const hiddenFolder = await screen.findByRole('button', { name: '.Trashes' });
        expect(hiddenFolder).toBeVisible();

        await fireEvent.click(hiddenFolder);
        expect(get(activeReferencedFolder)?.relative_path).toBe('.Trashes');
        await fireEvent.keyDown(window, { key: '.', code: 'Period', metaKey: true, shiftKey: true });
        await waitFor(() => expect(apiMocks.setAppSetting).toHaveBeenCalledWith('show_hidden_files', 'false'));
        await waitFor(() => expect(get(activeReferencedFolder)?.relative_path).toBe(''));
        expect(screen.queryByRole('button', { name: '.Trashes' })).not.toBeInTheDocument();
        input.remove();
    });
});
