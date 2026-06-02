import type { SmartCollection } from './api';

export type SearchPresetKind = 'saved' | 'auto';

export interface SearchPreset {
    id: string;
    name: string;
    filterJson: string;
    query: string;
    imageCount: number;
    kind: SearchPresetKind;
}

export interface SearchPresetLists {
    saved: SearchPreset[];
    auto: SearchPreset[];
}

export type CountLookup = (filterJson: string) => Promise<number>;

interface SearchAutosuggestCandidate {
    id: string;
    name: string;
    query: string;
    filterJson: string;
}

const GPT_IMAGES_FILTER = JSON.stringify({
    type: 'group',
    op: 'or',
    children: [
        {
            type: 'rule',
            field: 'source_label',
            op: 'eq',
            value: 'gpt_image_2',
        },
        {
            type: 'rule',
            field: 'search_text',
            op: 'contains',
            value: 'gpt-image',
        },
        {
            type: 'rule',
            field: 'search_text',
            op: 'contains',
            value: 'chatgpt',
        },
    ],
});

export const SEARCH_AUTOSUGGEST_CANDIDATES: SearchAutosuggestCandidate[] = [
    {
        id: 'auto-gpt-images',
        name: 'GPT Images',
        query: 'gpt image',
        filterJson: GPT_IMAGES_FILTER,
    },
    {
        id: 'auto-midjourney',
        name: 'Midjourney',
        query: 'midjourney',
        filterJson: JSON.stringify({
            type: 'rule',
            field: 'source_label',
            op: 'eq',
            value: 'midjourney',
        }),
    },
    {
        id: 'auto-stable-diffusion',
        name: 'Stable Diffusion',
        query: 'stable diffusion',
        filterJson: JSON.stringify({
            type: 'rule',
            field: 'source_label',
            op: 'eq',
            value: 'stable_diffusion',
        }),
    },
    {
        id: 'auto-dalle',
        name: 'DALL-E',
        query: 'dall-e',
        filterJson: JSON.stringify({
            type: 'rule',
            field: 'source_label',
            op: 'in',
            value: ['dalle_3', 'dalle'],
        }),
    },
    {
        id: 'auto-comfyui',
        name: 'ComfyUI',
        query: 'comfyui',
        filterJson: JSON.stringify({
            type: 'rule',
            field: 'source_label',
            op: 'eq',
            value: 'comfyui',
        }),
    },
];

async function countForCollection(collection: SmartCollection, countLookup: CountLookup): Promise<number> {
    if (!collection.filter_json) return 0;
    if (typeof collection.image_count === 'number') return collection.image_count;
    return countLookup(collection.filter_json);
}

function collectionToPreset(collection: SmartCollection, count: number, kind: SearchPresetKind): SearchPreset | null {
    if (!collection.filter_json || count <= 0) return null;
    return {
        id: collection.id,
        name: collection.name,
        filterJson: collection.filter_json,
        query: collection.nl_query ?? collection.name.toLowerCase(),
        imageCount: count,
        kind,
    };
}

async function candidateToPreset(candidate: SearchAutosuggestCandidate, countLookup: CountLookup): Promise<SearchPreset | null> {
    const count = await countLookup(candidate.filterJson);
    if (count <= 0) return null;
    return {
        id: candidate.id,
        name: candidate.name,
        filterJson: candidate.filterJson,
        query: candidate.query,
        imageCount: count,
        kind: 'auto',
    };
}

function byFilterJson(items: SearchPreset[]): Set<string> {
    return new Set(items.map(item => item.filterJson));
}

export async function buildSearchPresetLists(
    collections: SmartCollection[],
    countLookup: CountLookup,
    options: { savedLimit?: number; autoLimit?: number } = {},
): Promise<SearchPresetLists> {
    const savedLimit = options.savedLimit ?? 6;
    const autoLimit = options.autoLimit ?? 8;

    const savedResults = await Promise.all(
        collections
            .filter(collection => !collection.is_preset)
            .map(async collection => collectionToPreset(collection, await countForCollection(collection, countLookup), 'saved')),
    );
    const saved = savedResults.filter((preset): preset is SearchPreset => preset !== null).slice(0, savedLimit);

    const builtInResults = await Promise.all(
        collections
            .filter(collection => collection.is_preset)
            .map(async collection => collectionToPreset(collection, await countForCollection(collection, countLookup), 'auto')),
    );
    const builtIn = builtInResults.filter((preset): preset is SearchPreset => preset !== null);

    const existingAutoFilters = byFilterJson(builtIn);
    const candidateResults = await Promise.all(
        SEARCH_AUTOSUGGEST_CANDIDATES
            .filter(candidate => !existingAutoFilters.has(candidate.filterJson))
            .map(candidate => candidateToPreset(candidate, countLookup)),
    );
    const candidates = candidateResults.filter((preset): preset is SearchPreset => preset !== null);

    return {
        saved,
        auto: [...candidates, ...builtIn].slice(0, autoLimit),
    };
}
