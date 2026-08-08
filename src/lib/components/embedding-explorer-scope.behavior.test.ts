// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, waitFor } from '@testing-library/svelte';

const mocks = vi.hoisted(() => ({
    getEmbeddingCountForScope: vi.fn(),
    getEmbeddingPageForScope: vi.fn(),
    listImageIdsForScope: vi.fn(),
    getImagesByIds: vi.fn(),
    workerMessages: [] as Array<Record<string, unknown>>,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockResolvedValue(vi.fn()),
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
    listImageIdsForScope: mocks.listImageIdsForScope,
}));

vi.mock('$lib/api', () => ({
    isEmbeddingModelAvailable: vi.fn().mockResolvedValue(true),
    getEmbeddingModelDownloadInfo: vi.fn().mockResolvedValue(null),
    listEmbeddingProviders: vi.fn().mockResolvedValue([]),
    downloadEmbeddingModel: vi.fn(),
    generateModelEmbeddings: vi.fn(),
    hasApiKey: vi.fn().mockResolvedValue(false),
    getImagesByIds: mocks.getImagesByIds,
    getGenerationRun: vi.fn().mockResolvedValue(null),
    regenerateThumbnails: vi.fn(),
    cancelJob: vi.fn(),
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

afterEach(() => cleanup());

beforeEach(() => {
    vi.clearAllMocks();
    mocks.workerMessages.length = 0;
    activeFolder.set('/photos/scoped');
    activeCollection.set(null);
    activeSmartCollection.set(null);
    activeDetectedClass.set(null);
    importBatchFilter.set(null);
    minSizeFilter.set(512);
    showRejected.set(false);

    mocks.listImageIdsForScope.mockResolvedValue(['in-a', 'in-b']);
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
        expect(mocks.listImageIdsForScope).toHaveBeenCalledWith(folderScope);

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
});
