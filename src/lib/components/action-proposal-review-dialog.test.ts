import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const source = readFileSync(
    join(process.cwd(), 'src/lib/components/ActionProposalReviewDialog.svelte'),
    'utf8',
);

describe('ActionProposalReviewDialog source contract', () => {
    it('renders visual context for proposal candidates', () => {
        expect(source).toContain("from '@tauri-apps/api/core'");
        expect(source).toContain("from '$lib/view-utils'");
        expect(source).toContain('visibleImages?: ImageWithFile[]');
        expect(source).toContain('currentViewContext');
        expect(source).toContain('parseAgentProposalSourceContext');
        expect(source).toContain('proposalActorLabel');
        expect(source).toContain('sourceContextIsStale');
        expect(source).toContain('Stale view');
        expect(source).toContain('safeAssetPreviewPath');
        expect(source).toContain('class="candidate-preview"');
        expect(source).toContain('filenameForCandidate(candidate.image_id)');
    });
});
