import { describe, expect, it } from 'vitest';
import { formatCollectionDate, generateImportCollectionName } from './collection-name';

const now = new Date(2026, 4, 17, 13, 9, 28);

describe('formatCollectionDate', () => {
    it('uses YYYY.MM.DD and 24-hour time', () => {
        expect(formatCollectionDate(now)).toBe('2026.05.17 13:09');
    });
});

describe('generateImportCollectionName', () => {
    it('prioritizes available prompts over generic paths', () => {
        const name = generateImportCollectionName([
            {
                path: '/Users/g/outputs/IMG_0001.png',
                aiPrompt: '/imagine prompt: portrait of a glass astronaut, cinematic lighting, highly detailed --ar 16:9',
            },
        ], { now });

        expect(name).toBe('Portrait of a Glass Astronaut - 2026.05.17 13:09');
    });

    it('uses sidecar generation prompts when image prompts are absent', () => {
        const name = generateImportCollectionName([
            {
                path: '/Users/g/outputs/export_01.png',
                generationPrompt: 'solar punk greenhouse interior, warm morning light',
            },
        ], { now });

        expect(name).toBe('Solar Punk Greenhouse Interior Warm Morning Light - 2026.05.17 13:09');
    });

    it('combines readable folder and filename context', () => {
        const name = generateImportCollectionName([
            { path: '/Users/g/art/2026-05-17/cyberpunk-city/city_gate_001.png' },
            { path: '/Users/g/art/2026-05-17/cyberpunk-city/city_gate_002.png' },
        ], { now });

        expect(name).toBe('Cyberpunk City - City Gate - 2026.05.17 13:09');
    });

    it('uses the first image import time when no explicit date is passed', () => {
        const name = generateImportCollectionName([
            { path: '/Users/g/art/portraits/winter_face_001.png', importedAt: '2026-05-17T14:12:00' },
            { path: '/Users/g/art/portraits/winter_face_002.png', importedAt: '2026-05-17T14:13:00' },
        ]);

        expect(name).toBe('Portraits - Winter Face - 2026.05.17 14:12');
    });

    it('falls back to a formatted import name when no useful context exists', () => {
        const name = generateImportCollectionName([
            { path: '/Users/g/outputs/IMG_0001.png' },
            { path: '/Users/g/outputs/IMG_0002.png' },
        ], { now });

        expect(name).toBe('Import - 2026.05.17 13:09');
    });
});
