// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';

const mocks = vi.hoisted(() => ({
    getEmbeddingCountForScope: vi.fn(),
    getEmbeddingPageForScope: vi.fn(),
    getImageCountForScope: vi.fn(),
    listImageIdsForScope: vi.fn(),
    getImagesByIds: vi.fn(),
    startModelEmbeddingGeneration: vi.fn(),
    cancelJob: vi.fn(),
    loadEmbeddingNeighbors: vi.fn(),
    eventListeners: new Map<string, Set<(event: { payload: Record<string, unknown> }) => void>>(),
    workerMessages: [] as Array<Record<string, unknown>>,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockImplementation(async (
        eventName: string,
        callback: (event: { payload: Record<string, unknown> }) => void,
    ) => {
        const listeners = mocks.eventListeners.get(eventName) ?? new Set();
        listeners.add(callback);
        mocks.eventListeners.set(eventName, listeners);
        return () => listeners.delete(callback);
    }),
}));

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: vi.fn((path: string) => path),
}));

vi.mock('$lib/settings-navigation', () => ({ openSettings: vi.fn() }));

vi.mock('$lib/view-utils', () => ({
    isAssetProtocolSafePath: vi.fn(() => true),
    safeAssetPreviewPath: vi.fn(() => null),
}));

vi.mock('$lib/embedding-scope', () => ({
    getEmbeddingCountForScope: mocks.getEmbeddingCountForScope,
    getEmbeddingPageForScope: mocks.getEmbeddingPageForScope,
    getImageCountForScope: mocks.getImageCountForScope,
    listImageIdsForScope: mocks.listImageIdsForScope,
}));

vi.mock('$lib/embedding-neighbors', () => ({
    loadEmbeddingNeighbors: mocks.loadEmbeddingNeighbors,
}));

vi.mock('$lib/api', () => ({
    isEmbeddingModelAvailable: vi.fn().mockResolvedValue(true),
    getEmbeddingModelDownloadInfo: vi.fn().mockResolvedValue(null),
    listEmbeddingProviders: vi.fn().mockResolvedValue([]),
    downloadEmbeddingModel: vi.fn(),
    startModelEmbeddingGeneration: mocks.startModelEmbeddingGeneration,
    hasApiKey: vi.fn().mockResolvedValue(false),
    getImagesByIds: mocks.getImagesByIds,
    getGenerationRun: vi.fn().mockResolvedValue(null),
    regenerateThumbnails: vi.fn(),
    cancelJob: mocks.cancelJob,
    pauseJob: vi.fn(),
    resumeJob: vi.fn(),
}));

import EmbeddingExplorer from './EmbeddingExplorer.svelte';
import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSmartCollection,
    importBatchFilter,
    minSizeFilter,
    showRejected,
    embeddingViewState,
    focusedImageOverride,
    viewMode,
} from '$lib/stores';

class FakeWorker {
    private listeners = new Map<string, Set<EventListener>>();

    addEventListener(type: string, listener: EventListener) {
        const listeners = this.listeners.get(type) ?? new Set<EventListener>();
        listeners.add(listener);
        this.listeners.set(type, listeners);
    }

    removeEventListener(type: string, listener: EventListener) {
        this.listeners.get(type)?.delete(listener);
    }

    postMessage(message: Record<string, unknown>) {
        mocks.workerMessages.push(message);
        queueMicrotask(() => {
            const ids = message.ids as string[];
            const event = new MessageEvent('message', {
                data: {
                    requestId: message.requestId,
                    points: ids.map((id, index) => ({ id, x: index, y: index, cluster: 0 })),
                    clusters: [{
                        id: 0,
                        label: 'Cluster 1',
                        count: ids.length,
                        colorIndex: 0,
                        previewPaths: [],
                        x: 0,
                        y: 0,
                    }],
                    projectionKey: `clip:${ids.join(',')}`,
                },
            });
            for (const listener of this.listeners.get('message') ?? []) listener(event);
        });
    }

    terminate() {}
}

function image(id: string) {
    return {
        image: {
            id,
            sha256_hash: id,
            width: 100,
            height: 100,
            format: 'png',
            file_size: 10,
            created_at: '2026-08-08T00:00:00Z',
            imported_at: '2026-08-08T00:00:00Z',
            ai_prompt: null,
            raw_metadata: null,
        },
        path: `/photos/${id}.png`,
        thumbnail_path: null,
        selection: null,
        source_label: null,
        missing_at: null,
    };
}

function emit(eventName: string, payload: Record<string, unknown>) {
    for (const listener of mocks.eventListeners.get(eventName) ?? []) listener({ payload });
}

afterEach(() => cleanup());

beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mocks.eventListeners.clear();
    mocks.workerMessages.length = 0;
    activeFolder.set('/photos/scoped');
    activeCollection.set(null);
    activeSmartCollection.set(null);
    activeDetectedClass.set(null);
    importBatchFilter.set(null);
    minSizeFilter.set(512);
    showRejected.set(false);
    embeddingViewState.update(state => ({ ...state, provider: 'clip' }));
    focusedImageOverride.set(null);
    viewMode.set('grid');

    mocks.listImageIdsForScope.mockResolvedValue(['in-a', 'in-b']);
    mocks.getImageCountForScope.mockResolvedValue(2);
    mocks.getEmbeddingCountForScope.mockResolvedValue(2);
    mocks.getEmbeddingPageForScope.mockResolvedValue({
        ids: ['in-a', 'in-b'],
        vectors: [10, 11, 20, 21],
        dims: 2,
        total: 2,
        offset: 0,
        limit: 5000,
        has_more: false,
    });
    mocks.getImagesByIds.mockResolvedValue([image('in-b'), image('in-a')]);
    mocks.startModelEmbeddingGeneration.mockResolvedValue({
        job_id: 'job_embed_1',
        total: 1,
        model: 'clip-vit-b32',
        mode: 'missing',
    });
    mocks.loadEmbeddingNeighbors.mockResolvedValue([
        { image: image('near-one'), score: 0.934 },
    ]);

    vi.stubGlobal('Worker', FakeWorker);
    vi.stubGlobal('ResizeObserver', class {
        observe() {}
        unobserve() {}
        disconnect() {}
    });
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
        clearRect: vi.fn(),
        fillRect: vi.fn(),
        beginPath: vi.fn(),
        arc: vi.fn(),
        fill: vi.fn(),
        stroke: vi.fn(),
        save: vi.fn(),
        restore: vi.fn(),
        translate: vi.fn(),
        scale: vi.fn(),
        moveTo: vi.fn(),
        lineTo: vi.fn(),
        drawImage: vi.fn(),
        measureText: vi.fn(() => ({ width: 20 })),
    } as unknown as CanvasRenderingContext2D);
});

