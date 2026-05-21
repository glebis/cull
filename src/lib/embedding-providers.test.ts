import { describe, expect, it } from 'vitest';
import { DEFAULT_MODEL_OPTIONS, modelOptionsFromProviderInfo } from './embedding-providers';

describe('embedding provider metadata', () => {
    it('maps backend provider metadata into selector options', () => {
        const options = modelOptionsFromProviderInfo([
            {
                id: 'clip',
                label: 'CLIP ViT-B/32',
                shortLabel: 'CLIP',
                modelName: 'clip-vit-b32',
                dimensions: 512,
                dimensionsLabel: '512d',
                scope: 'local',
                runtime: 'local-onnx',
                status: 'ready',
                available: true,
                downloadable: true,
                downloadLabel: 'Download CLIP (~350MB)',
                apiKeyProvider: null,
            },
            {
                id: 'gemini',
                label: 'Gemini Embedding 2',
                shortLabel: 'Gemini',
                modelName: 'gemini-embedding-2',
                dimensions: 3072,
                dimensionsLabel: '3072d',
                scope: 'cloud',
                runtime: 'cloud-api',
                status: 'key',
                available: false,
                downloadable: false,
                downloadLabel: null,
                apiKeyProvider: 'google',
            },
            {
                id: 'openai',
                label: 'OpenAI Text Embedding 3 Large',
                shortLabel: 'OpenAI',
                modelName: 'openai:text-embedding-3-large',
                dimensions: 3072,
                dimensionsLabel: '3072d',
                scope: 'cloud',
                runtime: 'cloud-api',
                status: 'key',
                available: false,
                downloadable: false,
                downloadLabel: null,
                apiKeyProvider: 'openai',
            },
            {
                id: 'ollama',
                label: 'Ollama Text Embeddings',
                shortLabel: 'Ollama',
                modelName: 'ollama:embeddinggemma',
                dimensions: 0,
                dimensionsLabel: 'model',
                scope: 'local',
                runtime: 'local-api',
                status: 'offline',
                available: false,
                downloadable: false,
                downloadLabel: null,
                apiKeyProvider: null,
            },
            {
                id: 'future-model',
                label: 'Future Model',
                shortLabel: 'Future',
                modelName: 'future-model',
                dimensions: 128,
                dimensionsLabel: '128d',
                scope: 'local',
                runtime: 'local-onnx',
                status: 'unsupported',
                available: false,
                downloadable: false,
                downloadLabel: null,
                apiKeyProvider: null,
            },
        ]);

        expect(options).toEqual([
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
                id: 'gemini',
                label: 'Gemini Embedding 2',
                shortLabel: 'Gemini',
                modelName: 'gemini-embedding-2',
                dims: '3072d',
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
        ]);
    });

    it('keeps a complete static default for initial render', () => {
        expect(DEFAULT_MODEL_OPTIONS.map(option => option.id)).toEqual(['clip', 'dinov2', 'gemini', 'openai', 'ollama']);
    });
});
