<script lang="ts">
    import { onMount } from 'svelte';
    import { listen, type UnlistenFn } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import {
        images,
        focusedIndex,
        focusedImageOverride,
        zenMode,
        navigateTo,
        embeddingViewState,
        settingsOpen,
        collections,
        requestTextInput,
        showRejected,
        showToast,
        type EmbeddingViewState,
        type EmbeddingProvider,
        type EmbeddingInteractionMode,
        type EmbeddingZPreset,
        type EmbeddingSpacePreset,
    } from '$lib/stores';
    import { get } from 'svelte/store';
    import { openSettings } from '$lib/settings-navigation';
    import { computeScatterThumbSize, formatBytes, formatDownloadRateEta } from '$lib/embedding-utils';
    import {
        DEFAULT_MODEL_OPTIONS,
        modelOptionsFromProviderInfo,
        type LocalEmbeddingProvider,
        type ModelOption,
    } from '$lib/embedding-providers';
    import {
        isEmbeddingModelAvailable,
        getEmbeddingModelDownloadInfo,
        listEmbeddingProviders,
        downloadEmbeddingModel,
        startModelEmbeddingGeneration,
        hasApiKey,
        setApiKey,
        validateApiKey,
        getOllamaEmbeddingConfig,
        setOllamaEmbeddingConfig,
        getImagesByIds,
        getGenerationRun,
        regenerateThumbnails,
        cancelJob,
        pauseJob,
        resumeJob,
        createCollectionWithImages,
        listCollections,
    } from '$lib/api';
    import type {
        EmbeddingGenerationMode,
        EmbeddingGenerationProgress,
        EmbeddingModelDownloadInfo,
        EmbeddingPage,
        GenerationRun,
        ImageWithFile,
    } from '$lib/api';
    import { isAssetProtocolSafePath, safeAssetPreviewPath } from '$lib/view-utils';
    import { libraryScope, libraryScopeKey, type LibraryScope } from '$lib/library-scope';
    import {
        getEmbeddingCountForScope,
        getEmbeddingPageForScope,
        getImageCountForScope,
        listImageIdsForScope,
    } from '$lib/embedding-scope';
    import { loadEmbeddingNeighbors, type EmbeddingNeighbor } from '$lib/embedding-neighbors';

    // State
    let downloading = $state(false);
    type ActiveEmbeddingGeneration = EmbeddingGenerationProgress & {
        provider: EmbeddingProvider;
        scope: LibraryScope;
        scopeKey: string;
    };
    let activeGeneration = $state<ActiveEmbeddingGeneration | null>(null);
    let generationUnlisten: UnlistenFn | null = null;
    let generating = $derived(activeGeneration?.status === 'running' || activeGeneration?.status === 'cancelling');
    let totalImages = $state(0);
    let regeneratingThumbs = $state(false);
    let staleEmbeddingCount = $state(0);

    // Provider config
    type LocalProvider = LocalEmbeddingProvider;
    type RemoteProvider = Exclude<EmbeddingProvider, LocalProvider>;
    let modelOptions = $state<ModelOption[]>(DEFAULT_MODEL_OPTIONS);
    let selectedProvider = $state<EmbeddingProvider>('clip');
    let configOpen = $state(false);
    let hasGoogleKey = $state(false);
    let hasCohereKey = $state(false);
    let hasOpenAiKey = $state(false);
    let ollamaEmbeddingReady = $state(false);
    let providerKeyInput = $state('');
    let providerConfigStatus = $state<'idle' | 'loading' | 'saving' | 'saved' | 'invalid' | 'error'>('idle');
    let providerConfigLoadSeq = 0;
    let ollamaEmbeddingUrl = $state('http://localhost:11434/api/embed');
    let ollamaEmbeddingModel = $state('embeddinggemma');
    let localModelAvailable = $state<Record<LocalProvider, boolean>>({ clip: false, dinov2: false });
    let localEmbeddingCounts = $state<Record<LocalProvider, number>>({ clip: 0, dinov2: 0 });
    let localModelDownloadInfo = $state<Record<LocalProvider, EmbeddingModelDownloadInfo | null>>({ clip: null, dinov2: null });
    let remoteEmbeddingCounts = $state<Record<RemoteProvider, number>>({ gemini: 0, cohere: 0, openai: 0, ollama: 0 });
    let currentEmbeddingCount = $derived(providerEmbeddingCount(selectedProvider));
    let missingEmbeddingCount = $derived(Math.max(0, totalImages - currentEmbeddingCount));
    let selectedModel = $derived(modelOptions.find(option => option.id === selectedProvider) ?? modelOptions[0] ?? DEFAULT_MODEL_OPTIONS[0]);
    let selectedModelAvailable = $derived(providerReady(selectedProvider));
    let selectedDownloadInfo = $derived(isLocalProvider(selectedProvider) ? localModelDownloadInfo[selectedProvider] : null);
    const PROJECTION_EMBEDDING_LIMIT = 5000;

    // Visual embed interaction config
    type ZLayer = { key: string; label: string; count: number; rank: number; color: string | null };
    const INTERACTION_MODES: { id: EmbeddingInteractionMode; label: string; title: string }[] = [
        { id: 'map', label: 'Map', title: 'Pan and zoom the whole projection' },
        { id: 'stack', label: 'Stack', title: 'Work one z-layer at a time' },
        { id: 'review', label: 'Review', title: 'Step through images with a large preview' },
        { id: 'text', label: 'Text', title: 'Inspect text output for the active image or layer' },
    ];
    const Z_PRESETS: { id: EmbeddingZPreset; label: string; title: string }[] = [
        { id: 'projection', label: 'Projection', title: 'Use the natural projection order' },
        { id: 'cluster', label: 'Cluster', title: 'Bring one visual cluster forward' },
        { id: 'source', label: 'Source', title: 'Layer images by detected generator/source' },
        { id: 'rating', label: 'Rating', title: 'Layer higher-rated images above unrated images' },
        { id: 'decision', label: 'Decision', title: 'Layer accepted, rejected, and undecided images' },
        { id: 'recency', label: 'Recency', title: 'Layer images by import month' },
        { id: 'resolution', label: 'Resolution', title: 'Layer images by megapixel bucket' },
    ];
    type SpacePresetValues = {
        spacing: number;
        depth: number;
        scale: number;
        perspective: number;
    };
    const SPACE_PRESETS: { id: Exclude<EmbeddingSpacePreset, 'custom'>; label: string; title: string; values: SpacePresetValues }[] = [
        {
            id: 'balanced',
            label: 'Balanced',
            title: 'Neutral spacing with shallow z separation',
            values: { spacing: 1, depth: 0.35, scale: 1, perspective: 0.3 },
        },
        {
            id: 'compact',
            label: 'Compact',
            title: 'Dense contact-sheet view for large sets',
            values: { spacing: 0.72, depth: 0.18, scale: 0.82, perspective: 0.12 },
        },
        {
            id: 'gallery',
            label: 'Gallery',
            title: 'More air between images with larger thumbnails',
            values: { spacing: 1.35, depth: 0.32, scale: 1.28, perspective: 0.18 },
        },
        {
            id: 'deep',
            label: 'Deep',
            title: 'Layered z-space with stronger perspective',
            values: { spacing: 1.08, depth: 0.88, scale: 1.08, perspective: 0.72 },
        },
    ];
    const SOURCE_DISPLAY: Record<string, string> = {
        gpt_image_2: 'GPT-image-2',
        dalle_3: 'DALL-E 3',
        dalle: 'DALL-E',
        openai: 'OpenAI',
        stable_diffusion: 'Stable Diffusion',
        comfyui: 'ComfyUI',
        midjourney: 'Midjourney',
        nanobanana: 'Nanobanana',
    };
    let interactionMode = $state<EmbeddingInteractionMode>('map');
    let zPreset = $state<EmbeddingZPreset>('cluster');
    let activeZLayerKey = $state<string | null>(null);
    let focusActiveLayer = $state(false);
    let largePreviewOpen = $state(true);
    let textOutputOpen = $state(false);
    let canvasLabelsOpen = $state(false);
    let spacePreset = $state<EmbeddingSpacePreset>('balanced');
    let spaceSpacing = $state(1);
    let spaceDepth = $state(0.35);
    let spaceScale = $state(1);
    let spacePerspective = $state(0.3);

    // Download progress
    let downloadProgress = $state({ downloaded: 0, total: 0, status: '', job_id: null as string | null, error: null as string | null });
    let downloadStartTime = $state(0);
    let downloadSpeed = $state('');
    let downloadJobId = $state<string | null>(null);

    // UMAP projection
    type Point = { id: string; x: number; y: number; cluster: number };
    let points = $state<Point[]>([]);
    let clusters = $state<{ id: number; label: string; count: number; color: string; previewPaths: string[]; x: number; y: number }[]>([]);
    let hoveredPoint = $state<Point | null>(null);
    let selectedPoint = $state<Point | null>(null);
    let highlightedCluster = $state<number | null>(null);

    // Thumbnail images for scatter — keyed by "{id}_{size}"
    let thumbnailImages: Map<string, HTMLImageElement> = new Map();
    let loadingThumbnailKeys: Set<string> = new Set();
    let failedThumbnailIds = $state<Set<string>>(new Set());
    const THUMB_SIZES = [64, 128, 256, 800];
    const MAX_THUMBNAIL_CACHE = 600;

    // Canvas interaction
    let explorerEl: HTMLDivElement;
    let canvas: HTMLCanvasElement;
    let canvasWidth = $state(800);
    let canvasHeight = $state(600);
    let panX = $state(0);
    let panY = $state(0);
    let scale = $state(1);
    let dragging = $state(false);
    let drawQueued = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let dragStartPanX = 0;
    let dragStartPanY = 0;
    type AreaSelectionRect = { startX: number; startY: number; currentX: number; currentY: number };
    let areaSelectionMode = $state(false);
    let areaSelectionRect = $state<AreaSelectionRect | null>(null);
    let areaSelectedIds = $state<Set<string>>(new Set());
    let creatingAreaCollection = $state(false);
    let areaSelectedCount = $derived(areaSelectedIds.size);

    // Projection identity for view state validation
    let projectionKey = $state<string | null>(null);

    // Image lookup for thumbnails
    let imageMap = $state<Map<string, ImageWithFile>>(new Map());
    let selectedGenerationRun = $state<GenerationRun | null>(null);
    let selectedGenerationLoadSeq = 0;
    let nearestNeighbors = $state<EmbeddingNeighbor[]>([]);
    let neighborsLoading = $state(false);
    let neighborsError = $state<string | null>(null);
    let neighborLoadSeq = 0;
    let projecting = $state(false);
    let projectionWorker: Worker | null = null;
    let cancelProjectionWork: (() => void) | null = null;
    let projectionRequestId = 0;
    let projectionLoadSeq = 0;

    const CLUSTER_COLORS = [
        '#7aa2f7', '#9ece6a', '#e0af68', '#bb9af7', '#f7768e',
        '#73daca', '#ff9e64', '#2ac3de', '#c0caf5', '#a9b1d6',
        '#41a6b5', '#c3e88d', '#fc5d7c', '#89b4fa', '#f5c2e7',
        '#fab387', '#94e2d5', '#cba6f7',
    ];

    let zLayers = $derived(buildZLayersForPreset(zPreset));
    let projectionCenter = $derived(computeProjectionCenter(points));
    let zLayerRankRange = $derived(computeZLayerRankRange(zLayers));
    let activeZLayer = $derived(activeZLayerKey ? zLayers.find(layer => layer.key === activeZLayerKey) ?? null : null);
    let activeLayerPoints = $derived(activeZLayerKey ? getPointsForLayer(activeZLayerKey) : points);
    let selectedImage = $derived(selectedPoint ? imageMap.get(selectedPoint.id) ?? null : null);
    let selectedFilename = $derived(selectedImage?.path.split('/').pop() ?? '');
    let selectedPrompt = $derived(selectedGenerationRun?.prompt ?? selectedImage?.image.ai_prompt ?? null);
    let navigationPoints = $derived(getNavigationPoints());
    let selectedNavigationIndex = $derived(selectedPoint ? navigationPoints.findIndex(point => point.id === selectedPoint?.id) : -1);
    let selectedNavigationLabel = $derived(
        navigationPoints.length === 0
            ? '0/0'
            : `${selectedNavigationIndex >= 0 ? selectedNavigationIndex + 1 : 0}/${navigationPoints.length}`
    );
    let textOutput = $derived(buildTextOutput());

    $effect(() => {
        const layers = zLayers;
        if (layers.length === 0) {
            if (activeZLayerKey !== null) activeZLayerKey = null;
            return;
        }
        if (activeZLayerKey && !layers.some(layer => layer.key === activeZLayerKey)) {
            activeZLayerKey = layers[0].key;
            requestDraw();
        }
    });

    $effect(() => {
        zPreset;
        activeZLayerKey;
        focusActiveLayer;
        canvasLabelsOpen;
        spaceSpacing;
        spaceDepth;
        spaceScale;
        spacePerspective;
        requestDraw();
    });

    $effect(() => {
        const id = selectedPoint?.id ?? null;
        const loadSeq = ++selectedGenerationLoadSeq;
        selectedGenerationRun = null;
        if (!id) return;
        getGenerationRun(id)
            .then(run => {
                if (loadSeq === selectedGenerationLoadSeq) selectedGenerationRun = run;
            })
            .catch(() => {
                if (loadSeq === selectedGenerationLoadSeq) selectedGenerationRun = null;
            });
    });

    $effect(() => {
        const id = selectedPoint?.id ?? null;
        const scope = $libraryScope;
        const requestedScopeKey = libraryScopeKey(scope);
        const model = modelNameForProvider(selectedProvider);
        const loadSeq = ++neighborLoadSeq;
        nearestNeighbors = [];
        neighborsError = null;
        if (!id) {
            neighborsLoading = false;
            return;
        }

        neighborsLoading = true;
        loadEmbeddingNeighbors(scope, id, model, 6)
            .then(neighbors => {
                if (loadSeq !== neighborLoadSeq) return;
                if (selectedPoint?.id !== id) return;
                if (libraryScopeKey(get(libraryScope)) !== requestedScopeKey) return;
                if (modelNameForProvider(selectedProvider) !== model) return;
                nearestNeighbors = neighbors;
                neighborsLoading = false;
            })
            .catch(error => {
                if (loadSeq !== neighborLoadSeq) return;
                if (selectedPoint?.id !== id) return;
                if (libraryScopeKey(get(libraryScope)) !== requestedScopeKey) return;
                if (modelNameForProvider(selectedProvider) !== model) return;
                neighborsError = error instanceof Error ? error.message : String(error);
                neighborsLoading = false;
            });
    });

    function isLocalProvider(provider: EmbeddingProvider): provider is LocalProvider {
        return provider === 'clip' || provider === 'dinov2';
    }

    function isRemoteProvider(provider: EmbeddingProvider): provider is RemoteProvider {
        return !isLocalProvider(provider);
    }

    function knownProviderId(id: string): EmbeddingProvider | null {
        if (id === 'clip' || id === 'dinov2' || id === 'gemini' || id === 'cohere' || id === 'openai' || id === 'ollama') return id;
        return null;
    }

    function localProviderOptions(): Array<ModelOption & { id: LocalProvider }> {
        return modelOptions.filter((option): option is ModelOption & { id: LocalProvider } => isLocalProvider(option.id));
    }

    function modelNameForProvider(provider: EmbeddingProvider): string {
        return modelOptions.find(option => option.id === provider)?.modelName ?? 'clip-vit-b32';
    }

    function providerEmbeddingCount(provider: EmbeddingProvider): number {
        if (isLocalProvider(provider)) return localEmbeddingCounts[provider];
        return remoteEmbeddingCounts[provider];
    }

    function providerReady(provider: EmbeddingProvider): boolean {
        if (provider === 'gemini') return hasGoogleKey;
        if (provider === 'cohere') return hasCohereKey;
        if (provider === 'openai') return hasOpenAiKey;
        if (provider === 'ollama') return ollamaEmbeddingReady;
        if (isLocalProvider(provider)) return localModelAvailable[provider];
        return false;
    }

    function providerStatusLabel(provider: EmbeddingProvider): string {
        if (provider === 'gemini') return hasGoogleKey ? 'ready' : 'key';
        if (provider === 'cohere') return hasCohereKey ? 'ready' : 'key';
        if (provider === 'openai') return hasOpenAiKey ? 'ready' : 'key';
        if (provider === 'ollama') return ollamaEmbeddingReady ? 'ready' : 'offline';
        if (isLocalProvider(provider)) return localModelAvailable[provider] ? 'ready' : 'model';
        return 'config';
    }

    async function selectProvider(provider: EmbeddingProvider) {
        if (provider === selectedProvider) return;
        providerConfigLoadSeq += 1;
        providerKeyInput = '';
        providerConfigStatus = 'idle';
        selectedProvider = provider;
        await handleProviderChange();
        if (configOpen) await loadSelectedProviderConfig();
    }

    function handleProviderSelection(event: Event) {
        const provider = (event.currentTarget as HTMLSelectElement).value as EmbeddingProvider;
        void selectProvider(provider);
    }

    function embeddingApiKeyProvider(provider: EmbeddingProvider): 'google' | 'cohere' | 'openai' | null {
        if (provider === 'gemini') return 'google';
        if (provider === 'cohere' || provider === 'openai') return provider;
        return null;
    }

    async function toggleEmbeddingConfig() {
        configOpen = !configOpen;
        if (configOpen) {
            await loadSelectedProviderConfig();
        } else {
            providerConfigLoadSeq += 1;
            providerKeyInput = '';
            providerConfigStatus = 'idle';
        }
    }

    async function loadSelectedProviderConfig() {
        const provider = selectedProvider;
        const requestSeq = ++providerConfigLoadSeq;
        providerKeyInput = '';
        providerConfigStatus = provider === 'ollama' ? 'loading' : 'idle';
        if (provider !== 'ollama') return;
        try {
            const [url, model] = await getOllamaEmbeddingConfig();
            if (requestSeq !== providerConfigLoadSeq || !configOpen || selectedProvider !== provider) return;
            ollamaEmbeddingUrl = url;
            ollamaEmbeddingModel = model;
            providerConfigStatus = 'idle';
        } catch {
            if (requestSeq !== providerConfigLoadSeq || !configOpen || selectedProvider !== provider) return;
            providerConfigStatus = 'error';
        }
    }

    async function saveSelectedProviderConfig() {
        const provider = selectedProvider;
        const requestSeq = providerConfigLoadSeq;
        providerConfigStatus = 'saving';
        try {
            if (provider === 'ollama') {
                const url = ollamaEmbeddingUrl.trim() || 'http://localhost:11434/api/embed';
                const model = ollamaEmbeddingModel.trim() || 'embeddinggemma';
                ollamaEmbeddingUrl = url;
                ollamaEmbeddingModel = model;
                await setOllamaEmbeddingConfig(
                    url,
                    model,
                );
                await loadProviderOptions();
                if (selectedProvider === provider) await loadEmbeddingState();
                if (selectedProvider !== provider) return;
                providerConfigStatus = 'saved';
                return;
            }

            const apiProvider = embeddingApiKeyProvider(provider);
            const key = providerKeyInput.trim();
            if (!apiProvider || !key) {
                providerConfigStatus = 'invalid';
                return;
            }
            const valid = await validateApiKey(apiProvider, key);
            if (requestSeq !== providerConfigLoadSeq || selectedProvider !== provider) return;
            if (!valid) {
                providerConfigStatus = 'invalid';
                return;
            }
            await setApiKey(apiProvider, key);
            if (apiProvider === 'google') hasGoogleKey = true;
            if (apiProvider === 'cohere') hasCohereKey = true;
            if (apiProvider === 'openai') hasOpenAiKey = true;
            await loadProviderOptions();
            if (selectedProvider !== provider) return;
            providerKeyInput = '';
            providerConfigStatus = 'saved';
        } catch {
            if (selectedProvider !== provider) return;
            providerConfigStatus = 'error';
        }
    }

    function restoreInteractionState(savedState: Partial<EmbeddingViewState>) {
        selectedProvider = savedState.provider ?? 'clip';
        interactionMode = savedState.interactionMode ?? 'map';
        zPreset = savedState.zPreset ?? 'cluster';
        activeZLayerKey = savedState.activeZLayerKey ?? null;
        focusActiveLayer = savedState.focusActiveLayer ?? false;
        largePreviewOpen = savedState.largePreviewOpen ?? true;
        textOutputOpen = savedState.textOutputOpen ?? false;
        canvasLabelsOpen = savedState.canvasLabelsOpen ?? false;
        const defaultPreset = SPACE_PRESETS[0];
        spacePreset = savedState.spacePreset ?? defaultPreset.id;
        const presetValues = SPACE_PRESETS.find(preset => preset.id === spacePreset)?.values ?? defaultPreset.values;
        spaceSpacing = savedState.spaceSpacing ?? presetValues.spacing;
        spaceDepth = savedState.spaceDepth ?? presetValues.depth;
        spaceScale = savedState.spaceScale ?? presetValues.scale;
        spacePerspective = savedState.spacePerspective ?? presetValues.perspective;
    }

    onMount(() => {
        void (async () => {
            const savedState = get(embeddingViewState);
            restoreInteractionState(savedState);
            await loadProviderOptions();
            await loadLocalModelDownloadInfo();
            await checkLocalModels();
            await loadApiKeyState();
            await loadEmbeddingState();
        })();

        return () => {
            generationUnlisten?.();
            generationUnlisten = null;
            resetProjectionWorker();
        };
    });

    async function loadProviderOptions() {
        try {
            const providers = await listEmbeddingProviders();
            const nextOptions = modelOptionsFromProviderInfo(providers);
            if (nextOptions.length > 0) {
                modelOptions = nextOptions;
            }

            const nextAvailable = { ...localModelAvailable };
            for (const providerInfo of providers) {
                const provider = knownProviderId(providerInfo.id);
                if (!provider) continue;
                if (isLocalProvider(provider)) {
                    nextAvailable[provider] = providerInfo.available;
                } else if (provider === 'gemini') {
                    hasGoogleKey = providerInfo.available;
                } else if (provider === 'cohere') {
                    hasCohereKey = providerInfo.available;
                } else if (provider === 'openai') {
                    hasOpenAiKey = providerInfo.available;
                } else if (provider === 'ollama') {
                    ollamaEmbeddingReady = providerInfo.available;
                }
            }
            localModelAvailable = nextAvailable;
        } catch (e) {
            console.error('Failed to load embedding providers:', e);
        }
    }

    async function checkLocalModels() {
        try {
            const checks = await Promise.all(
                localProviderOptions().map(async option => [
                    option.id,
                    await isEmbeddingModelAvailable(option.modelName),
                ] as const)
            );
            const nextAvailable = { ...localModelAvailable };
            for (const [provider, available] of checks) {
                nextAvailable[provider] = available;
            }
            localModelAvailable = nextAvailable;
        } catch (e) {
            console.error('Failed to check embedding models:', e);
        }
    }

    async function loadLocalModelDownloadInfo() {
        try {
            const infos = await Promise.all(
                localProviderOptions().map(async option => [
                    option.id,
                    await getEmbeddingModelDownloadInfo(option.modelName),
                ] as const)
            );
            const nextInfo = { ...localModelDownloadInfo };
            for (const [provider, info] of infos) {
                nextInfo[provider] = info;
            }
            localModelDownloadInfo = nextInfo;
        } catch (e) {
            console.error('Failed to load embedding model download info:', e);
        }
    }

    async function loadEmbeddingState() {
        try {
            const scope = get(libraryScope);
            const requestedScopeKey = libraryScopeKey(scope);
            const imageTotal = await getImageCountForScope(scope);
            if (requestedScopeKey !== libraryScopeKey(get(libraryScope))) return;
            const countEntries = await Promise.all(
                modelOptions.map(async option => [
                    option.id,
                    await getEmbeddingCountForScope(scope, option.modelName),
                ] as const)
            );
            if (requestedScopeKey !== libraryScopeKey(get(libraryScope))) return;
            totalImages = imageTotal;
            const nextLocalCounts = { ...localEmbeddingCounts };
            const nextRemoteCounts = { ...remoteEmbeddingCounts };
            let selectedCount = 0;
            for (const [provider, count] of countEntries) {
                if (isLocalProvider(provider)) {
                    nextLocalCounts[provider] = count;
                } else {
                    nextRemoteCounts[provider] = count;
                }
                if (provider === selectedProvider) selectedCount = count;
            }
            localEmbeddingCounts = nextLocalCounts;
            remoteEmbeddingCounts = nextRemoteCounts;
            if (selectedCount > 0) {
                await loadProjection(scope);
            } else {
                points = [];
                clusters = [];
                resetThumbnailCache();
            }
        } catch (e) {
            console.error('Failed to load embedding state:', e);
        }
    }

    async function loadApiKeyState() {
        try {
            const [googleKey, cohereKey, openAiKey] = await Promise.all([
                hasApiKey('google'),
                hasApiKey('cohere'),
                hasApiKey('openai'),
            ]);
            hasGoogleKey = googleKey;
            hasCohereKey = cohereKey;
            hasOpenAiKey = openAiKey;
        } catch (e) {
            console.error('Failed to load API key state:', e);
        }
    }

    let prevSettingsOpen = false;
    $effect(() => {
        const isOpen = $settingsOpen;
        if (prevSettingsOpen && !isOpen) {
            void (async () => {
                await loadApiKeyState();
                await loadProviderOptions();
                await loadEmbeddingState();
            })();
        }
        prevSettingsOpen = isOpen;
    });

    let observedScopeKey = '';
    $effect(() => {
        const nextScopeKey = libraryScopeKey($libraryScope);
        if (observedScopeKey === '') {
            observedScopeKey = nextScopeKey;
            return;
        }
        if (nextScopeKey === observedScopeKey) return;
        observedScopeKey = nextScopeKey;
        resetProjectionWorker();
        points = [];
        clusters = [];
        selectedPoint = null;
        highlightedCluster = null;
        resetAreaSelection();
        resetThumbnailCache();
        void loadEmbeddingState();
    });

    async function handleProviderChange() {
        resetProjectionWorker();
        points = [];
        clusters = [];
        selectedPoint = null;
        highlightedCluster = null;
        resetAreaSelection();
        activeZLayerKey = null;
        resetThumbnailCache();
        await loadEmbeddingState();
        saveViewState();
    }

    async function startGeneration(mode: EmbeddingGenerationMode) {
        if (generating || !selectedModelAvailable) return;
        const provider = selectedProvider;
        const modelName = modelNameForProvider(provider);
        const scope = get(libraryScope);
        const scopeKey = libraryScopeKey(scope);
        activeGeneration = {
            job_id: '',
            total: mode === 'missing' ? missingEmbeddingCount : totalImages,
            model: modelName,
            mode,
            current: 0,
            status: 'running',
            error: null,
            provider,
            scope,
            scopeKey,
        };

        try {
            const pendingProgress = new Map<string, EmbeddingGenerationProgress>();
            const imageIds = await listImageIdsForScope(scope);
            if (scopeKey !== libraryScopeKey(get(libraryScope))) {
                activeGeneration = null;
                return;
            }
            totalImages = imageIds.length;

            generationUnlisten?.();
            generationUnlisten = await listen<EmbeddingGenerationProgress>('embedding-progress', event => {
                const progress = event.payload;
                const active = activeGeneration;
                if (!active || progress.model !== modelName || progress.mode !== mode) return;
                if (!active.job_id) {
                    pendingProgress.set(progress.job_id, progress);
                    return;
                }
                if (progress.job_id !== active.job_id) return;
                activeGeneration = { ...active, ...progress };
                if (progress.status === 'completed' || progress.status === 'cancelled' || progress.status === 'failed') {
                    generationUnlisten?.();
                    generationUnlisten = null;
                    void refreshAfterGeneration({ ...active, ...progress });
                }
            });

            const started = await startModelEmbeddingGeneration(modelName, imageIds, mode);
            const active = activeGeneration;
            if (!active) return;
            const bufferedProgress = pendingProgress.get(started.job_id);
            activeGeneration = { ...active, ...started, ...bufferedProgress };
            if (bufferedProgress && (
                bufferedProgress.status === 'completed'
                || bufferedProgress.status === 'cancelled'
                || bufferedProgress.status === 'failed'
            )) {
                generationUnlisten?.();
                generationUnlisten = null;
                await refreshAfterGeneration(activeGeneration);
                return;
            }
            if (started.total === 0) {
                activeGeneration = { ...activeGeneration!, status: 'completed', current: 0 };
                generationUnlisten?.();
                generationUnlisten = null;
                await refreshAfterGeneration(activeGeneration);
            }
        } catch (error) {
            generationUnlisten?.();
            generationUnlisten = null;
            const message = error instanceof Error ? error.message : String(error);
            if (activeGeneration) activeGeneration = { ...activeGeneration, status: 'failed', error: message };
        }
    }

    async function refreshAfterGeneration(generation: ActiveEmbeddingGeneration) {
        if (generation.scopeKey !== libraryScopeKey(get(libraryScope))) return;
        try {
            const count = await getEmbeddingCountForScope(generation.scope, generation.model);
            if (generation.scopeKey !== libraryScopeKey(get(libraryScope))) return;
            if (isLocalProvider(generation.provider)) {
                localEmbeddingCounts = { ...localEmbeddingCounts, [generation.provider]: count };
            } else {
                remoteEmbeddingCounts = { ...remoteEmbeddingCounts, [generation.provider]: count };
            }
            if (modelNameForProvider(selectedProvider) === generation.model) {
                await loadProjection(generation.scope);
            }
        } catch (error) {
            console.error('Failed to refresh embedding state after generation:', error);
        }
    }

    async function cancelGeneration() {
        const generation = activeGeneration;
        if (!generation?.job_id || generation.status !== 'running') return;
        activeGeneration = { ...generation, status: 'cancelling' };
        try {
            await cancelJob(generation.job_id);
        } catch (error) {
            activeGeneration = {
                ...generation,
                status: 'failed',
                error: error instanceof Error ? error.message : String(error),
            };
        }
    }

    function generationModelLabel(model: string): string {
        return modelOptions.find(option => option.modelName === model)?.shortLabel ?? model;
    }

    async function handleDownload() {
        if (!isLocalProvider(selectedProvider)) return;
        const provider = selectedProvider;
        const modelName = modelNameForProvider(provider);
        downloading = true;
        downloadProgress = { downloaded: 0, total: 0, status: 'downloading', job_id: null, error: null };
        downloadJobId = null;
        downloadStartTime = Date.now();
        downloadSpeed = '';

        const unlisten: UnlistenFn = await listen<{ downloaded: number; total: number; status: string; job_id?: string; error?: string; model?: string }>(
            'model-download-progress',
            (event) => {
                if (event.payload.model && event.payload.model !== modelName) return;
                downloadProgress = { ...event.payload, job_id: event.payload.job_id ?? downloadJobId, error: event.payload.error ?? null };
                if (event.payload.job_id) downloadJobId = event.payload.job_id;
                downloadSpeed = formatDownloadRateEta(
                    event.payload.downloaded,
                    event.payload.total,
                    downloadStartTime
                );
            }
        );

        try {
            await downloadEmbeddingModel(modelName);
            localModelAvailable = { ...localModelAvailable, [provider]: true };
        } catch (e) {
            console.error('Download failed:', e);
            downloadProgress = { ...downloadProgress, status: downloadProgress.status === 'cancelled' ? 'cancelled' : 'failed', error: String(e) };
        } finally {
            unlisten();
            downloading = false;
        }
    }

    async function handlePauseDownload() {
        if (!downloadJobId) return;
        await pauseJob(downloadJobId);
        downloadProgress = { ...downloadProgress, status: 'paused' };
    }

    async function handleResumeDownload() {
        if (!downloadJobId) return;
        await resumeJob(downloadJobId);
        downloadProgress = { ...downloadProgress, status: 'downloading' };
    }

    async function handleCancelDownload() {
        if (!downloadJobId) return;
        await cancelJob(downloadJobId);
        downloadProgress = { ...downloadProgress, status: 'cancelled' };
    }

    async function handleRegenerateThumbnails() {
        regeneratingThumbs = true;
        try {
            await regenerateThumbnails();
            await loadProjection();
        } catch (e) {
            console.error('Thumbnail regeneration failed:', e);
        } finally {
            regeneratingThumbs = false;
        }
    }

    function sizedThumbPath(basePath: string, size: number): string {
        if (size === 800) return basePath;
        return basePath.replace(/\.jpg$/, `_${size}.jpg`);
    }

    function resetThumbnailCache() {
        thumbnailImages.clear();
        loadingThumbnailKeys.clear();
        failedThumbnailIds = new Set();
    }

    function preferredThumbSize(displayPx: number): number {
        const dpr = window.devicePixelRatio || 1;
        const physicalPx = displayPx * dpr * 1.5;
        return THUMB_SIZES.find(size => size >= physicalPx) ?? THUMB_SIZES[THUMB_SIZES.length - 1];
    }

    function rememberThumbnail(key: string, el: HTMLImageElement) {
        if (thumbnailImages.has(key)) thumbnailImages.delete(key);
        thumbnailImages.set(key, el);
        while (thumbnailImages.size > MAX_THUMBNAIL_CACHE) {
            const oldest = thumbnailImages.keys().next().value;
            if (!oldest) break;
            thumbnailImages.delete(oldest);
        }
    }

    function queueThumbnailLoad(id: string, size: number) {
        const key = `${id}_${size}`;
        if (thumbnailImages.has(key) || loadingThumbnailKeys.has(key)) return;
        const img = imageMap.get(id);
        if (!img?.thumbnail_path || !isAssetProtocolSafePath(img.thumbnail_path)) {
            failedThumbnailIds = new Set([...failedThumbnailIds, id]);
            return;
        }

        loadingThumbnailKeys.add(key);
        const el = new Image();
        el.onload = () => {
            loadingThumbnailKeys.delete(key);
            rememberThumbnail(key, el);
            requestDraw();
        };
        el.onerror = () => {
            loadingThumbnailKeys.delete(key);
            if (!THUMB_SIZES.some(s => thumbnailImages.has(`${id}_${s}`))) {
                failedThumbnailIds = new Set([...failedThumbnailIds, id]);
            }
        };
        el.src = convertFileSrc(sizedThumbPath(img.thumbnail_path, size));
    }

    function pickThumbnail(id: string, displayPx: number): HTMLImageElement | undefined {
        if (failedThumbnailIds.has(id)) return undefined;
        const preferred = preferredThumbSize(displayPx);
        const preferredEl = thumbnailImages.get(`${id}_${preferred}`);
        if (preferredEl?.complete && preferredEl.naturalWidth > 0) {
            thumbnailImages.delete(`${id}_${preferred}`);
            thumbnailImages.set(`${id}_${preferred}`, preferredEl);
            return preferredEl;
        }
        queueThumbnailLoad(id, preferred);

        for (let i = THUMB_SIZES.length - 1; i >= 0; i--) {
            const el = thumbnailImages.get(`${id}_${THUMB_SIZES[i]}`);
            if (el?.complete && el.naturalWidth > 0) return el;
        }
        return undefined;
    }

    function displaySourceLabel(source: string | null): string {
        if (!source) return 'Unknown source';
        return SOURCE_DISPLAY[source] ?? source.replace(/_/g, ' ');
    }

    function formatRating(rating: number): string {
        return rating > 0 ? `${rating} star${rating === 1 ? '' : 's'}` : 'Unrated';
    }

    function formatDecision(decision: string | null | undefined): string {
        if (!decision || decision === 'undecided') return 'Undecided';
        return decision.charAt(0).toUpperCase() + decision.slice(1);
    }

    function importMonthKey(importedAt: string | null | undefined): string {
        if (!importedAt) return 'unknown';
        const date = new Date(importedAt);
        if (Number.isNaN(date.getTime())) return 'unknown';
        return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`;
    }

    function megapixels(img: ImageWithFile | null | undefined): number {
        if (!img) return 0;
        return (img.image.width * img.image.height) / 1_000_000;
    }

    function resolutionBucket(img: ImageWithFile | null | undefined): { key: string; label: string; rank: number } {
        const mp = megapixels(img);
        if (mp >= 24) return { key: 'resolution:xxl', label: '24MP+', rank: 4 };
        if (mp >= 12) return { key: 'resolution:large', label: '12-24MP', rank: 3 };
        if (mp >= 4) return { key: 'resolution:medium', label: '4-12MP', rank: 2 };
        if (mp > 0) return { key: 'resolution:small', label: '<4MP', rank: 1 };
        return { key: 'resolution:unknown', label: 'Unknown size', rank: 0 };
    }

    function clamp(value: number, min: number, max: number): number {
        return Math.max(min, Math.min(max, value));
    }

    function computeProjectionCenter(sourcePoints: Point[]): { x: number; y: number } {
        if (sourcePoints.length === 0) return { x: 0, y: 0 };
        const xs = sourcePoints.map(point => point.x);
        const ys = sourcePoints.map(point => point.y);
        return {
            x: (Math.min(...xs) + Math.max(...xs)) / 2,
            y: (Math.min(...ys) + Math.max(...ys)) / 2,
        };
    }

    function computeZLayerRankRange(layers: ZLayer[]): { min: number; max: number } {
        if (layers.length === 0) return { min: 0, max: 0 };
        let min = Infinity;
        let max = -Infinity;
        for (const layer of layers) {
            if (!Number.isFinite(layer.rank)) continue;
            min = Math.min(min, layer.rank);
            max = Math.max(max, layer.rank);
        }
        if (!Number.isFinite(min) || !Number.isFinite(max)) return { min: 0, max: 0 };
        return { min, max };
    }

    function formatMultiplier(value: number): string {
        return `${value.toFixed(2)}x`;
    }

    function formatPercent(value: number): string {
        return `${Math.round(value * 100)}%`;
    }

    function matchingSpacePreset(values: SpacePresetValues): EmbeddingSpacePreset {
        const match = SPACE_PRESETS.find(preset =>
            Math.abs(preset.values.spacing - values.spacing) < 0.001
            && Math.abs(preset.values.depth - values.depth) < 0.001
            && Math.abs(preset.values.scale - values.scale) < 0.001
            && Math.abs(preset.values.perspective - values.perspective) < 0.001
        );
        return match?.id ?? 'custom';
    }

    function applySpacePreset(presetId: EmbeddingSpacePreset, persist: boolean = true) {
        const preset = SPACE_PRESETS.find(item => item.id === presetId);
        if (!preset) return;
        spacePreset = preset.id;
        spaceSpacing = preset.values.spacing;
        spaceDepth = preset.values.depth;
        spaceScale = preset.values.scale;
        spacePerspective = preset.values.perspective;
        requestDraw();
        if (persist) saveViewState();
    }

    function handleSpacePresetChange(e: Event) {
        const presetId = (e.currentTarget as HTMLSelectElement).value as EmbeddingSpacePreset;
        applySpacePreset(presetId);
    }

    function updateSpaceControl(key: keyof SpacePresetValues, value: number) {
        const next = {
            spacing: spaceSpacing,
            depth: spaceDepth,
            scale: spaceScale,
            perspective: spacePerspective,
            [key]: value,
        };
        spaceSpacing = next.spacing;
        spaceDepth = next.depth;
        spaceScale = next.scale;
        spacePerspective = next.perspective;
        spacePreset = matchingSpacePreset(next);
        requestDraw();
        saveViewState();
    }

    function layerForPoint(point: Point, preset: EmbeddingZPreset): ZLayer {
        const img = imageMap.get(point.id) ?? null;
        switch (preset) {
            case 'projection':
                return { key: 'projection:all', label: 'All Images', count: 0, rank: 0, color: null };
            case 'cluster': {
                const cluster = clusters.find(item => item.id === point.cluster);
                return {
                    key: `cluster:${point.cluster}`,
                    label: cluster?.label ?? `cluster ${point.cluster + 1}`,
                    count: 0,
                    rank: point.cluster,
                    color: cluster?.color ?? CLUSTER_COLORS[point.cluster % CLUSTER_COLORS.length],
                };
            }
            case 'source': {
                const source = img?.source_label ?? 'unknown';
                return {
                    key: `source:${source}`,
                    label: displaySourceLabel(img?.source_label ?? null),
                    count: 0,
                    rank: source === 'unknown' ? -1 : source.charCodeAt(0),
                    color: null,
                };
            }
            case 'rating': {
                const rating = img?.selection?.star_rating ?? 0;
                return {
                    key: `rating:${rating}`,
                    label: formatRating(rating),
                    count: 0,
                    rank: rating,
                    color: rating > 0 ? 'var(--orange)' : null,
                };
            }
            case 'decision': {
                const decision = img?.selection?.decision ?? 'undecided';
                const color = decision === 'accept' ? 'var(--green)' : decision === 'reject' ? 'var(--red)' : null;
                return {
                    key: `decision:${decision}`,
                    label: formatDecision(decision),
                    count: 0,
                    rank: decision === 'accept' ? 3 : decision === 'undecided' ? 2 : 1,
                    color,
                };
            }
            case 'recency': {
                const key = importMonthKey(img?.image.imported_at);
                return {
                    key: `recency:${key}`,
                    label: key === 'unknown' ? 'Unknown date' : key,
                    count: 0,
                    rank: key === 'unknown' ? 0 : Number(key.replace('-', '')),
                    color: null,
                };
            }
            case 'resolution': {
                const bucket = resolutionBucket(img);
                return { ...bucket, count: 0, color: null };
            }
        }
    }

    function buildZLayersForPreset(preset: EmbeddingZPreset): ZLayer[] {
        const byKey = new Map<string, ZLayer>();
        for (const point of points) {
            const layer = layerForPoint(point, preset);
            const existing = byKey.get(layer.key);
            if (existing) {
                existing.count += 1;
            } else {
                byKey.set(layer.key, { ...layer, count: 1 });
            }
        }
        return Array.from(byKey.values()).sort((a, b) => {
            if (preset === 'cluster') return a.rank - b.rank;
            return b.rank - a.rank || a.label.localeCompare(b.label);
        });
    }

    function pointLayerKey(point: Point, preset: EmbeddingZPreset = zPreset): string {
        return layerForPoint(point, preset).key;
    }

    function getPointsForLayer(layerKey: string | null | undefined): Point[] {
        if (!layerKey) return points;
        return points.filter(point => pointLayerKey(point) === layerKey);
    }

    function getNavigationPoints(): Point[] {
        const source = focusActiveLayer || interactionMode === 'stack'
            ? activeLayerPoints
            : points;
        return [...source].sort((a, b) => {
            const layerA = layerForPoint(a, zPreset);
            const layerB = layerForPoint(b, zPreset);
            if (layerA.rank !== layerB.rank) return layerB.rank - layerA.rank;
            if (a.y !== b.y) return a.y - b.y;
            return a.x - b.x;
        });
    }

    function getRenderPoints(): Point[] {
        return [...points].sort((a, b) => {
            return visualDepthForPoint(a) - visualDepthForPoint(b);
        });
    }

    function pointIsDimmed(point: Point): boolean {
        if (!focusActiveLayer && interactionMode !== 'stack') return false;
        if (!activeZLayerKey) return false;
        if (selectedPoint?.id === point.id || hoveredPoint?.id === point.id) return false;
        return pointLayerKey(point) !== activeZLayerKey;
    }

    function getPointLabel(point: Point): string {
        const img = imageMap.get(point.id);
        if (!img) return point.id;
        return img.path.split('/').pop() ?? point.id;
    }

    function shouldDrawCanvasLabel(point: Point, selected: boolean, hovered: boolean): boolean {
        if (!canvasLabelsOpen) return false;
        if (selected || hovered) return true;
        if (pointIsDimmed(point)) return false;
        return scale > 40 || activeLayerPoints.length <= 18;
    }

    function drawCanvasPointLabel(ctx: CanvasRenderingContext2D, point: Point, sx: number, sy: number, thumbSize: number) {
        let label = getPointLabel(point);
        ctx.save();
        ctx.font = '10px JetBrains Mono, monospace';
        const maxWidth = 180;
        while (label.length > 12 && ctx.measureText(label).width > maxWidth) {
            label = `${label.slice(0, -5)}...`;
        }
        const textWidth = Math.min(maxWidth, ctx.measureText(label).width);
        const labelX = sx - textWidth / 2 - 5;
        const labelY = sy + thumbSize / 2 + 8;
        ctx.fillStyle = 'rgba(12, 12, 18, 0.9)';
        ctx.fillRect(labelX, labelY, textWidth + 10, 17);
        ctx.fillStyle = '#e0e0e0';
        ctx.textAlign = 'center';
        ctx.fillText(label, sx, labelY + 12, maxWidth);
        ctx.restore();
    }

    function setInteractionMode(mode: EmbeddingInteractionMode) {
        interactionMode = mode;
        if (mode === 'stack') focusActiveLayer = true;
        if (mode === 'review') largePreviewOpen = true;
        if (mode === 'text') textOutputOpen = true;
        requestDraw();
        saveViewState();
    }

    function handleZPresetChange() {
        const layers = buildZLayersForPreset(zPreset);
        setActiveLayer(layers[0]?.key ?? null, false);
    }

    function setActiveLayer(layerKey: string | null, reveal: boolean = true) {
        activeZLayerKey = layerKey;
        if (zPreset === 'cluster' && layerKey?.startsWith('cluster:')) {
            highlightedCluster = Number(layerKey.split(':')[1]);
        } else if (zPreset !== 'cluster') {
            highlightedCluster = null;
        }

        const layerPoints = getPointsForLayer(layerKey);
        if (reveal && layerPoints.length > 0) {
            fitPointSet(layerPoints, layerPoints.length === points.length ? 60 : 90);
            if (!selectedPoint || !layerPoints.some(point => point.id === selectedPoint?.id)) {
                selectPoint(layerPoints[0], false);
                return;
            }
        }
        requestDraw();
        saveViewState();
    }

    function navigateZLayer(delta: number) {
        if (zLayers.length === 0) return;
        const rawIndex = zLayers.findIndex(layer => layer.key === activeZLayerKey);
        const currentIndex = rawIndex >= 0 ? rawIndex : (delta > 0 ? -1 : 0);
        const nextIndex = (currentIndex + delta + zLayers.length) % zLayers.length;
        setActiveLayer(zLayers[nextIndex].key, true);
    }

    function selectPoint(point: Point, reveal: boolean) {
        selectedPoint = point;
        focusImageForLoupe(point.id);
        if (reveal) centerPoint(point);
        requestDraw();
        saveViewState();
    }

    function selectRelativePoint(delta: number) {
        if (navigationPoints.length === 0) return;
        const currentIndex = navigationPoints.findIndex(point => point.id === selectedPoint?.id);
        const startIndex = currentIndex >= 0 ? currentIndex : (delta > 0 ? -1 : 0);
        const nextIndex = (startIndex + delta + navigationPoints.length) % navigationPoints.length;
        selectPoint(navigationPoints[nextIndex], interactionMode !== 'map' || largePreviewOpen);
    }

    function openNeighbor(neighbor: EmbeddingNeighbor) {
        const point = points.find(candidate => candidate.id === neighbor.image.image.id);
        if (point) {
            selectPoint(point, true);
            return;
        }
        focusedImageOverride.set(neighbor.image);
        navigateTo('loupe');
    }

    function fitPointSet(viewPoints: Point[], padding: number) {
        if (viewPoints.length === 0) return;
        const xs = viewPoints.map(p => p.x);
        const ys = viewPoints.map(p => p.y);
        const minX = Math.min(...xs);
        const maxX = Math.max(...xs);
        const minY = Math.min(...ys);
        const maxY = Math.max(...ys);
        const rangeX = maxX - minX || 1;
        const rangeY = maxY - minY || 1;
        const scaleX = (canvasWidth - padding * 2) / rangeX;
        const scaleY = (canvasHeight - padding * 2) / rangeY;
        scale = Math.min(scaleX, scaleY);
        panX = canvasWidth / 2 - ((minX + maxX) / 2) * scale;
        panY = canvasHeight / 2 - ((minY + maxY) / 2) * scale;
    }

    function centerPoint(point: Point) {
        const targetScale = Math.max(scale, 220);
        const targetPanX = canvasWidth / 2 - point.x * targetScale;
        const targetPanY = canvasHeight / 2 - point.y * targetScale;
        animateViewport(targetScale, targetPanX, targetPanY, 180);
    }

    function animateViewport(targetScale: number, targetPanX: number, targetPanY: number, duration: number) {
        const startScale = scale;
        const startPanX = panX;
        const startPanY = panY;
        const startTime = performance.now();

        function animate(now: number) {
            const t = Math.min((now - startTime) / duration, 1);
            const ease = 1 - (1 - t) * (1 - t);
            scale = startScale + (targetScale - startScale) * ease;
            panX = startPanX + (targetPanX - startPanX) * ease;
            panY = startPanY + (targetPanY - startPanY) * ease;
            requestDraw();
            if (t < 1) requestAnimationFrame(animate);
        }
        requestAnimationFrame(animate);
    }

    function buildTextOutput(): string {
        if (!selectedImage) {
            const layerLabel = activeZLayer?.label ?? 'All Images';
            return [
                `preset: ${Z_PRESETS.find(preset => preset.id === zPreset)?.label ?? zPreset}`,
                `space: ${SPACE_PRESETS.find(preset => preset.id === spacePreset)?.label ?? 'Custom'}`,
                `layer: ${layerLabel}`,
                `images: ${activeLayerPoints.length}`,
            ].join('\n');
        }

        const rating = selectedImage.selection?.star_rating ?? 0;
        const decision = selectedImage.selection?.decision ?? 'undecided';
        const source = displaySourceLabel(selectedImage.source_label);
        const lines = [
            `file: ${selectedFilename}`,
            `id: ${selectedImage.image.id}`,
            `dimensions: ${selectedImage.image.width} x ${selectedImage.image.height}`,
            `format: ${selectedImage.image.format}`,
            `source: ${source}`,
            `rating: ${formatRating(rating)}`,
            `decision: ${formatDecision(decision)}`,
            `z-preset: ${Z_PRESETS.find(preset => preset.id === zPreset)?.label ?? zPreset}`,
            `z-layer: ${activeZLayer?.label ?? 'All Images'}`,
            `space: ${SPACE_PRESETS.find(preset => preset.id === spacePreset)?.label ?? 'Custom'}`,
        ];
        if (selectedGenerationRun?.provider || selectedGenerationRun?.model || selectedGenerationRun?.seed) {
            lines.push(`generation: ${[
                selectedGenerationRun.provider,
                selectedGenerationRun.model,
                selectedGenerationRun.seed ? `seed ${selectedGenerationRun.seed}` : null,
            ].filter(Boolean).join(' / ')}`);
        }
        if (selectedPrompt) lines.push(`prompt:\n${selectedPrompt}`);
        return lines.join('\n');
    }

    async function copyTextOutput() {
        try {
            await navigator.clipboard.writeText(textOutput);
        } catch (e) {
            console.error('Failed to copy embedding text output:', e);
        }
    }

    function isInteractiveTarget(target: EventTarget | null): boolean {
        const el = target as HTMLElement | null;
        return !!el?.closest('input, textarea, select, button, [contenteditable="true"]');
    }

    function handleExplorerKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape' && areaSelectionMode) {
            e.preventDefault();
            resetAreaSelection();
            explorerEl?.focus();
            return;
        }
        if (isInteractiveTarget(e.target)) return;
        if (points.length === 0) return;
        if ((e.key === ' ' || e.code === 'Space') && areaSelectionMode && selectedPoint) {
            e.preventDefault();
            togglePointInAreaSelection(selectedPoint.id);
        } else if (e.key === 'ArrowRight') {
            e.preventDefault();
            selectRelativePoint(1);
            if (areaSelectionMode && e.shiftKey && selectedPoint) addPointToAreaSelection(selectedPoint.id);
        } else if (e.key === 'ArrowLeft') {
            e.preventDefault();
            selectRelativePoint(-1);
            if (areaSelectionMode && e.shiftKey && selectedPoint) addPointToAreaSelection(selectedPoint.id);
        } else if (e.key === 'ArrowDown') {
            e.preventDefault();
            navigateZLayer(1);
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            navigateZLayer(-1);
        } else if (e.key === 'Enter' && selectedPoint) {
            e.preventDefault();
            handleFocusInGrid();
        } else if (e.key.toLowerCase() === 'p') {
            largePreviewOpen = !largePreviewOpen;
            saveViewState();
        } else if (e.key.toLowerCase() === 't') {
            textOutputOpen = !textOutputOpen;
            saveViewState();
        }
    }

    function focusCluster(clusterId: number) {
        highlightedCluster = highlightedCluster === clusterId ? null : clusterId;
        if (zPreset === 'cluster') {
            activeZLayerKey = highlightedCluster === null ? null : `cluster:${clusterId}`;
        }
        if (highlightedCluster !== null) {
            // Pan/zoom to fit the cluster
            const clusterPts = points.filter(p => p.cluster === clusterId);
            if (clusterPts.length > 0) {
                const xs = clusterPts.map(p => p.x);
                const ys = clusterPts.map(p => p.y);
                const minX = Math.min(...xs);
                const maxX = Math.max(...xs);
                const minY = Math.min(...ys);
                const maxY = Math.max(...ys);
                const rangeX = maxX - minX || 1;
                const rangeY = maxY - minY || 1;
                const padding = 120;
                const scaleX = (canvasWidth - padding * 2) / rangeX;
                const scaleY = (canvasHeight - padding * 2) / rangeY;
                scale = Math.min(scaleX, scaleY);
                panX = canvasWidth / 2 - ((minX + maxX) / 2) * scale;
                panY = canvasHeight / 2 - ((minY + maxY) / 2) * scale;
            }
        }
        requestDraw();
        saveViewState();
    }

    interface ProjectionWorkerCluster {
        id: number;
        label: string;
        count: number;
        colorIndex: number;
        previewPaths: string[];
        x: number;
        y: number;
    }

    interface ProjectionWorkerResponse {
        requestId: number;
        points: Point[];
        clusters: ProjectionWorkerCluster[];
        projectionKey: string;
    }

    function getProjectionWorker(): Worker {
        if (!projectionWorker) {
            projectionWorker = new Worker(new URL('../embedding-projection.worker.ts', import.meta.url), {
                type: 'module',
            });
        }
        return projectionWorker;
    }

    function runProjectionInWorker(
        embeddingPage: EmbeddingPage,
        embeddingImages: ImageWithFile[],
    ): Promise<ProjectionWorkerResponse> {
        const worker = getProjectionWorker();
        const requestId = ++projectionRequestId;
        const vectors = new Float32Array(embeddingPage.vectors);

        return new Promise((resolve, reject) => {
            cancelProjectionWork = () => {
                cleanup();
                reject(new Error('Projection cancelled'));
            };
            const handleMessage = (event: MessageEvent<ProjectionWorkerResponse>) => {
                if (event.data.requestId !== requestId) return;
                cleanup();
                resolve(event.data);
            };
            const handleError = (event: ErrorEvent) => {
                cleanup();
                reject(event.error ?? new Error(event.message));
            };
            const cleanup = () => {
                if (cancelProjectionWork) cancelProjectionWork = null;
                worker.removeEventListener('message', handleMessage as EventListener);
                worker.removeEventListener('error', handleError);
            };

            worker.addEventListener('message', handleMessage as EventListener);
            worker.addEventListener('error', handleError);
            worker.postMessage({
                requestId,
                provider: selectedProvider,
                ids: embeddingPage.ids,
                vectors,
                dims: embeddingPage.dims,
                images: embeddingImages.map(img => ({
                    id: img.image.id,
                    path: img.path,
                    thumbnailPath: safeAssetPreviewPath(img),
                })),
            }, [vectors.buffer]);
        });
    }

    function resetProjectionWorker() {
        const cancel = cancelProjectionWork;
        cancelProjectionWork = null;
        cancel?.();
        projectionWorker?.terminate();
        projectionWorker = null;
    }

    async function loadProjection(scope: LibraryScope = get(libraryScope)) {
        const loadSeq = ++projectionLoadSeq;
        try {
            const modelName = modelNameForProvider(selectedProvider);
            const requestedScopeKey = libraryScopeKey(scope);
            const embeddingPage = await getEmbeddingPageForScope(
                scope,
                modelName,
                PROJECTION_EMBEDDING_LIMIT,
                0,
            );
            if (loadSeq !== projectionLoadSeq) return;
            if (requestedScopeKey !== libraryScopeKey(get(libraryScope))) return;
            if (embeddingPage.ids.length < 2) {
                resetAreaSelection();
                points = [];
                clusters = [];
                resetThumbnailCache();
                return;
            }

            // Build image map from the projected embedding page.
            const embeddingIds = embeddingPage.ids;
            const embeddingImages = await getImagesByIds(embeddingIds);
            if (loadSeq !== projectionLoadSeq) return;
            const map = new Map<string, ImageWithFile>();
            for (const img of embeddingImages) {
                map.set(img.image.id, img);
            }
            imageMap = map;
            staleEmbeddingCount = embeddingIds.length - embeddingImages.length;

            resetProjectionWorker();
            projecting = true;
            const projection = await runProjectionInWorker(embeddingPage, embeddingImages);
            if (loadSeq !== projectionLoadSeq) return;

            resetAreaSelection();
            points = projection.points;
            clusters = projection.clusters.map(cluster => ({
                id: cluster.id,
                label: cluster.label,
                count: cluster.count,
                color: CLUSTER_COLORS[cluster.colorIndex % CLUSTER_COLORS.length],
                previewPaths: cluster.previewPaths,
                x: cluster.x,
                y: cluster.y,
            }));

            const newProjectionKey = projection.projectionKey;
            projectionKey = newProjectionKey;

            // Restore view state if it matches the current projection
            const savedState = get(embeddingViewState);
            if (savedState.hasUserView && savedState.provider === selectedProvider && savedState.projectionKey === newProjectionKey) {
                panX = savedState.panX;
                panY = savedState.panY;
                scale = savedState.scale;
                highlightedCluster = savedState.highlightedCluster;
                selectedPoint = savedState.selectedPointId
                    ? points.find(p => p.id === savedState.selectedPointId) ?? null
                    : null;
            } else {
                highlightedCluster = null;
                fitView();
            }

            resetThumbnailCache();
            requestDraw();
        } catch (e) {
            if (e instanceof Error && e.message === 'Projection cancelled') return;
            console.error('Failed to load projection:', e);
        } finally {
            if (loadSeq === projectionLoadSeq) projecting = false;
        }
    }

    function fitView() {
        if (points.length === 0) return;
        const xs = points.map(p => p.x);
        const ys = points.map(p => p.y);
        const minX = Math.min(...xs);
        const maxX = Math.max(...xs);
        const minY = Math.min(...ys);
        const maxY = Math.max(...ys);
        const rangeX = maxX - minX || 1;
        const rangeY = maxY - minY || 1;
        const padding = 60;
        const scaleX = (canvasWidth - padding * 2) / rangeX;
        const scaleY = (canvasHeight - padding * 2) / rangeY;
        scale = Math.min(scaleX, scaleY);
        panX = canvasWidth / 2 - ((minX + maxX) / 2) * scale;
        panY = canvasHeight / 2 - ((minY + maxY) / 2) * scale;
    }

    function screenToWorld(sx: number, sy: number): [number, number] {
        return [(sx - panX) / scale, (sy - panY) / scale];
    }

    function worldToScreen(wx: number, wy: number): [number, number] {
        return [wx * scale + panX, wy * scale + panY];
    }

    function baseLayerDepth(point: Point): number {
        if (zLayers.length <= 1 || zLayerRankRange.max === zLayerRankRange.min) return 0;
        const layer = layerForPoint(point, zPreset);
        return ((layer.rank - zLayerRankRange.min) / (zLayerRankRange.max - zLayerRankRange.min)) * 2 - 1;
    }

    function visualDepthForPoint(point: Point): number {
        let depth = baseLayerDepth(point);
        if (highlightedCluster !== null && point.cluster === highlightedCluster) depth += 0.45;
        if (activeZLayerKey && pointLayerKey(point) === activeZLayerKey) depth += 0.75;
        if (selectedPoint?.id === point.id) depth += 1.05;
        if (hoveredPoint?.id === point.id) depth += 1.2;
        return clamp(depth, -1.2, 2.2);
    }

    function projectPointForCanvas(point: Point): { sx: number; sy: number; depth: number; perspectiveScale: number } {
        const spacedX = projectionCenter.x + (point.x - projectionCenter.x) * spaceSpacing;
        const spacedY = projectionCenter.y + (point.y - projectionCenter.y) * spaceSpacing;
        const [flatX, flatY] = worldToScreen(spacedX, spacedY);
        const depth = visualDepthForPoint(point) * spaceDepth;
        const perspectiveScale = clamp(1 + depth * spacePerspective * 0.22, 0.55, 1.7);
        const vanishingX = canvasWidth * 0.5;
        const vanishingY = canvasHeight * 0.42;
        return {
            sx: vanishingX + (flatX - vanishingX) * perspectiveScale + depth * 12,
            sy: vanishingY + (flatY - vanishingY) * perspectiveScale - depth * 42,
            depth,
            perspectiveScale,
        };
    }

    function requestDraw() {
        if (drawQueued) return;
        if (typeof requestAnimationFrame !== 'undefined') {
            drawQueued = true;
            requestAnimationFrame(() => {
                drawQueued = false;
                draw();
            });
        }
    }

    function draw() {
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        ctx.clearRect(0, 0, canvasWidth, canvasHeight);

        const { size: baseThumbSize, useThumb } = computeScatterThumbSize(scale, points.length);
        const renderPoints = getRenderPoints();

        // Draw background layers first so active z-layers and selected items stay inspectable.
        const margin = baseThumbSize * Math.max(1, spaceScale * (1 + spaceDepth * spacePerspective)) + 64;
        for (const p of renderPoints) {
            const { sx, sy, depth, perspectiveScale } = projectPointForCanvas(p);
            const thumbSize = baseThumbSize * spaceScale * perspectiveScale;
            if (sx < -margin || sx > canvasWidth + margin || sy < -margin || sy > canvasHeight + margin) continue;

            const color = CLUSTER_COLORS[p.cluster % CLUSTER_COLORS.length];
            const isSelected = areaSelectedIds.has(p.id) || (selectedPoint && selectedPoint.id === p.id);
            const isHovered = hoveredPoint && hoveredPoint.id === p.id;
            const dimmed = pointIsDimmed(p);

            const thumbEl = useThumb ? pickThumbnail(p.id, thumbSize) : undefined;
            const depthAlpha = clamp(0.62 + depth * 0.18, 0.38, 1);
            ctx.globalAlpha = dimmed ? 0.22 : depthAlpha;

            if (thumbEl && thumbEl.complete && thumbEl.naturalWidth > 0) {
                const aspect = thumbEl.naturalWidth / thumbEl.naturalHeight;
                let dw: number, dh: number;
                if (aspect >= 1) {
                    dw = thumbSize;
                    dh = thumbSize / aspect;
                } else {
                    dh = thumbSize;
                    dw = thumbSize * aspect;
                }

                const dpr = window.devicePixelRatio || 1;
                const dx = Math.round((sx - dw / 2) * dpr) / dpr;
                const dy = Math.round((sy - dh / 2) * dpr) / dpr;
                const drawW = Math.round(dw * dpr) / dpr;
                const drawH = Math.round(dh * dpr) / dpr;
                ctx.drawImage(thumbEl, dx, dy, drawW, drawH);

                if (isSelected || isHovered) {
                    ctx.strokeStyle = '#ffffff';
                    ctx.lineWidth = isSelected ? 2 : 1;
                    ctx.strokeRect(sx - dw / 2, sy - dh / 2, dw, dh);
                }
            } else {
                // Colored dot fallback
                const radius = Math.max(2, Math.min(6, (4 / Math.sqrt(scale)) * spaceScale * perspectiveScale));
                ctx.fillStyle = color;
                ctx.beginPath();
                ctx.arc(sx, sy, radius, 0, Math.PI * 2);
                ctx.fill();

                if (isSelected || isHovered) {
                    ctx.strokeStyle = '#ffffff';
                    ctx.lineWidth = 1;
                    ctx.stroke();
                }
            }

            if (shouldDrawCanvasLabel(p, !!isSelected, !!isHovered)) {
                drawCanvasPointLabel(ctx, p, sx, sy, thumbSize);
            }
            ctx.globalAlpha = 1;
        }

        // Draw cluster labels at centroid when zoomed out enough
        if (highlightedCluster === null && scale < 5) {
            ctx.save();
            ctx.font = 'bold 11px JetBrains Mono, monospace';
            ctx.textAlign = 'center';
            for (const cluster of clusters) {
                const clusterX = projectionCenter.x + (cluster.x - projectionCenter.x) * spaceSpacing;
                const clusterY = projectionCenter.y + (cluster.y - projectionCenter.y) * spaceSpacing;
                const [sx, sy] = worldToScreen(clusterX, clusterY);
                // Background pill
                const text = cluster.label;
                const tw = ctx.measureText(text).width;
                ctx.fillStyle = 'rgba(0, 0, 0, 0.6)';
                ctx.beginPath();
                const pillX = sx - tw / 2 - 6;
                const pillY = sy - baseThumbSize / 2 - 22;
                const pillW = tw + 12;
                const pillH = 18;
                const r = 4;
                ctx.moveTo(pillX + r, pillY);
                ctx.lineTo(pillX + pillW - r, pillY);
                ctx.quadraticCurveTo(pillX + pillW, pillY, pillX + pillW, pillY + r);
                ctx.lineTo(pillX + pillW, pillY + pillH - r);
                ctx.quadraticCurveTo(pillX + pillW, pillY + pillH, pillX + pillW - r, pillY + pillH);
                ctx.lineTo(pillX + r, pillY + pillH);
                ctx.quadraticCurveTo(pillX, pillY + pillH, pillX, pillY + pillH - r);
                ctx.lineTo(pillX, pillY + r);
                ctx.quadraticCurveTo(pillX, pillY, pillX + r, pillY);
                ctx.fill();
                ctx.fillStyle = cluster.color;
                ctx.fillText(text, sx, pillY + 13);
            }
            ctx.restore();
        }

        // Draw hover tooltip
        if (hoveredPoint) {
            const { sx, sy } = projectPointForCanvas(hoveredPoint);
            const img = imageMap.get(hoveredPoint.id);
            if (img) {
                const name = img.path.split('/').pop() || img.image.id;
                ctx.fillStyle = 'rgba(0, 0, 0, 0.8)';
                ctx.font = '11px JetBrains Mono, monospace';
                const textWidth = ctx.measureText(name).width;
                const tx = Math.min(sx + 12, canvasWidth - textWidth - 16);
                const ty = Math.max(sy - 8, 20);
                ctx.fillRect(tx - 4, ty - 12, textWidth + 8, 18);
                ctx.fillStyle = '#e0e0e0';
                ctx.fillText(name, tx, ty);
            }
        }

        if (areaSelectionRect) {
            const left = Math.min(areaSelectionRect.startX, areaSelectionRect.currentX);
            const top = Math.min(areaSelectionRect.startY, areaSelectionRect.currentY);
            const width = Math.abs(areaSelectionRect.currentX - areaSelectionRect.startX);
            const height = Math.abs(areaSelectionRect.currentY - areaSelectionRect.startY);
            const selectionColor = getComputedStyle(canvas).getPropertyValue('--blue').trim();
            if (selectionColor) {
                ctx.save();
                ctx.strokeStyle = selectionColor;
                ctx.lineWidth = 2;
                ctx.strokeRect(left, top, width, height);
                ctx.restore();
            }
        }
    }

    function saveViewState() {
        embeddingViewState.set({
            panX, panY, scale,
            selectedPointId: selectedPoint?.id ?? null,
            highlightedCluster,
            provider: selectedProvider,
            projectionKey,
            hasUserView: true,
            interactionMode,
            zPreset,
            activeZLayerKey,
            focusActiveLayer,
            largePreviewOpen,
            textOutputOpen,
            canvasLabelsOpen,
            spacePreset,
            spaceSpacing,
            spaceDepth,
            spaceScale,
            spacePerspective,
        });
    }

    function handleWheel(e: WheelEvent) {
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;

        const factor = e.deltaY > 0 ? 0.9 : 1.1;
        const newScale = scale * factor;

        // Zoom toward cursor
        panX = mx - (mx - panX) * (newScale / scale);
        panY = my - (my - panY) * (newScale / scale);
        scale = newScale;

        requestDraw();
        saveViewState();
    }

    function handleMouseDown(e: MouseEvent) {
        explorerEl?.focus();
        if (areaSelectionMode) {
            if (e.button !== 0) return;
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            areaSelectionRect = { startX: x, startY: y, currentX: x, currentY: y };
            hoveredPoint = null;
            requestDraw();
            return;
        }
        dragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        dragStartPanX = panX;
        dragStartPanY = panY;
    }

    function handleMouseMove(e: MouseEvent) {
        if (areaSelectionRect) {
            const rect = canvas.getBoundingClientRect();
            areaSelectionRect = {
                ...areaSelectionRect,
                currentX: e.clientX - rect.left,
                currentY: e.clientY - rect.top,
            };
            requestDraw();
            return;
        }
        if (areaSelectionMode) {
            hoveredPoint = null;
            canvas.style.cursor = 'crosshair';
            return;
        }
        if (dragging) {
            panX = dragStartPanX + (e.clientX - dragStartX);
            panY = dragStartPanY + (e.clientY - dragStartY);
            requestDraw();
            return;
        }

        // Hit test - use thumbnail size for hit area when thumbnails are visible
        const rect = canvas.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const { size: baseThumbSize } = computeScatterThumbSize(scale, points.length);

        let found: Point | null = null;
        const renderPoints = getRenderPoints();
        for (let i = renderPoints.length - 1; i >= 0; i--) {
            const p = renderPoints[i];
            const { sx, sy, perspectiveScale } = projectPointForCanvas(p);
            const hitHalf = Math.max(6, (baseThumbSize * spaceScale * perspectiveScale) / 2);
            if (Math.abs(mx - sx) < hitHalf && Math.abs(my - sy) < hitHalf) {
                found = p;
                break;
            }
        }

        if (found !== hoveredPoint) {
            hoveredPoint = found;
            canvas.style.cursor = found ? 'pointer' : 'grab';
            requestDraw();
        }
    }

    function handleMouseUp(e: MouseEvent) {
        if (areaSelectionRect) {
            const rect = canvas.getBoundingClientRect();
            const endX = e.clientX - rect.left;
            const endY = e.clientY - rect.top;
            const left = Math.min(areaSelectionRect.startX, endX);
            const right = Math.max(areaSelectionRect.startX, endX);
            const top = Math.min(areaSelectionRect.startY, endY);
            const bottom = Math.max(areaSelectionRect.startY, endY);
            areaSelectedIds = new Set(getRenderPoints()
                .filter(point => {
                    const { sx, sy } = projectPointForCanvas(point);
                    return sx >= left && sx <= right && sy >= top && sy <= bottom;
                })
                .map(point => point.id));
            areaSelectionRect = null;
            requestDraw();
            return;
        }
        if (dragging) {
            const moved = Math.abs(e.clientX - dragStartX) + Math.abs(e.clientY - dragStartY);
            if (moved < 4 && hoveredPoint) {
                handlePointClick(hoveredPoint);
            }
        }
        dragging = false;
        saveViewState();
    }

    function toggleAreaSelection() {
        areaSelectionMode = !areaSelectionMode;
        areaSelectionRect = null;
        if (!areaSelectionMode) areaSelectedIds = new Set();
        hoveredPoint = null;
        requestDraw();
        if (areaSelectionMode) explorerEl?.focus();
    }

    function addPointToAreaSelection(imageId: string) {
        areaSelectedIds = new Set([...areaSelectedIds, imageId]);
        requestDraw();
    }

    function togglePointInAreaSelection(imageId: string) {
        const next = new Set(areaSelectedIds);
        if (next.has(imageId)) {
            next.delete(imageId);
        } else {
            next.add(imageId);
        }
        areaSelectedIds = next;
        requestDraw();
    }

    function clearAreaSelection() {
        areaSelectionRect = null;
        areaSelectedIds = new Set();
        requestDraw();
    }

    function resetAreaSelection() {
        areaSelectionMode = false;
        clearAreaSelection();
    }

    function cancelAreaSelectionDrag() {
        dragging = false;
        areaSelectionRect = null;
        hoveredPoint = null;
        requestDraw();
    }

    async function createCollectionFromAreaSelection() {
        const imageIds = getRenderPoints()
            .filter(point => areaSelectedIds.has(point.id))
            .map(point => point.id);
        if (imageIds.length === 0 || creatingAreaCollection) return;
        const imageLabel = imageIds.length === 1 ? '1 selected image' : `${imageIds.length} selected images`;
        const name = await requestTextInput({
            title: 'New Collection from Embedding Map',
            label: 'Collection name',
            description: `${imageLabel} will be added.`,
            placeholder: 'Collection name',
            confirmLabel: 'Create Collection',
        });
        if (!name?.trim()) return;

        creatingAreaCollection = true;
        try {
            await createCollectionWithImages(name.trim(), imageIds);
            areaSelectedIds = new Set();
            areaSelectionMode = false;
            showToast(`Collection "${name.trim()}" created`, { type: 'success', duration: 5000 });
            try {
                collections.set(await listCollections($showRejected));
            } catch (refreshError) {
                console.error('Collection created, but the collection list could not refresh:', refreshError);
            }
        } catch (error) {
            console.error('Failed to create collection from embedding selection:', error);
            showToast('Failed to create collection', { type: 'error' });
        } finally {
            creatingAreaCollection = false;
            requestDraw();
        }
    }

    function focusImageForLoupe(imageId: string): boolean {
        const idx = $images.findIndex(img => img.image.id === imageId);
        if (idx >= 0) {
            focusedImageOverride.set(null);
            focusedIndex.set(idx);
            return true;
        }
        const img = imageMap.get(imageId);
        if (!img) return false;
        focusedImageOverride.set(img);
        return true;
    }

    function handlePointClick(point: Point) {
        selectedPoint = point;
        focusImageForLoupe(point.id);
        zoomToPoint(point);
        saveViewState();
    }

    function zoomToPoint(point: Point) {
        const targetScale = Math.max(scale * 2.5, 800);
        const targetPanX = canvasWidth / 2 - point.x * targetScale;
        const targetPanY = canvasHeight / 2 - point.y * targetScale;

        const startScale = scale;
        const startPanX = panX;
        const startPanY = panY;
        const duration = 300;
        const startTime = performance.now();

        function animate(now: number) {
            const t = Math.min((now - startTime) / duration, 1);
            const ease = 1 - (1 - t) * (1 - t);
            scale = startScale + (targetScale - startScale) * ease;
            panX = startPanX + (targetPanX - startPanX) * ease;
            panY = startPanY + (targetPanY - startPanY) * ease;
            requestDraw();
            if (t < 1) requestAnimationFrame(animate);
        }
        requestAnimationFrame(animate);
    }

    function handleFocusInGrid() {
        if (selectedPoint && focusImageForLoupe(selectedPoint.id)) {
            navigateTo('loupe');
        }
    }

    function handleCanvasDblClick(e: MouseEvent) {
        if (areaSelectionMode) return;
        if (!hoveredPoint) return;
        if (focusImageForLoupe(hoveredPoint.id)) {
            navigateTo('loupe');
        }
    }

    function handleResize(node: HTMLDivElement) {
        const observer = new ResizeObserver(entries => {
            for (const entry of entries) {
                canvasWidth = entry.contentRect.width;
                canvasHeight = entry.contentRect.height;
                if (canvas) {
                    const dpr = window.devicePixelRatio || 1;
                    canvas.width = canvasWidth * dpr;
                    canvas.height = canvasHeight * dpr;
                    canvas.style.width = canvasWidth + 'px';
                    canvas.style.height = canvasHeight + 'px';
                    const ctx = canvas.getContext('2d');
                    if (ctx) {
                        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
                    }
                }
                if (points.length > 0) {
                    const savedState = get(embeddingViewState);
                    if (!savedState.hasUserView || savedState.provider !== selectedProvider || savedState.projectionKey !== projectionKey) {
                        fitView();
                    }
                    requestDraw();
                }
            }
        });
        observer.observe(node);
        return {
            destroy() { observer.disconnect(); }
        };
    }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
    class="embedding-explorer"
    class:zen={$zenMode}
    bind:this={explorerEl}
    onkeydown={handleExplorerKeydown}
    role="application"
    aria-label="Visual embeddings"
    tabindex="0"
>
    {#if !$zenMode}
    <div class="left-panel">
        <div class="panel-section">
            <div class="section-header-row">
                <div class="section-header">MODEL</div>
                <button
                    class="gear-btn"
                    onclick={toggleEmbeddingConfig}
                    aria-label={configOpen ? 'Close embedding configuration' : 'Configure embedding model'}
                    aria-expanded={configOpen}
                    title={configOpen ? 'Close configuration' : 'Configure embedding model'}
                >
                    &#9881;
                </button>
            </div>
            <div class="model-summary">
                <span class="model-summary-name">{selectedModel.label}</span>
                <span class="model-summary-count">{currentEmbeddingCount}/{totalImages} images embedded</span>
            </div>

            {#if configOpen}
                <label class="provider-config-row" for="embedding-provider">
                    <span>Provider</span>
                    <select
                        id="embedding-provider"
                        class="provider-select"
                        value={selectedProvider}
                        onchange={handleProviderSelection}
                        aria-label="Embedding provider"
                    >
                        {#each modelOptions as option}
                            <option value={option.id}>{option.label}</option>
                        {/each}
                    </select>
                </label>
                <div class="provider-config-meta">
                    <span>{selectedModel.scope}</span>
                    <span>{selectedModel.dims}</span>
                    <span class:ready={selectedModelAvailable}>{providerStatusLabel(selectedProvider)}</span>
                </div>
            {/if}
        </div>

        {#if configOpen && !isLocalProvider(selectedProvider)}
            <div class="panel-section config-section">
                {#if selectedProvider === 'ollama'}
                    <div class="section-header">OLLAMA EMBEDDINGS</div>
                    <label class="provider-field">
                        <span>URL</span>
                        <input aria-label="Ollama embedding URL" bind:value={ollamaEmbeddingUrl} disabled={providerConfigStatus === 'loading'} />
                    </label>
                    <label class="provider-field">
                        <span>Model</span>
                        <input aria-label="Ollama embedding model" bind:value={ollamaEmbeddingModel} disabled={providerConfigStatus === 'loading'} />
                    </label>
                {:else}
                    <div class="section-header">{selectedModel.shortLabel.toUpperCase()} CREDENTIALS</div>
                    <label class="provider-field">
                        <span>API key</span>
                        <input
                            type="password"
                            aria-label="{selectedModel.shortLabel} API key"
                            placeholder={selectedModelAvailable ? 'Connected · enter to replace' : 'Enter API key'}
                            bind:value={providerKeyInput}
                        />
                    </label>
                {/if}
                <button
                    class="settings-link-btn"
                    onclick={saveSelectedProviderConfig}
                    disabled={providerConfigStatus === 'loading' || providerConfigStatus === 'saving'}
                >
                    {providerConfigStatus === 'loading' ? 'Loading…' : providerConfigStatus === 'saving' ? 'Saving…' : selectedProvider === 'ollama' ? 'Save Ollama config' : 'Save API key'}
                </button>
                {#if providerConfigStatus === 'saved'}
                    <p class="provider-config-message ready" role="status">Configuration saved.</p>
                {:else if providerConfigStatus === 'invalid'}
                    <p class="provider-config-message error" role="alert">Enter a valid API key.</p>
                {:else if providerConfigStatus === 'error'}
                    <p class="provider-config-message error" role="alert">Could not save provider configuration.</p>
                {/if}
                <button class="settings-link-btn secondary-link" onclick={() => openSettings('ai')}>More AI settings</button>
            </div>
        {/if}

        <div class="panel-section visual-embed-section">
            <div class="section-header">VISUAL EMBEDS</div>
            <div class="mode-grid">
                {#each INTERACTION_MODES as mode}
                    <button
                        class="mode-btn"
                        class:active={interactionMode === mode.id}
                        onclick={() => setInteractionMode(mode.id)}
                        title={mode.title}
                    >
                        {mode.label}
                    </button>
                {/each}
            </div>

            <label class="control-label" for="embedding-z-preset">Z PRESET</label>
            <select id="embedding-z-preset" class="provider-select" bind:value={zPreset} onchange={handleZPresetChange}>
                {#each Z_PRESETS as preset}
                    <option value={preset.id}>{preset.label}</option>
                {/each}
            </select>

            <label class="control-label" for="embedding-space-preset">SPACE PRESET</label>
            <select
                id="embedding-space-preset"
                class="provider-select"
                value={spacePreset}
                onchange={handleSpacePresetChange}
                title={SPACE_PRESETS.find(preset => preset.id === spacePreset)?.title ?? 'Custom visual spacing'}
            >
                {#each SPACE_PRESETS as preset}
                    <option value={preset.id}>{preset.label}</option>
                {/each}
                {#if spacePreset === 'custom'}
                    <option value="custom">Custom</option>
                {/if}
            </select>

            <div class="space-control-panel">
                <label class="range-control">
                    <span class="range-header">
                        <span>Spacing</span>
                        <span class="range-value">{formatMultiplier(spaceSpacing)}</span>
                    </span>
                    <input
                        type="range"
                        min="0.6"
                        max="1.8"
                        step="0.05"
                        value={spaceSpacing}
                        oninput={(e) => updateSpaceControl('spacing', (e.currentTarget as HTMLInputElement).valueAsNumber)}
                    />
                </label>
                <label class="range-control">
                    <span class="range-header">
                        <span>Z depth</span>
                        <span class="range-value">{formatPercent(spaceDepth)}</span>
                    </span>
                    <input
                        type="range"
                        min="0"
                        max="1"
                        step="0.05"
                        value={spaceDepth}
                        oninput={(e) => updateSpaceControl('depth', (e.currentTarget as HTMLInputElement).valueAsNumber)}
                    />
                </label>
                <label class="range-control">
                    <span class="range-header">
                        <span>Scale</span>
                        <span class="range-value">{formatMultiplier(spaceScale)}</span>
                    </span>
                    <input
                        type="range"
                        min="0.55"
                        max="1.75"
                        step="0.05"
                        value={spaceScale}
                        oninput={(e) => updateSpaceControl('scale', (e.currentTarget as HTMLInputElement).valueAsNumber)}
                    />
                </label>
                <label class="range-control">
                    <span class="range-header">
                        <span>Perspective</span>
                        <span class="range-value">{formatPercent(spacePerspective)}</span>
                    </span>
                    <input
                        type="range"
                        min="0"
                        max="1"
                        step="0.05"
                        value={spacePerspective}
                        oninput={(e) => updateSpaceControl('perspective', (e.currentTarget as HTMLInputElement).valueAsNumber)}
                    />
                </label>
            </div>

            <div class="layer-row">
                <button class="layer-step-btn" onclick={() => navigateZLayer(-1)} disabled={zLayers.length === 0} title="Previous z-layer">↑</button>
                <select
                    class="provider-select layer-select"
                    value={activeZLayerKey ?? ''}
                    onchange={(e) => setActiveLayer((e.currentTarget as HTMLSelectElement).value || null)}
                >
                    <option value="">All Images</option>
                    {#each zLayers as layer}
                        <option value={layer.key}>{layer.label} ({layer.count})</option>
                    {/each}
                </select>
                <button class="layer-step-btn" onclick={() => navigateZLayer(1)} disabled={zLayers.length === 0} title="Next z-layer">↓</button>
            </div>

            <div class="layer-meta">
                <span class="cluster-dot" style="background: {activeZLayer?.color ?? 'var(--text-secondary)'}"></span>
                {activeZLayer?.label ?? 'All Images'}
                <span class="cluster-count">({activeLayerPoints.length})</span>
            </div>

            <label class="toggle-row">
                <input type="checkbox" bind:checked={focusActiveLayer} onchange={() => { requestDraw(); saveViewState(); }} />
                <span>Focus z-layer</span>
            </label>

            <div class="toggle-grid">
                <button
                    class="toggle-pill"
                    class:active={largePreviewOpen}
                    onclick={() => { largePreviewOpen = !largePreviewOpen; saveViewState(); }}
                    aria-pressed={largePreviewOpen}
                >
                    Preview
                </button>
                <button
                    class="toggle-pill"
                    class:active={textOutputOpen}
                    onclick={() => { textOutputOpen = !textOutputOpen; saveViewState(); }}
                    aria-pressed={textOutputOpen}
                >
                    Text
                </button>
                <button
                    class="toggle-pill"
                    class:active={canvasLabelsOpen}
                    onclick={() => { canvasLabelsOpen = !canvasLabelsOpen; requestDraw(); saveViewState(); }}
                    aria-pressed={canvasLabelsOpen}
                >
                    Labels
                </button>
            </div>

            <div class="selection-nav">
                <button class="layer-step-btn" onclick={() => selectRelativePoint(-1)} disabled={navigationPoints.length === 0} title="Previous image">←</button>
                <span class="selection-position">{selectedNavigationLabel}</span>
                <button class="layer-step-btn" onclick={() => selectRelativePoint(1)} disabled={navigationPoints.length === 0} title="Next image">→</button>
            </div>

            <div class="area-selection-controls" aria-live="polite">
                <button
                    class="toggle-pill area-select-toggle"
                    class:active={areaSelectionMode}
                    onclick={toggleAreaSelection}
                    aria-pressed={areaSelectionMode}
                    aria-label={areaSelectionMode ? 'Exit embedding map area selection' : 'Select an area of the embedding map'}
                >
                    {areaSelectionMode ? 'Selecting area' : 'Select area'}
                </button>
                <span class="area-selection-status">
                    {areaSelectedCount} {areaSelectedCount === 1 ? 'image' : 'images'} selected
                </span>
                {#if areaSelectionMode}
                    <span class="area-selection-hint">Drag on the map, or use arrows and Space.</span>
                {/if}
                {#if areaSelectedCount > 0}
                    <button
                        class="settings-link-btn"
                        onclick={createCollectionFromAreaSelection}
                        disabled={creatingAreaCollection}
                        aria-label={`Create collection from ${areaSelectedCount} selected ${areaSelectedCount === 1 ? 'image' : 'images'}`}
                    >
                        {creatingAreaCollection ? 'Creating…' : 'Create collection'}
                    </button>
                    <button class="settings-link-btn secondary-link" onclick={clearAreaSelection}>Clear selection</button>
                {/if}
            </div>
        </div>

        {#if isLocalProvider(selectedProvider) && !selectedModelAvailable}
            {#if configOpen}
                <div class="panel-section">
                    {#if downloading}
                        <div class="download-progress">
                            <div class="progress-text">
                                {#if downloadProgress.status === 'paused'}
                                    Paused: {formatBytes(downloadProgress.downloaded)} / {formatBytes(downloadProgress.total)}
                                {:else if downloadProgress.total > 0}
                                    Downloading: {formatBytes(downloadProgress.downloaded)} / {formatBytes(downloadProgress.total)}
                                    ({Math.round((downloadProgress.downloaded / downloadProgress.total) * 100)}%)
                                {:else}
                                    Downloading...
                                {/if}
                            </div>
                            <div class="progress-bar-track">
                                <div
                                    class="progress-bar-fill"
                                    style="width: {downloadProgress.total > 0 ? (downloadProgress.downloaded / downloadProgress.total) * 100 : 0}%"
                                ></div>
                            </div>
                            {#if downloadSpeed}
                                <div class="progress-speed">{downloadSpeed}</div>
                            {/if}
                            <div class="download-actions">
                                {#if downloadProgress.status === 'paused'}
                                    <button class="small-btn" onclick={handleResumeDownload} disabled={!downloadJobId}>Resume</button>
                                {:else}
                                    <button class="small-btn" onclick={handlePauseDownload} disabled={!downloadJobId}>Pause</button>
                                {/if}
                                <button class="small-btn danger" onclick={handleCancelDownload} disabled={!downloadJobId}>Cancel</button>
                            </div>
                        </div>
                    {:else}
                        <button class="action-btn" onclick={handleDownload}>
                            {#if downloadProgress.status === 'failed' || downloadProgress.status === 'cancelled'}
                                Resume Download
                            {:else}
                                {selectedModel.downloadLabel ?? 'Download Model'}
                            {/if}
                        </button>
                        {#if downloadProgress.status === 'failed' && downloadProgress.error}
                            <div class="download-error">{downloadProgress.error}</div>
                        {/if}
                    {/if}
                    <div class="manual-download">
                        <div class="section-header" style="margin-top: 10px">MANUAL DOWNLOAD</div>
                        {#if selectedDownloadInfo}
                            <div class="manual-path">{selectedDownloadInfo.model_path}</div>
                            <div class="download-provenance">
                                <div class="provenance-row">
                                    <span>License</span>
                                    <span class="provenance-value">{selectedDownloadInfo.spdx_license}</span>
                                </div>
                                <div class="provenance-row">
                                    <span>Size</span>
                                    <span class="provenance-value">{formatBytes(selectedDownloadInfo.expected_size_bytes)}</span>
                                </div>
                                <div class="provenance-row">
                                    <span>Source</span>
                                    <a class="provenance-link" href={selectedDownloadInfo.source_repo} target="_blank" rel="noreferrer">
                                        {selectedDownloadInfo.source_repo}
                                    </a>
                                </div>
                                <div class="provenance-row">
                                    <span>Model card</span>
                                    <a class="provenance-link" href={selectedDownloadInfo.model_card_url} target="_blank" rel="noreferrer">
                                        {selectedDownloadInfo.model_card_url}
                                    </a>
                                </div>
                                <div class="provenance-hash">{selectedDownloadInfo.expected_sha256}</div>
                            </div>
                            <pre class="manual-cmd">{selectedDownloadInfo.curl_command}</pre>
                        {:else}
                            <pre class="manual-cmd">{modelNameForProvider(selectedProvider)}</pre>
                        {/if}
                    </div>
                </div>
            {/if}
        {:else}
            <div class="panel-section">
                {#if configOpen}
                    <div class="stat-row">
                        <span class="stat-label">Images</span>
                        <span class="stat-value">{totalImages}</span>
                    </div>
                    <div class="stat-row">
                        <span class="stat-label">Embeddings</span>
                        <span class="stat-value">{currentEmbeddingCount}</span>
                    </div>
                    <div class="stat-row">
                        <span class="stat-label">Need embeddings</span>
                        <span class="stat-value">{missingEmbeddingCount}</span>
                    </div>
                    {#if isLocalProvider(selectedProvider)}
                        <div class="stat-row">
                            <span class="stat-label">Model</span>
                            <span class="stat-value">{selectedModel.dims}</span>
                        </div>
                    {/if}
                {/if}
                <div class="generation-actions">
                    <button
                        class="action-btn"
                        onclick={() => startGeneration('missing')}
                        disabled={generating || !selectedModelAvailable || missingEmbeddingCount === 0}
                        title={selectedModelAvailable ? '' : providerStatusLabel(selectedProvider)}
                    >
                        {missingEmbeddingCount === 0 ? 'Up to date' : `Generate missing (${missingEmbeddingCount})`}
                    </button>
                    {#if configOpen}
                        <button
                            class="action-btn secondary"
                            onclick={() => startGeneration('all')}
                            disabled={generating || !selectedModelAvailable || totalImages === 0}
                            title={selectedModelAvailable ? '' : providerStatusLabel(selectedProvider)}
                        >
                            Regenerate all ({totalImages})
                        </button>
                    {/if}
                </div>
            </div>
        {/if}

        {#if activeGeneration}
            <div class="panel-section generation-status" aria-live="polite">
                <div class="section-header">{generationModelLabel(activeGeneration.model).toUpperCase()} GENERATION</div>
                {#if activeGeneration.status === 'running' || activeGeneration.status === 'cancelling'}
                    <div class="progress-text">{activeGeneration.current}/{activeGeneration.total} images</div>
                    <div
                        class="progress-bar-track"
                        role="progressbar"
                        aria-label="{generationModelLabel(activeGeneration.model)} embedding progress"
                        aria-valuemin="0"
                        aria-valuenow={activeGeneration.current}
                        aria-valuemax={Math.max(1, activeGeneration.total)}
                    >
                        <div
                            class="progress-bar-fill"
                            style="width: {activeGeneration.total > 0 ? (activeGeneration.current / activeGeneration.total) * 100 : 0}%"
                        ></div>
                    </div>
                    <button
                        class="action-btn secondary"
                        onclick={cancelGeneration}
                        disabled={!activeGeneration.job_id || activeGeneration.status === 'cancelling'}
                        aria-label={!activeGeneration.job_id
                            ? 'Starting embedding generation'
                            : activeGeneration.status === 'cancelling'
                                ? 'Cancelling embedding generation'
                                : 'Cancel embedding generation'}
                    >
                        {!activeGeneration.job_id ? 'Starting…' : activeGeneration.status === 'cancelling' ? 'Cancelling…' : 'Cancel'}
                    </button>
                {:else if activeGeneration.status === 'completed'}
                    <div class="generation-result">Generation completed: {activeGeneration.current}/{activeGeneration.total}</div>
                {:else if activeGeneration.status === 'cancelled'}
                    <div class="generation-result">Generation cancelled at {activeGeneration.current}/{activeGeneration.total}</div>
                {:else}
                    <div class="generation-result error">Generation failed{activeGeneration.error ? `: ${activeGeneration.error}` : ''}</div>
                {/if}
            </div>
        {/if}

        {#if failedThumbnailIds.size > 0 || staleEmbeddingCount > 0}
            <div class="panel-section warning-section">
                {#if failedThumbnailIds.size > 0}
                    <div class="warning-row">
                        <span class="warning-icon">&#9888;</span>
                        <span>{failedThumbnailIds.size} missing thumbnails</span>
                    </div>
                    <button class="action-btn warning" onclick={handleRegenerateThumbnails} disabled={regeneratingThumbs}>
                        {regeneratingThumbs ? 'Regenerating...' : 'Regenerate Thumbnails'}
                    </button>
                {/if}
                {#if staleEmbeddingCount > 0}
                    <div class="warning-row">
                        <span class="warning-icon">&#9888;</span>
                        <span>{staleEmbeddingCount} stale embeddings</span>
                    </div>
                    <div class="warning-hint">Images deleted but embeddings remain. Regenerate embeddings to clean up.</div>
                {/if}
            </div>
        {/if}

        {#if clusters.length > 0}
            <div class="panel-section">
                <div class="section-header">CLUSTERS</div>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div class="cluster-item all" class:active={highlightedCluster === null} onclick={() => { highlightedCluster = null; activeZLayerKey = null; fitView(); requestDraw(); saveViewState(); }}>
                    <span class="cluster-dot" style="background: var(--text-secondary)"></span>
                    All Images
                    <span class="cluster-count">({points.length})</span>
                </div>
                {#each clusters as cluster}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div class="cluster-item" class:active={highlightedCluster === cluster.id} onclick={() => focusCluster(cluster.id)}>
                        <div class="cluster-info-row">
                            <span class="cluster-dot" style="background: {cluster.color}"></span>
                            <span class="cluster-name">{cluster.label}</span>
                            <span class="cluster-count">({cluster.count})</span>
                        </div>
                        {#if cluster.previewPaths.length > 0}
                            <div class="cluster-previews">
                                {#each cluster.previewPaths.slice(0, 4) as preview}
                                    <img src={convertFileSrc(preview)} class="cluster-thumb" alt="" />
                                {/each}
                            </div>
                        {/if}
                    </div>
                {/each}
            </div>
        {/if}

        {#if selectedPoint}
            <div class="panel-section">
                <div class="section-header">SELECTED</div>
                {#if imageMap.get(selectedPoint.id)}
                    {@const img = imageMap.get(selectedPoint.id)!}
                    {@const previewPath = safeAssetPreviewPath(img)}
                    <div class="selected-preview">
                        {#if previewPath}
                            <img
                                src={convertFileSrc(previewPath)}
                                alt=""
                                class="preview-img"
                            />
                        {:else}
                            <div class="preview-img preview-unavailable">Preview unavailable</div>
                        {/if}
                        <div class="preview-name">{img.path.split('/').pop()}</div>
                        <div class="preview-dims">{img.image.width} x {img.image.height}</div>
                    </div>
                    <button class="action-btn small" onclick={handleFocusInGrid}>
                        Open in Loupe
                    </button>
                {/if}
            </div>
            <div class="panel-section">
                <div class="section-header">NEAREST</div>
                {#if neighborsLoading}
                    <div class="neighbor-status" role="status" aria-live="polite">Finding similar images...</div>
                {:else if neighborsError}
                    <div class="neighbor-status error" role="status" aria-live="polite">Could not load similar images</div>
                {:else if nearestNeighbors.length === 0}
                    <div class="neighbor-status" role="status" aria-live="polite">No similar images in this scope</div>
                {:else}
                    <div class="sr-only" role="status" aria-live="polite">
                        {nearestNeighbors.length} similar {nearestNeighbors.length === 1 ? 'image' : 'images'} found
                    </div>
                    <div class="neighbor-list">
                        {#each nearestNeighbors as neighbor (neighbor.image.image.id)}
                            {@const previewPath = safeAssetPreviewPath(neighbor.image)}
                            {@const projectedPoint = points.find(point => point.id === neighbor.image.image.id)}
                            {@const filename = neighbor.image.path.split('/').pop()}
                            {@const score = `${(neighbor.score * 100).toFixed(1)}%`}
                            <button
                                class="neighbor-row"
                                onclick={() => openNeighbor(neighbor)}
                                aria-label={projectedPoint
                                    ? `Select ${filename}, similarity ${score}`
                                    : `Open ${filename} in Loupe, similarity ${score}`}
                            >
                                {#if previewPath}
                                    <img src={convertFileSrc(previewPath)} alt="" class="neighbor-thumb" />
                                {:else}
                                    <span class="neighbor-thumb unavailable" aria-hidden="true"></span>
                                {/if}
                                <span class="neighbor-name">{filename}</span>
                                <span class="neighbor-score">{score}</span>
                            </button>
                        {/each}
                    </div>
                {/if}
            </div>
        {/if}
    </div>
    {/if}

    <div class="right-panel" use:handleResize>
        {#if points.length === 0}
            <div class="empty-state">
                {#if projecting}
                    <div class="empty-icon">&#8987;</div>
                    <div class="empty-title">Projecting...</div>
                {:else if isLocalProvider(selectedProvider) && !selectedModelAvailable}
                    <div class="empty-icon">&#9881;</div>
                    <div class="empty-title">Model Required</div>
                    <div class="empty-text">Download {selectedModel.label} to generate embeddings</div>
                {:else if isRemoteProvider(selectedProvider) && !selectedModelAvailable}
                    <div class="empty-icon">&#9881;</div>
                    <div class="empty-title">{selectedProvider === 'ollama' ? 'Ollama Offline' : 'API Key Required'}</div>
                    <div class="empty-text">{selectedProvider === 'ollama' ? 'Start Ollama before generating embeddings' : `Set an API key before generating ${selectedModel.shortLabel} embeddings`}</div>
                {:else if currentEmbeddingCount === 0}
                    <div class="empty-icon">&#9673;</div>
                    <div class="empty-title">No Embeddings Yet</div>
                    <div class="empty-text">Generate embeddings for your images to visualize them in 2D space</div>
                {:else}
                    <div class="empty-icon">&#8987;</div>
                    <div class="empty-title">Loading...</div>
                {/if}
            </div>
        {/if}
        <canvas
            bind:this={canvas}
            width={canvasWidth}
            height={canvasHeight}
            onwheel={handleWheel}
            onmousedown={handleMouseDown}
            onmousemove={handleMouseMove}
            onmouseup={handleMouseUp}
            ondblclick={handleCanvasDblClick}
            onmouseleave={cancelAreaSelectionDrag}
            class:hidden={points.length === 0}
            style:cursor={areaSelectionMode ? 'crosshair' : 'grab'}
        ></canvas>

        {#if (largePreviewOpen && selectedImage) || textOutputOpen}
            <div class="embed-inspector">
                {#if largePreviewOpen && selectedImage}
                    {@const previewPath = safeAssetPreviewPath(selectedImage)}
                    <div class="inspector-section-block preview-block">
                        <div class="inspector-header-row">
                            <span class="section-header">PREVIEW</span>
                            <button class="inspector-close" onclick={() => { largePreviewOpen = false; saveViewState(); }} title="Close preview">×</button>
                        </div>
                        <div class="large-preview-frame">
                            {#if previewPath}
                                <img
                                    src={convertFileSrc(previewPath)}
                                    alt=""
                                    class="large-preview-img"
                                />
                            {:else}
                                <div class="large-preview-img preview-unavailable">Preview unavailable</div>
                            {/if}
                        </div>
                        <div class="preview-meta-grid">
                            <span>{selectedFilename}</span>
                            <span>{selectedImage.image.width} x {selectedImage.image.height}</span>
                            <span>{displaySourceLabel(selectedImage.source_label)}</span>
                            <span>{formatRating(selectedImage.selection?.star_rating ?? 0)}</span>
                            <span>{formatDecision(selectedImage.selection?.decision)}</span>
                            <span>{activeZLayer?.label ?? 'All Images'}</span>
                        </div>
                        {#if selectedPrompt}
                            <div class="prompt-snippet">{selectedPrompt}</div>
                        {/if}
                    </div>
                {/if}

                {#if textOutputOpen}
                    <div class="inspector-section-block text-output-block">
                        <div class="inspector-header-row">
                            <span class="section-header">TEXT OUTPUT</span>
                            <div class="inspector-actions">
                                <button class="inspector-copy" onclick={copyTextOutput}>Copy</button>
                                <button class="inspector-close" onclick={() => { textOutputOpen = false; saveViewState(); }} title="Close text output">×</button>
                            </div>
                        </div>
                        <pre class="text-output">{textOutput}</pre>
                    </div>
                {/if}
            </div>
        {/if}
    </div>
</div>

<style>
    .embedding-explorer {
        grid-area: main;
        display: flex;
        height: 100%;
        overflow: hidden;
    }
    .embedding-explorer:focus {
        outline: none;
    }
    .embedding-explorer.zen .right-panel {
        width: 100%;
    }

    .left-panel {
        width: 240px;
        min-width: 240px;
        background: var(--surface);
        border-right: 1px solid var(--border);
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        gap: 0;
    }

    .panel-section {
        padding: 12px;
        border-bottom: 1px solid var(--border);
    }

    .warning-section {
        background: color-mix(in srgb, var(--orange) 8%, transparent);
    }

    .warning-row {
        display: flex;
        align-items: center;
        gap: 6px;
        font-size: 11px;
        color: var(--orange);
    }

    .warning-icon {
        font-size: 12px;
    }

    .warning-hint {
        font-size: 9px;
        color: var(--text-secondary);
        margin-top: 4px;
    }

    .action-btn.warning {
        background: color-mix(in srgb, var(--orange) 15%, transparent);
        color: var(--orange);
    }

    .action-btn.warning:hover:not(:disabled) {
        background: color-mix(in srgb, var(--orange) 25%, transparent);
        border-color: var(--orange);
    }

    .section-header {
        font-size: 10px;
        font-weight: 700;
        color: var(--text-secondary);
        letter-spacing: 0.1em;
        margin-bottom: 8px;
    }

    .model-summary {
        display: grid;
        gap: 4px;
    }

    .model-summary-name {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 11px;
        font-weight: 700;
        color: var(--text);
    }

    .model-summary-count {
        font-size: 10px;
        color: var(--text-secondary);
    }

    .provider-config-row {
        display: grid;
        gap: 4px;
        margin-top: var(--spacing);
        font-size: 10px;
        color: var(--text-secondary);
    }

    .provider-config-meta {
        display: flex;
        gap: var(--spacing);
        margin-top: 4px;
        color: var(--text-secondary);
        font-size: 9px;
    }

    .provider-config-meta .ready {
        color: var(--green);
    }

    .stat-row {
        display: flex;
        justify-content: space-between;
        font-size: 11px;
        padding: 2px 0;
    }

    .stat-label {
        color: var(--text-secondary);
    }

    .stat-value {
        color: var(--text);
        font-weight: 500;
    }

    .action-btn {
        width: 100%;
        margin-top: 8px;
        background: color-mix(in srgb, var(--blue) 15%, transparent);
        color: var(--blue);
        border: 1px solid var(--border);
        font-family: var(--font);
        font-size: 11px;
        padding: 6px 12px;
        border-radius: var(--radius);
        cursor: pointer;
        transition: all 0.15s;
    }

    .action-btn:hover:not(:disabled) {
        background: color-mix(in srgb, var(--blue) 25%, transparent);
        border-color: var(--blue);
    }

    .action-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .action-btn.small {
        font-size: 10px;
        padding: 4px 8px;
    }

    .generation-actions {
        display: grid;
        gap: var(--spacing);
    }

    .action-btn.secondary {
        background: var(--surface);
        color: var(--text-secondary);
    }

    .action-btn.secondary:hover:not(:disabled) {
        color: var(--text);
        border-color: var(--text-secondary);
    }

    .generation-status {
        display: grid;
        gap: var(--spacing);
    }

    .generation-status .action-btn {
        margin-top: 0;
    }

    .generation-result {
        color: var(--text);
        font-size: 10px;
    }

    .generation-result.error {
        color: var(--red);
    }

    .visual-embed-section {
        background: color-mix(in srgb, var(--surface) 88%, var(--bg));
    }

    .mode-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 4px;
        margin-bottom: 10px;
    }

    .mode-btn,
    .toggle-pill,
    .layer-step-btn,
    .inspector-copy,
    .inspector-close {
        background: var(--bg);
        color: var(--text-secondary);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        font-family: var(--font);
        cursor: pointer;
        transition: border-color 0.15s, color 0.15s, background 0.15s;
    }

    .mode-btn {
        min-height: 28px;
        font-size: 10px;
        padding: 5px 6px;
    }

    .mode-btn:hover,
    .toggle-pill:hover,
    .layer-step-btn:hover:not(:disabled),
    .inspector-copy:hover,
    .inspector-close:hover {
        border-color: var(--blue);
        color: var(--text);
    }

    .mode-btn.active,
    .toggle-pill.active {
        background: color-mix(in srgb, var(--blue) 18%, var(--bg));
        border-color: var(--blue);
        color: var(--blue);
    }

    .control-label {
        display: block;
        margin: 8px 0 4px;
        font-size: 9px;
        font-weight: 700;
        letter-spacing: 0.08em;
        color: var(--text-secondary);
    }

    .space-control-panel {
        display: grid;
        gap: 7px;
        margin-top: 6px;
        padding: 8px;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
    }

    .range-control {
        display: grid;
        gap: 3px;
        min-width: 0;
        color: var(--text-secondary);
        font-size: 9px;
    }

    .range-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        min-width: 0;
    }

    .range-value {
        flex-shrink: 0;
        color: var(--text);
    }

    .range-control input[type="range"] {
        width: 100%;
        height: 16px;
        margin: 0;
        accent-color: var(--blue);
        cursor: pointer;
    }

    .layer-row {
        display: grid;
        grid-template-columns: 28px minmax(0, 1fr) 28px;
        gap: 4px;
        margin-top: 6px;
    }

    .layer-select {
        min-width: 0;
    }

    .layer-step-btn {
        width: 28px;
        height: 28px;
        padding: 0;
        font-size: 13px;
        line-height: 1;
    }

    .layer-step-btn:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }

    .layer-meta {
        display: flex;
        align-items: center;
        gap: 6px;
        margin-top: 6px;
        min-height: 18px;
        font-size: 10px;
        color: var(--text);
    }

    .toggle-row {
        display: flex;
        align-items: center;
        gap: 6px;
        margin-top: 8px;
        font-size: 10px;
        color: var(--text-secondary);
        cursor: pointer;
    }

    .toggle-row input {
        accent-color: var(--blue);
    }

    .toggle-grid {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: 4px;
        margin-top: 8px;
    }

    .toggle-pill {
        min-height: 26px;
        padding: 4px 5px;
        font-size: 10px;
    }

    .selection-nav {
        display: grid;
        grid-template-columns: 28px minmax(0, 1fr) 28px;
        align-items: center;
        gap: 4px;
        margin-top: 8px;
    }

    .selection-position {
        color: var(--text-secondary);
        font-size: 10px;
        text-align: center;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .area-selection-controls {
        display: grid;
        gap: var(--spacing);
        margin-top: var(--spacing);
    }

    .area-select-toggle {
        width: 100%;
    }

    .area-selection-status {
        color: var(--text-secondary);
        font-size: 10px;
        text-align: center;
    }

    .area-selection-hint {
        color: var(--text-secondary);
        font-size: 9px;
        line-height: 1.4;
        text-align: center;
    }

    .cluster-item {
        display: flex;
        flex-direction: column;
        gap: 4px;
        font-size: 11px;
        padding: 5px 4px;
        color: var(--text);
        cursor: pointer;
        border-radius: var(--radius);
        transition: background 0.15s;
    }

    .cluster-item:hover {
        background: color-mix(in srgb, var(--text) 5%, transparent);
    }

    .cluster-item.active {
        background: color-mix(in srgb, var(--blue) 10%, transparent);
    }

    .cluster-item.all {
        flex-direction: row;
        align-items: center;
        gap: 6px;
        margin-bottom: 4px;
        font-weight: 500;
    }

    .cluster-info-row {
        display: flex;
        align-items: center;
        gap: 6px;
    }

    .cluster-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        flex-shrink: 0;
    }

    .cluster-name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .cluster-count {
        color: var(--text-secondary);
        margin-left: auto;
        font-size: 10px;
        flex-shrink: 0;
    }

    .cluster-previews {
        display: flex;
        gap: 2px;
        margin-left: 14px;
    }

    .cluster-thumb {
        width: 20px;
        height: 20px;
        object-fit: cover;
        border-radius: 2px;
        opacity: 0.85;
    }

    .selected-preview {
        text-align: center;
    }

    .preview-img {
        max-width: 100%;
        max-height: 140px;
        border-radius: var(--radius);
        margin-bottom: 6px;
    }

    .preview-unavailable {
        display: grid;
        place-items: center;
        min-height: 72px;
        color: var(--text-secondary);
        background: var(--bg);
        border: 1px solid var(--border);
        font-size: 10px;
        line-height: 1.2;
        text-align: center;
        padding: 6px;
    }

    .preview-name {
        font-size: 10px;
        color: var(--text);
        word-break: break-all;
    }

    .preview-dims {
        font-size: 10px;
        color: var(--text-secondary);
    }

    .neighbor-list {
        display: grid;
        gap: var(--spacing);
    }

    .neighbor-row {
        display: grid;
        grid-template-columns: 36px minmax(0, 1fr) auto;
        align-items: center;
        gap: var(--spacing);
        width: 100%;
        padding: var(--spacing);
        background: var(--bg);
        color: var(--text);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        font-family: var(--font);
        cursor: pointer;
        text-align: left;
    }

    .neighbor-row:hover,
    .neighbor-row:focus-visible {
        border-color: var(--blue);
    }

    .neighbor-thumb {
        width: 36px;
        height: 36px;
        object-fit: cover;
        border-radius: var(--radius);
        background: var(--surface);
    }

    .neighbor-thumb.unavailable {
        border: 1px solid var(--border);
    }

    .neighbor-name {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 10px;
    }

    .neighbor-score {
        color: var(--blue);
        font-size: 10px;
        font-variant-numeric: tabular-nums;
    }

    .neighbor-status {
        color: var(--text-secondary);
        font-size: 10px;
        line-height: 1.4;
    }

    .neighbor-status.error {
        color: var(--red);
    }

    .sr-only {
        position: absolute;
        width: 1px;
        height: 1px;
        padding: 0;
        margin: -1px;
        overflow: hidden;
        clip: rect(0, 0, 0, 0);
        white-space: nowrap;
        border: 0;
    }

    .right-panel {
        flex: 1;
        position: relative;
        background: var(--bg);
        overflow: hidden;
    }

    .embed-inspector {
        position: absolute;
        top: 12px;
        right: 12px;
        bottom: 12px;
        width: min(360px, 38vw);
        z-index: 20;
        display: flex;
        flex-direction: column;
        gap: 8px;
        pointer-events: auto;
    }

    .inspector-section-block {
        min-height: 0;
        background: color-mix(in srgb, var(--surface) 96%, var(--bg));
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 14px 40px color-mix(in srgb, var(--bg) 70%, transparent);
    }

    .preview-block {
        display: flex;
        flex-direction: column;
        gap: 8px;
        padding: 10px;
        max-height: 62%;
    }

    .text-output-block {
        display: flex;
        min-height: 120px;
        flex: 1;
        flex-direction: column;
        padding: 10px;
    }

    .inspector-header-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        min-height: 22px;
    }

    .inspector-header-row .section-header {
        margin-bottom: 0;
    }

    .inspector-actions {
        display: flex;
        align-items: center;
        gap: 4px;
    }

    .inspector-copy,
    .inspector-close {
        height: 22px;
        padding: 0 7px;
        font-size: 10px;
    }

    .inspector-close {
        width: 24px;
        padding: 0;
    }

    .large-preview-frame {
        min-height: 180px;
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        overflow: hidden;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
    }

    .large-preview-img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
    }

    .large-preview-img.preview-unavailable {
        width: 100%;
        height: 100%;
        min-height: 180px;
    }

    .preview-meta-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 3px 8px;
        font-size: 10px;
        color: var(--text-secondary);
    }

    .preview-meta-grid span {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .preview-meta-grid span:first-child {
        grid-column: 1 / -1;
        color: var(--text);
    }

    .prompt-snippet {
        max-height: 76px;
        overflow: auto;
        border-top: 1px solid var(--border);
        padding-top: 7px;
        color: var(--text-secondary);
        font-size: 10px;
        line-height: 1.45;
    }

    .text-output {
        min-height: 0;
        flex: 1;
        margin: 8px 0 0;
        overflow: auto;
        white-space: pre-wrap;
        word-break: break-word;
        color: var(--text);
        font-family: var(--font);
        font-size: 10px;
        line-height: 1.5;
    }

    canvas {
        display: block;
        width: 100%;
        height: 100%;
    }

    canvas.hidden {
        display: none;
    }

    .empty-state {
        position: absolute;
        inset: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
        color: var(--text-secondary);
    }

    .empty-icon {
        font-size: 32px;
        opacity: 0.5;
    }

    .empty-title {
        font-size: 14px;
        font-weight: 700;
    }

    .empty-text {
        font-size: 11px;
        opacity: 0.6;
        max-width: 280px;
        text-align: center;
    }

    .download-progress {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }

    .progress-text {
        font-size: 10px;
        color: var(--text);
    }

    .progress-bar-track {
        width: 100%;
        height: 6px;
        background: var(--border);
        border-radius: 3px;
        overflow: hidden;
    }

    .progress-bar-fill {
        height: 100%;
        background: var(--blue);
        border-radius: 3px;
        transition: width 0.2s ease;
    }

    .progress-speed {
        font-size: 9px;
        color: var(--text-secondary);
        text-align: right;
    }

    .download-actions {
        display: flex;
        gap: 6px;
        margin-top: 4px;
    }

    .small-btn {
        flex: 1;
        background: color-mix(in srgb, var(--surface) 80%, var(--blue));
        color: var(--text);
        border: 1px solid var(--border);
        font-family: var(--font);
        font-size: 10px;
        padding: 4px 6px;
        border-radius: var(--radius);
        cursor: pointer;
    }

    .small-btn:hover:not(:disabled) {
        border-color: var(--blue);
    }

    .small-btn.danger:hover:not(:disabled) {
        border-color: var(--red);
        color: var(--red);
    }

    .small-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .download-error {
        margin-top: 6px;
        color: var(--red);
        font-size: 10px;
        line-height: 1.4;
        word-break: break-word;
    }

    .manual-download {
        margin-top: 4px;
    }

    .manual-path {
        font-family: 'JetBrains Mono', monospace;
        font-size: 9px;
        color: var(--text);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 5px 6px;
        margin-bottom: 6px;
        word-break: break-all;
        line-height: 1.4;
    }

    .download-provenance {
        display: grid;
        gap: 4px;
        margin-bottom: 6px;
        padding: 6px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text-secondary);
        font-size: 9px;
        line-height: 1.4;
    }

    .provenance-row {
        display: grid;
        grid-template-columns: 56px minmax(0, 1fr);
        gap: 6px;
        min-width: 0;
    }

    .provenance-value,
    .provenance-link,
    .provenance-hash {
        min-width: 0;
        color: var(--text);
        overflow-wrap: anywhere;
    }

    .provenance-link {
        text-decoration: none;
    }

    .provenance-link:hover {
        color: var(--blue);
    }

    .provenance-hash {
        font-family: 'JetBrains Mono', monospace;
        color: var(--green);
    }

    .manual-cmd {
        font-family: 'JetBrains Mono', monospace;
        font-size: 9px;
        color: var(--text-secondary);
        background: color-mix(in srgb, var(--bg) 70%, var(--surface));
        padding: 6px;
        border-radius: var(--radius);
        overflow-x: auto;
        white-space: pre-wrap;
        word-break: break-all;
        line-height: 1.4;
        margin: 0;
    }

    .section-header-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 8px;
    }

    .section-header-row .section-header {
        margin-bottom: 0;
    }

    .gear-btn {
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 14px;
        padding: 0 2px;
        line-height: 1;
        transition: color 0.15s;
    }

    .gear-btn:hover {
        color: var(--text);
    }

    .provider-select {
        width: 100%;
        background: var(--bg);
        color: var(--text);
        border: 1px solid var(--border);
        font-family: var(--font);
        font-size: 11px;
        padding: 4px 6px;
        border-radius: var(--radius);
        cursor: pointer;
    }

    .provider-select:focus {
        outline: none;
        border-color: var(--blue);
    }

    .config-section {
        background: color-mix(in srgb, var(--bg) 55%, var(--surface));
    }

    .provider-field {
        display: grid;
        gap: 4px;
        margin-bottom: var(--spacing);
        font-size: 10px;
        color: var(--text-secondary);
    }

    .provider-field input {
        box-sizing: border-box;
        width: 100%;
        background: var(--bg);
        color: var(--text);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 6px 8px;
        font: 11px var(--font);
    }

    .settings-link-btn {
        background: none;
        border: 1px solid var(--blue);
        border-radius: var(--radius);
        padding: 4px 12px;
        font-size: 12px;
        font-family: var(--font);
        color: var(--blue);
        cursor: pointer;
    }

    .settings-link-btn:hover {
        background: color-mix(in srgb, var(--blue) 10%, transparent);
    }

    .settings-link-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .secondary-link {
        margin-left: 4px;
        color: var(--text-secondary);
        border-color: var(--border);
    }

    .provider-config-message {
        margin: var(--spacing) 0 0;
        font-size: 10px;
    }

    .provider-config-message.ready {
        color: var(--green);
    }

    .provider-config-message.error {
        color: var(--red);
    }

    @media (max-width: 920px) {
        .embed-inspector {
            left: 12px;
            width: auto;
        }
    }
</style>
