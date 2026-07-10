import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(fileURLToPath(new URL('./AiSettings.svelte', import.meta.url)), 'utf8');

describe('AI settings', () => {
    it('orders credentials before local and embedding models', () => {
        const credentials = source.indexOf('Provider Credentials');
        const local = source.indexOf('Local Models');
        const embeddings = source.indexOf('Embedding Models');
        expect(credentials).toBeGreaterThan(-1);
        expect(local).toBeGreaterThan(credentials);
        expect(embeddings).toBeGreaterThan(local);
    });

    it('configures models without exposing library processing actions', () => {
        expect(source).toContain("getAppSetting('yolo_variant')");
        expect(source).toContain("setAppSetting('yolo_variant'");
        expect(source).not.toMatch(/Detect remaining|Describe images|Scan library/i);
    });
});
