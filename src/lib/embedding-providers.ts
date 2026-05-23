import type { EmbeddingProviderInfo } from './api';
import type { EmbeddingProvider } from './stores';

export type LocalEmbeddingProvider = Extract<EmbeddingProvider, 'clip' | 'dinov2'>;

export type ModelOption = {
    id: EmbeddingProvider;
    label: string;
    shortLabel: string;
    modelName: string;
    dims: string;
    scope: string;
    downloadLabel?: string;
};

export const DEFAULT_MODEL_OPTIONS: ModelOption[] = [
    {
        id: 'clip',
        label: 'CLIP ViT-B/32',
        shortLabel: 'CLIP',
        modelName: 'clip-vit-b32',
        dims: '512d',
        scope: 'local',
        downloadLabel: 'Download CLIP (~350MB)',
    },
    {
        id: 'dinov2',
        label: 'DINOv2 ViT-S/14',
        shortLabel: 'DINOv2',
        modelName: 'dinov2-vits14',
        dims: '384d',
        scope: 'local',
        downloadLabel: 'Download DINOv2 (~87MB)',
    },
    {
        id: 'gemini',
        label: 'Gemini Embedding 2',
        shortLabel: 'Gemini',
        modelName: 'gemini-embedding-2',
        dims: '3072d',
        scope: 'cloud',
    },
    {
        id: 'cohere',
        label: 'Cohere Embed v4 Multimodal',
        shortLabel: 'Cohere',
        modelName: 'cohere:embed-v4.0',
        dims: '1024d',
        scope: 'cloud',
    },
    {
        id: 'openai',
        label: 'OpenAI Text Embedding 3 Large',
        shortLabel: 'OpenAI',
        modelName: 'openai:text-embedding-3-large',
        dims: '3072d',
        scope: 'cloud',
    },
    {
        id: 'ollama',
        label: 'Ollama Text Embeddings',
        shortLabel: 'Ollama',
        modelName: 'ollama:embeddinggemma',
        dims: 'model',
        scope: 'local',
    },
];

const KNOWN_PROVIDER_IDS = new Set<EmbeddingProvider>(['clip', 'dinov2', 'gemini', 'cohere', 'openai', 'ollama']);

function isEmbeddingProvider(id: string): id is EmbeddingProvider {
    return KNOWN_PROVIDER_IDS.has(id as EmbeddingProvider);
}

export function modelOptionsFromProviderInfo(providers: EmbeddingProviderInfo[]): ModelOption[] {
    return providers.flatMap(provider => {
        if (!isEmbeddingProvider(provider.id)) return [];
        return [{
            id: provider.id,
            label: provider.label,
            shortLabel: provider.shortLabel,
            modelName: provider.modelName,
            dims: provider.dimensionsLabel || `${provider.dimensions}d`,
            scope: provider.scope,
            ...(provider.downloadLabel ? { downloadLabel: provider.downloadLabel } : {}),
        }];
    });
}
