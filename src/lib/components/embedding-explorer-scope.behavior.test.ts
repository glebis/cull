// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { get } from 'svelte/store';

const mocks = vi.hoisted(() => ({
    getEmbeddingCountForScope: vi.fn(),
    getEmbeddingPageForScope: vi.fn(),
    getImageCountForScope: vi.fn(),
    listImageIdsForScope: vi.fn(),
    getImagesByIds: vi.fn(),
    startModelEmbeddingGeneration: vi.fn(),
    cancelJob: vi.fn(),
    setApiKey: vi.fn(),
    validateApiKey: vi.fn(),
    getOllamaEmbeddingConfig: vi.fn(),
    setOllamaEmbeddingConfig: vi.fn(),
    isEmbeddingModelAvailable: vi.fn(),
    listEmbeddingProviders: vi.fn(),
    loadEmbeddingNeighbors: vi.fn(),
    createCollectionWithImages: vi.fn(),
    listCollections: vi.fn(),
    nameEmbeddingClusters: vi.fn(),
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
    isEmbeddingModelAvailable: mocks.isEmbeddingModelAvailable,
    getEmbeddingModelDownloadInfo: vi.fn().mockResolvedValue(null),
    listEmbeddingProviders: mocks.listEmbeddingProviders,
    downloadEmbeddingModel: vi.fn(),
    startModelEmbeddingGeneration: mocks.startModelEmbeddingGeneration,
    hasApiKey: vi.fn().mockResolvedValue(false),
    setApiKey: mocks.setApiKey,
    validateApiKey: mocks.validateApiKey,
    getOllamaEmbeddingConfig: mocks.getOllamaEmbeddingConfig,
    setOllamaEmbeddingConfig: mocks.setOllamaEmbeddingConfig,
    getImagesByIds: mocks.getImagesByIds,
    getGenerationRun: vi.fn().mockResolvedValue(null),
    regenerateThumbnails: vi.fn(),
    cancelJob: mocks.cancelJob,
    pauseJob: vi.fn(),
    resumeJob: vi.fn(),
    createCollectionWithImages: mocks.createCollectionWithImages,
    listCollections: mocks.listCollections,
    nameEmbeddingClusters: mocks.nameEmbeddingClusters,
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
    collections,
    resolveTextInputDialog,
    settingsOpen,
    textInputDialog,
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
    settingsOpen.set(false);
    embeddingViewState.set({
        panX: 0,
        panY: 0,
        scale: 1,
        selectedPointId: null,
        highlightedCluster: null,
        provider: 'clip',
        projectionKey: null,
        hasUserView: false,
        interactionMode: 'map',
        zPreset: 'cluster',
        activeZLayerKey: null,
        focusActiveLayer: false,
        largePreviewOpen: true,
        textOutputOpen: false,
        canvasLabelsOpen: false,
        spacePreset: 'balanced',
        spaceSpacing: 1,
        spaceDepth: 0.35,
        spaceScale: 1,
        spacePerspective: 0.3,
    });
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
    mocks.validateApiKey.mockResolvedValue(true);
    mocks.getOllamaEmbeddingConfig.mockResolvedValue(['http://localhost:11434/api/embed', 'embeddinggemma']);
    mocks.isEmbeddingModelAvailable.mockResolvedValue(true);
    mocks.listEmbeddingProviders.mockResolvedValue([]);
    mocks.createCollectionWithImages.mockResolvedValue('collection-from-map');
    mocks.listCollections.mockResolvedValue([['collection-from-map', 'Map picks', 1]]);
    mocks.nameEmbeddingClusters.mockResolvedValue([]);
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
        strokeRect: vi.fn(),
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
    it('keeps provider configuration hidden until the gear is opened', async () => {
        mocks.getEmbeddingCountForScope.mockResolvedValue(1);
        const user = userEvent.setup();
        const { container } = render(EmbeddingExplorer);

        expect(await screen.findByText('CLIP ViT-B/32')).toBeInTheDocument();
        expect(await screen.findByText('1/2 images embedded')).toBeInTheDocument();
        expect([...container.querySelectorAll('.stat-label')]).toHaveLength(0);
        expect(screen.queryByRole('combobox', { name: 'Embedding provider' })).not.toBeInTheDocument();
        expect(screen.queryByRole('button', { name: 'Regenerate all (2)' })).not.toBeInTheDocument();

        await user.click(screen.getByRole('button', { name: 'Configure embedding model' }));
        const provider = screen.getByRole('combobox', { name: 'Embedding provider' });
        expect(provider).toBeInTheDocument();
        expect([...container.querySelectorAll('.stat-label')].map(label => label.textContent)).toEqual([
            'Images', 'Embeddings', 'Need embeddings', 'Model',
        ]);
        expect(screen.getByRole('button', { name: 'Regenerate all (2)' })).toBeInTheDocument();

        await user.selectOptions(provider, 'gemini');
        const keyInput = await screen.findByLabelText('Gemini API key');
        expect(screen.queryByLabelText('OpenAI API key')).not.toBeInTheDocument();
        await user.type(keyInput, 'google-secret');
        await user.click(screen.getByRole('button', { name: 'Save API key' }));
        await waitFor(() => expect(mocks.validateApiKey).toHaveBeenCalledWith('google', 'google-secret'));
        expect(mocks.setApiKey).toHaveBeenCalledWith('google', 'google-secret');
        expect(await screen.findByText('Configuration saved.')).toBeInTheDocument();

        await user.selectOptions(provider, 'ollama');
        const ollamaUrl = await screen.findByLabelText('Ollama embedding URL');
        const ollamaModel = screen.getByLabelText('Ollama embedding model');
        await user.clear(ollamaUrl);
        await user.type(ollamaUrl, 'http://localhost:11434/api/embed-v2');
        await user.clear(ollamaModel);
        await user.type(ollamaModel, 'nomic-embed-text');
        mocks.listEmbeddingProviders.mockResolvedValue([{
            id: 'ollama',
            label: 'Ollama · nomic-embed-text',
            shortLabel: 'Ollama',
            modelName: 'ollama:nomic-embed-text',
            dimensions: 0,
            dimensionsLabel: 'model',
            scope: 'local',
            runtime: 'ollama',
            status: 'offline',
            available: false,
            downloadable: false,
            downloadLabel: null,
            expectedSha256: null,
            expectedSizeBytes: null,
            spdxLicense: null,
            sourceRepo: null,
            modelCardUrl: null,
            apiKeyProvider: null,
        }]);
        await user.click(screen.getByRole('button', { name: 'Save Ollama config' }));
        await waitFor(() => expect(mocks.setOllamaEmbeddingConfig).toHaveBeenCalledWith(
            'http://localhost:11434/api/embed-v2',
            'nomic-embed-text',
        ));
        await waitFor(() =>
            expect(container.querySelector('.model-summary-name')).toHaveTextContent(
                'Ollama · nomic-embed-text',
            ),
        );
        expect(screen.getByText('offline')).toBeInTheDocument();

        await user.click(screen.getByRole('button', { name: 'Close embedding configuration' }));
        expect(screen.queryByRole('combobox', { name: 'Embedding provider' })).not.toBeInTheDocument();
    });

    it('clears unsaved credentials synchronously when the provider changes', async () => {
        mocks.getEmbeddingCountForScope.mockResolvedValue(1);
        const user = userEvent.setup();
        render(EmbeddingExplorer);

        await user.click(await screen.findByRole('button', { name: 'Configure embedding model' }));
        const provider = screen.getByRole('combobox', { name: 'Embedding provider' });
        await user.selectOptions(provider, 'gemini');
        await user.type(await screen.findByLabelText('Gemini API key'), 'google-secret-not-saved');

        await user.selectOptions(provider, 'openai');
        expect(await screen.findByLabelText('OpenAI API key')).toHaveValue('');
        await user.click(screen.getByRole('button', { name: 'Save API key' }));
        expect(mocks.validateApiKey).not.toHaveBeenCalled();
        expect(mocks.setApiKey).not.toHaveBeenCalled();
    });

    it('ignores a delayed invalid-key result after switching providers', async () => {
        let resolveValidation!: (value: boolean) => void;
        mocks.validateApiKey.mockReturnValue(new Promise(resolve => {
            resolveValidation = resolve;
        }));
        const user = userEvent.setup();
        render(EmbeddingExplorer);

        await user.click(await screen.findByRole('button', { name: 'Configure embedding model' }));
        const provider = screen.getByRole('combobox', { name: 'Embedding provider' });
        await user.selectOptions(provider, 'gemini');
        await user.type(await screen.findByLabelText('Gemini API key'), 'pending-google-key');
        void user.click(screen.getByRole('button', { name: 'Save API key' }));
        await waitFor(() => expect(mocks.validateApiKey).toHaveBeenCalledWith('google', 'pending-google-key'));

        await user.selectOptions(provider, 'openai');
        resolveValidation(false);
        await waitFor(() => expect(screen.getByLabelText('OpenAI API key')).toHaveValue(''));
        expect(screen.queryByText('Enter a valid API key.')).not.toBeInTheDocument();
        expect(mocks.setApiKey).not.toHaveBeenCalled();
    });

    it('keeps Ollama configuration disabled until the current load resolves', async () => {
        let resolveConfig!: (value: [string, string]) => void;
        mocks.getOllamaEmbeddingConfig.mockReturnValue(new Promise(resolve => {
            resolveConfig = resolve;
        }));
        const user = userEvent.setup();
        render(EmbeddingExplorer);

        await user.click(await screen.findByRole('button', { name: 'Configure embedding model' }));
        await user.selectOptions(screen.getByRole('combobox', { name: 'Embedding provider' }), 'ollama');
        const urlInput = await screen.findByLabelText('Ollama embedding URL');
        expect(urlInput).toBeDisabled();
        expect(screen.getByRole('button', { name: 'Loading…' })).toBeDisabled();

        resolveConfig(['http://127.0.0.1:11434/api/embed', 'snowflake-arctic-embed']);
        await waitFor(() => expect(urlInput).toBeEnabled());
        expect(urlInput).toHaveValue('http://127.0.0.1:11434/api/embed');
        expect(screen.getByLabelText('Ollama embedding model')).toHaveValue('snowflake-arctic-embed');
    });

    it('saves explicit Ollama defaults when cleared fields are submitted', async () => {
        const user = userEvent.setup();
        render(EmbeddingExplorer);

        await user.click(await screen.findByRole('button', { name: 'Configure embedding model' }));
        await user.selectOptions(screen.getByRole('combobox', { name: 'Embedding provider' }), 'ollama');
        const urlInput = await screen.findByLabelText('Ollama embedding URL');
        const modelInput = screen.getByLabelText('Ollama embedding model');
        await user.clear(urlInput);
        await user.clear(modelInput);
        await user.click(screen.getByRole('button', { name: 'Save Ollama config' }));

        await waitFor(() => expect(mocks.setOllamaEmbeddingConfig).toHaveBeenCalledWith(
            'http://localhost:11434/api/embed',
            'embeddinggemma',
        ));
        expect(urlInput).toHaveValue('http://localhost:11434/api/embed');
        expect(modelInput).toHaveValue('embeddinggemma');
    });

    it('uses the newly saved Ollama model for generation', async () => {
        mocks.getEmbeddingCountForScope.mockResolvedValue(1);
        mocks.listEmbeddingProviders.mockResolvedValue([{
            id: 'ollama',
            label: 'Ollama · nomic-embed-text',
            shortLabel: 'Ollama',
            modelName: 'ollama:nomic-embed-text',
            dimensions: 0,
            dimensionsLabel: 'model',
            scope: 'local',
            runtime: 'ollama',
            status: 'ready',
            available: true,
            downloadable: false,
            downloadLabel: null,
            expectedSha256: null,
            expectedSizeBytes: null,
            spdxLicense: null,
            sourceRepo: null,
            modelCardUrl: null,
            apiKeyProvider: null,
        }]);
        mocks.startModelEmbeddingGeneration.mockResolvedValue({
            job_id: 'job_ollama_new_model',
            total: 1,
            model: 'ollama:nomic-embed-text',
            mode: 'missing',
        });
        const user = userEvent.setup();
        render(EmbeddingExplorer);

        await user.click(await screen.findByRole('button', { name: 'Configure embedding model' }));
        await user.selectOptions(screen.getByRole('combobox', { name: 'Embedding provider' }), 'ollama');
        const modelInput = await screen.findByLabelText('Ollama embedding model');
        await user.clear(modelInput);
        await user.type(modelInput, 'nomic-embed-text');
        await user.click(screen.getByRole('button', { name: 'Save Ollama config' }));
        await waitFor(() => expect(screen.getByRole('button', { name: 'Generate missing (1)' })).toBeEnabled());

        await user.click(screen.getByRole('button', { name: 'Generate missing (1)' }));
        await waitFor(() => expect(mocks.startModelEmbeddingGeneration).toHaveBeenCalledWith(
            'ollama:nomic-embed-text',
            ['in-a', 'in-b'],
            'missing',
        ));
    });

    it('reveals local model download controls only in configuration', async () => {
        mocks.isEmbeddingModelAvailable.mockResolvedValue(false);
        const user = userEvent.setup();
        render(EmbeddingExplorer);

        await screen.findByText('CLIP ViT-B/32');
        expect(screen.queryByRole('button', { name: /Download CLIP/i })).not.toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Configure embedding model' }));
        expect(await screen.findByRole('button', { name: /Download CLIP/i })).toBeInTheDocument();
    });

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
        await user.click(screen.getByRole('button', { name: 'Configure embedding model' }));
        await user.click(screen.getByRole('button', { name: 'Regenerate all (2)' }));
        expect(mocks.startModelEmbeddingGeneration).toHaveBeenLastCalledWith(
            'clip-vit-b32',
            ['in-a', 'in-b'],
            'all',
        );
    });

    it('ignores unrelated progress before the start response identifies its job', async () => {
        mocks.getEmbeddingCountForScope.mockResolvedValue(1);
        let resolveStart!: (value: {
            job_id: string;
            total: number;
            model: string;
            mode: 'missing';
        }) => void;
        mocks.startModelEmbeddingGeneration.mockReturnValue(new Promise(resolve => {
            resolveStart = resolve;
        }));
        const user = userEvent.setup();
        render(EmbeddingExplorer);

        await user.click(await screen.findByRole('button', { name: 'Generate missing (1)' }));
        await waitFor(() => expect(mocks.startModelEmbeddingGeneration).toHaveBeenCalledOnce());
        emit('embedding-progress', {
            job_id: 'job_unrelated',
            model: 'clip-vit-b32',
            mode: 'missing',
            status: 'running',
            current: 1,
            total: 9,
        });
        resolveStart({
            job_id: 'job_embed_ours',
            total: 1,
            model: 'clip-vit-b32',
            mode: 'missing',
        });

        await user.click(await screen.findByRole('button', { name: 'Cancel embedding generation' }));
        expect(mocks.cancelJob).toHaveBeenCalledWith('job_embed_ours');
        expect(mocks.cancelJob).not.toHaveBeenCalledWith('job_unrelated');
    });

    it('does not let generation from an old scope overwrite the newly selected scope', async () => {
        mocks.getEmbeddingCountForScope.mockImplementation(
            (scope: { type: string }) => Promise.resolve(scope.type === 'collection' ? 0 : 1),
        );

        const user = userEvent.setup();
        render(EmbeddingExplorer);
        await user.click(await screen.findByRole('button', { name: 'Generate missing (1)' }));
        await waitFor(() => expect(mocks.startModelEmbeddingGeneration).toHaveBeenCalledOnce());

        activeCollection.set('new-collection');
        await waitFor(() => expect(screen.getByText('0/2 images embedded')).toBeInTheDocument());

        emit('embedding-progress', {
            job_id: 'job_embed_1',
            model: 'clip-vit-b32',
            mode: 'missing',
            status: 'completed',
            current: 1,
            total: 1,
        });
        await waitFor(() => expect(screen.getByText('Generation completed: 1/1')).toBeInTheDocument());
        expect(screen.getByText('0/2 images embedded')).toBeInTheDocument();
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

        await user.click(screen.getByRole('button', { name: 'Configure embedding model' }));
        await user.selectOptions(screen.getByRole('combobox', { name: 'Embedding provider' }), 'dinov2');
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

    it('creates a collection from exactly the points inside a dragged rectangle', async () => {
        const user = userEvent.setup();
        const { container } = render(EmbeddingExplorer);
        await waitFor(() => expect(mocks.workerMessages).toHaveLength(1));

        await user.click(await screen.findByRole('button', { name: 'Select an area of the embedding map' }));
        const canvas = container.querySelector('canvas')!;
        await fireEvent.mouseDown(canvas, { clientX: 100, clientY: 20 });
        await fireEvent.mouseMove(canvas, { clientX: 300, clientY: 200 });
        await fireEvent.mouseUp(canvas, { clientX: 300, clientY: 200 });

        expect(await screen.findByText('1 image selected')).toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Create collection from 1 selected image' }));
        expect(get(textInputDialog)?.description).toBe('1 selected image will be added.');
        resolveTextInputDialog('Map picks');

        await waitFor(() => expect(mocks.createCollectionWithImages).toHaveBeenCalledWith(
            'Map picks',
            ['in-a'],
        ));
        await waitFor(() => expect(get(collections)).toEqual([['collection-from-map', 'Map picks', 1]]));
        expect(await screen.findByText('0 images selected')).toBeInTheDocument();
    });

    it('uses backend tag and detection evidence to auto-name projected clusters', async () => {
        mocks.nameEmbeddingClusters.mockResolvedValue([
            { cluster_id: 0, label: 'Golden Hour', source: 'tag' },
        ]);
        render(EmbeddingExplorer);

        expect(await screen.findByText('Golden Hour')).toBeInTheDocument();
        expect(mocks.nameEmbeddingClusters).toHaveBeenCalledWith([
            { cluster_id: 0, image_ids: ['in-a', 'in-b'] },
        ]);
    });

    it('renders the projection before naming completes and ignores a stale provider label', async () => {
        const user = userEvent.setup();
        let resolveOldName!: (value: Array<{ cluster_id: number; label: string; source: string }>) => void;
        mocks.nameEmbeddingClusters
            .mockReturnValueOnce(new Promise(resolve => {
                resolveOldName = resolve;
            }))
            .mockResolvedValueOnce([
                { cluster_id: 0, label: 'New Provider Cluster', source: 'filename' },
            ]);

        render(EmbeddingExplorer);
        expect(await screen.findByText('Cluster 1')).toBeInTheDocument();

        await user.click(screen.getByRole('button', { name: 'Configure embedding model' }));
        await user.selectOptions(screen.getByRole('combobox', { name: 'Embedding provider' }), 'dinov2');
        expect(await screen.findByText('New Provider Cluster')).toBeInTheDocument();

        resolveOldName([{ cluster_id: 0, label: 'Old Provider Cluster', source: 'tag' }]);
        await tick();
        expect(screen.queryByText('Old Provider Cluster')).not.toBeInTheDocument();
        expect(screen.getByText('New Provider Cluster')).toBeInTheDocument();
    });

    it('supports keyboard point selection and Escape from a focused selection control', async () => {
        const user = userEvent.setup();
        render(EmbeddingExplorer);
        await waitFor(() => expect(mocks.workerMessages).toHaveLength(1));

        await user.click(await screen.findByRole('button', { name: 'Select an area of the embedding map' }));
        const explorer = screen.getByRole('application', { name: 'Visual embeddings' });
        expect(explorer).toHaveFocus();
        await user.keyboard('{ArrowRight} {Shift>}{ArrowRight}{/Shift}');
        expect(await screen.findByText('2 images selected')).toBeInTheDocument();

        const clearButton = screen.getByRole('button', { name: 'Clear selection' });
        clearButton.focus();
        await user.keyboard('{Escape}');
        expect(screen.getByRole('button', { name: 'Select an area of the embedding map' })).toHaveAttribute('aria-pressed', 'false');
        expect(screen.getByText('0 images selected')).toBeInTheDocument();
        expect(screen.queryByRole('button', { name: 'Clear selection' })).not.toBeInTheDocument();
        expect(explorer).toHaveFocus();
    });

    it('clears an area selection when the same scope receives a replacement projection', async () => {
        const user = userEvent.setup();
        const { container } = render(EmbeddingExplorer);
        await waitFor(() => expect(mocks.workerMessages).toHaveLength(1));
        await user.click(await screen.findByRole('button', { name: 'Select an area of the embedding map' }));
        const canvas = container.querySelector('canvas')!;
        await fireEvent.mouseDown(canvas, { clientX: 100, clientY: 20 });
        await fireEvent.mouseMove(canvas, { clientX: 300, clientY: 200 });
        await fireEvent.mouseUp(canvas, { clientX: 300, clientY: 200 });
        expect(await screen.findByText('1 image selected')).toBeInTheDocument();

        settingsOpen.set(true);
        await tick();
        settingsOpen.set(false);
        await waitFor(() => expect(mocks.workerMessages).toHaveLength(2));
        expect(screen.getByText('0 images selected')).toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'Select an area of the embedding map' })).toHaveAttribute('aria-pressed', 'false');
    });

});
