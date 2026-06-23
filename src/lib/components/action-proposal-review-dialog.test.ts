import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const source = readFileSync(
    join(process.cwd(), 'src/lib/components/ActionProposalReviewDialog.svelte'),
    'utf8',
);

describe('ActionProposalReviewDialog source contract', () => {
    it('requires native review before applying approved candidates', () => {
        expect(source).toContain('Review Trash proposal');
        expect(source).toContain('Review selection proposal');
        expect(source).toContain('approvedIds.size');
        expect(source).toContain('onapplyproposal(proposal.id, Array.from(approvedIds))');
    });

    it('uses separate labels for Trash and selection proposals', () => {
        expect(source).toContain("proposal?.kind === 'trash_images'");
        expect(source).toContain('Move approved to Trash');
        expect(source).toContain('Select approved');
    });

    it('keeps the confirmation path in component props instead of window events', () => {
        expect(source).toContain('onapplyproposal?: (proposalId: string, approvedImageIds: string[]) => void');
        expect(source).toContain('oncancelreview?: () => void');
        expect(source).not.toContain('dispatchEvent');
    });
});
