// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import type { ReferencedSource } from '$lib/api';

const apiMocks = vi.hoisted(() => ({
    listReferencedSources: vi.fn(),
    listSourceFolders: vi.fn().mockResolvedValue([]),
    openReferencedFolder: vi.fn(),
}));

vi.mock('$lib/api', () => apiMocks);
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('$lib/image-loading', () => ({ loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined) }));

import DevicesSection from './DevicesSection.svelte';
import { referencedSources } from '$lib/referenced-sources';

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

        expect(await screen.findByRole('button', { name: /FUJIFILM SD/ })).toBeInTheDocument();
        expect(screen.getByTestId('devices-section')).toBeInTheDocument();
        expect(screen.queryByText('DEVICES')).not.toBeInTheDocument();
    });
});