describe('Embedding Explorer library scope', () => {
    it('projects the full active scope and keeps embedding vectors paired with their IDs', async () => {
        render(EmbeddingExplorer);

        const folderScope = {
            type: 'folder',
            path: '/photos/scoped',
            min_size: 512,
            include_rejected: false,
        };
        await waitFor(() => expect(mocks.getEmbeddingPageForScope).toHaveBeenCalledWith(
            folderScope,
            'clip-vit-b32',
            5000,
            0,
        ));
        expect(mocks.getImageCountForScope).toHaveBeenCalledWith(folderScope);

        await waitFor(() => expect(mocks.workerMessages).toHaveLength(1));
        expect(mocks.workerMessages[0].ids).toEqual(['in-a', 'in-b']);
        expect(Array.from(mocks.workerMessages[0].vectors as Float32Array)).toEqual([10, 11, 20, 21]);
        expect((mocks.workerMessages[0].images as Array<{ id: string }>).map(item => item.id)).toEqual([
            'in-b',
            'in-a',
        ]);

        activeCollection.set('collection-2');
        await waitFor(() => expect(mocks.getEmbeddingPageForScope).toHaveBeenCalledWith(
            { type: 'collection', id: 'collection-2', include_rejected: false },
            'clip-vit-b32',
            5000,
            0,
        ));
    });

    it('offers missing and full regeneration, reports job progress, and cancels the exact job', async () => {
        mocks.getEmbeddingCountForScope.mockResolvedValue(1);
        const user = userEvent.setup();
        render(EmbeddingExplorer);

        await user.click(await screen.findByRole('button', { name: 'Generate missing (1)' }));
        await waitFor(() => expect(mocks.startModelEmbeddingGeneration).toHaveBeenCalledWith(
            'clip-vit-b32',
            ['in-a', 'in-b'],
            'missing',
        ));

        emit('embedding-progress', {
            job_id: 'job_embed_1',
            model: 'clip-vit-b32',
            mode: 'missing',
            status: 'running',
            current: 1,
            total: 2,
        });
        const progress = await screen.findByRole('progressbar', { name: 'CLIP embedding progress' });
        expect(progress).toHaveAttribute('aria-valuenow', '1');
        expect(progress).toHaveAttribute('aria-valuemax', '2');

        await user.click(screen.getByRole('button', { name: 'Cancel embedding generation' }));
        expect(mocks.cancelJob).toHaveBeenCalledWith('job_embed_1');
        expect(screen.getByRole('button', { name: 'Cancelling embedding generation' })).toBeDisabled();

        emit('embedding-progress', {
            job_id: 'job_embed_1',
            model: 'clip-vit-b32',
            mode: 'missing',
            status: 'cancelled',
            current: 1,
            total: 2,
        });
        await waitFor(() => expect(screen.getByText('Generation cancelled at 1/2')).toBeInTheDocument());

        mocks.startModelEmbeddingGeneration.mockResolvedValue({
            job_id: 'job_embed_2',
            total: 2,
            model: 'clip-vit-b32',
            mode: 'all',
        });
        await user.click(screen.getByRole('button', { name: 'Regenerate all (2)' }));
        expect(mocks.startModelEmbeddingGeneration).toHaveBeenLastCalledWith(
            'clip-vit-b32',
            ['in-a', 'in-b'],
            'all',
        );
    });

    it('does not let generation from an old scope overwrite the newly selected scope', async () => {
        mocks.getEmbeddingCountForScope.mockImplementation(
            (scope: { type: string }) => Promise.resolve(scope.type === 'collection' ? 0 : 1),
        );

        const user = userEvent.setup();
        const { container } = render(EmbeddingExplorer);
        await user.click(await screen.findByRole('button', { name: 'Generate missing (1)' }));
        await waitFor(() => expect(mocks.startModelEmbeddingGeneration).toHaveBeenCalledOnce());

        activeCollection.set('new-collection');
        await waitFor(() => {
            const embeddingRow = [...container.querySelectorAll('.stat-row')]
                .find(row => row.querySelector('.stat-label')?.textContent === 'Embeddings');
            expect(embeddingRow?.querySelector('.stat-value')).toHaveTextContent('0');
        });

        emit('embedding-progress', {
            job_id: 'job_embed_1',
            model: 'clip-vit-b32',
            mode: 'missing',
            status: 'completed',
            current: 1,
            total: 1,
        });
        await waitFor(() => expect(screen.getByText('Generation completed: 1/1')).toBeInTheDocument());
        const embeddingRow = [...container.querySelectorAll('.stat-row')]
            .find(row => row.querySelector('.stat-label')?.textContent === 'Embeddings');
        expect(embeddingRow?.querySelector('.stat-value')).toHaveTextContent('0');
    });

    it('keeps an active job attributed to its model while another model shows its own missing count', async () => {
        mocks.getEmbeddingCountForScope.mockImplementation(
            (_scope: unknown, model: string) => Promise.resolve(model === 'clip-vit-b32' ? 1 : 0),
        );
        const user = userEvent.setup();
        const { container } = render(EmbeddingExplorer);

        await user.click(await screen.findByRole('button', { name: 'Generate missing (1)' }));
        await waitFor(() => expect(mocks.startModelEmbeddingGeneration).toHaveBeenCalledOnce());
        emit('embedding-progress', {
            job_id: 'job_embed_1',
            model: 'clip-vit-b32',
            mode: 'missing',
            status: 'running',
            current: 1,
            total: 2,
        });

        await user.click(screen.getByRole('radio', { name: /DINOv2/i }));
        await waitFor(() => {
            const missingRow = [...container.querySelectorAll('.stat-row')]
                .find(row => row.querySelector('.stat-label')?.textContent === 'Need embeddings');
            expect(missingRow?.querySelector('.stat-value')).toHaveTextContent('2');
        });
        expect(screen.getByText('CLIP GENERATION')).toBeInTheDocument();
        expect(screen.getByRole('progressbar', { name: 'CLIP embedding progress' })).toHaveAttribute('aria-valuenow', '1');
    });

    it('loads ranked neighbors for the selected point in the current scope', async () => {
        const user = userEvent.setup();
        render(EmbeddingExplorer);
        await waitFor(() => expect(mocks.workerMessages).toHaveLength(1));

        const explorer = screen.getByRole('application', { name: 'Visual embeddings' });
        explorer.focus();
        await user.keyboard('{ArrowRight}');

        await waitFor(() => expect(mocks.loadEmbeddingNeighbors).toHaveBeenCalledWith(
            {
                type: 'folder',
                path: '/photos/scoped',
                min_size: 512,
                include_rejected: false,
            },
            'in-a',
            'clip-vit-b32',
            6,
        ));
        expect(await screen.findByText('near-one.png')).toBeInTheDocument();
        expect(screen.getByText('93.4%')).toBeInTheDocument();

        await user.click(screen.getByRole('button', {
            name: 'Open near-one.png in Loupe, similarity 93.4%',
        }));
        expect(get(viewMode)).toBe('loupe');
        expect(get(focusedImageOverride)?.image.id).toBe('near-one');
    });

});
