import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

describe('embedding download provenance UI', () => {
    it('renders checksum, size, license, source, and model card fields', () => {
        const source = readFileSync(
            fileURLToPath(new URL('./components/EmbeddingExplorer.svelte', import.meta.url)),
            'utf8'
        );

        expect(source).toContain('selectedDownloadInfo.expected_sha256');
        expect(source).toContain('selectedDownloadInfo.expected_size_bytes');
        expect(source).toContain('selectedDownloadInfo.spdx_license');
        expect(source).toContain('selectedDownloadInfo.source_repo');
        expect(source).toContain('selectedDownloadInfo.model_card_url');
    });
});
