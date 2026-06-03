import { describe, expect, it } from 'vitest';
import {
    buildSearchPresetLists,
    SEARCH_AUTOSUGGEST_CANDIDATES,
    type CountLookup,
} from './search-presets';
import type { SmartCollection } from './api';

function smart(overrides: Partial<SmartCollection>): SmartCollection {
    return {
        id: 'sc-1',
        name: 'Preset',
        description: null,
        collection_type: 'smart',
        filter_json: '{"type":"rule","field":"rating","op":"eq","value":5}',
        nl_query: null,
        is_preset: false,
        sort_order: 0,
        created_at: '2026-01-01T00:00:00Z',
        image_count: 1,
        ...overrides,
    };
}

function countLookup(counts: Record<string, number>): CountLookup {
    return async (filterJson: string) => counts[filterJson] ?? 0;
}

describe('buildSearchPresetLists', () => {
    it('separates user-saved presets from autosuggested presets and filters zero-count entries', async () => {
        const saved = smart({
            id: 'saved-1',
            name: 'Saved Picks',
            filter_json: '{"type":"rule","field":"decision","op":"eq","value":"accept"}',
            is_preset: false,
            image_count: 5,
        });
        const emptySaved = smart({
            id: 'saved-empty',
            name: 'Empty Saved',
            filter_json: '{"type":"rule","field":"rating","op":"eq","value":1}',
            is_preset: false,
            image_count: 0,
        });
        const builtIn = smart({
            id: 'preset-1',
            name: 'Landscape',
            filter_json: '{"type":"rule","field":"orientation","op":"eq","value":"landscape"}',
            is_preset: true,
            image_count: 7,
        });
        const emptyBuiltIn = smart({
            id: 'preset-empty',
            name: 'Portrait',
            filter_json: '{"type":"rule","field":"orientation","op":"eq","value":"portrait"}',
            is_preset: true,
            image_count: 0,
        });

        const result = await buildSearchPresetLists(
            [saved, emptySaved, builtIn, emptyBuiltIn],
            countLookup({}),
        );

        expect(result.saved.map(preset => preset.name)).toEqual(['Saved Picks']);
        expect(result.auto.map(preset => preset.name)).toContain('Landscape');
        expect(result.auto.map(preset => preset.name)).not.toContain('Portrait');
        expect(result.saved.every(preset => preset.kind === 'saved')).toBe(true);
        expect(result.auto.every(preset => preset.kind === 'auto')).toBe(true);
    });

    it('adds the GPT Images autosuggestion only when the database has GPT images', async () => {
        const gptFilter = SEARCH_AUTOSUGGEST_CANDIDATES.find(candidate => candidate.id === 'auto-gpt-images')?.filterJson;
        expect(gptFilter).toBeTruthy();
        expect(JSON.parse(gptFilter!)).toMatchObject({
            type: 'group',
            op: 'or',
            children: expect.arrayContaining([
                expect.objectContaining({ field: 'source_label', value: 'gpt_image_2' }),
                expect.objectContaining({ field: 'search_text', value: 'gpt-image' }),
            ]),
        });

        const absent = await buildSearchPresetLists([], countLookup({}));
        expect(absent.auto.map(preset => preset.id)).not.toContain('auto-gpt-images');

        const present = await buildSearchPresetLists(
            [],
            countLookup({ [gptFilter!]: 3 }),
        );

        expect(present.auto.find(preset => preset.id === 'auto-gpt-images')).toMatchObject({
            name: 'GPT Images',
            imageCount: 3,
            query: 'gpt image',
            kind: 'auto',
        });
    });

    it('reflects fresh counts when rebuilt after imports', async () => {
        const gptFilter = SEARCH_AUTOSUGGEST_CANDIDATES.find(candidate => candidate.id === 'auto-gpt-images')!.filterJson;

        const beforeImport = await buildSearchPresetLists([], countLookup({ [gptFilter]: 0 }));
        const afterImport = await buildSearchPresetLists([], countLookup({ [gptFilter]: 2 }));

        expect(beforeImport.auto.map(preset => preset.id)).not.toContain('auto-gpt-images');
        expect(afterImport.auto.find(preset => preset.id === 'auto-gpt-images')?.imageCount).toBe(2);
    });
});
