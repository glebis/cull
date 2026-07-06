import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const pageSource = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');

describe('agent proposal apply flow contract', () => {
    it('moves approved Trash proposals before marking the proposal applied', () => {
        const trashCall = pageSource.indexOf('const trashResult = await trashImagesDetailed(approvedImageIds)');
        const applyCall = pageSource.indexOf('await applyActionProposal(proposalId, approvedImageIds, JSON.stringify(trashResult))');
        expect(trashCall).toBeGreaterThan(-1);
        expect(applyCall).toBeGreaterThan(-1);
        expect(trashCall).toBeLessThan(applyCall);
        expect(pageSource).toContain("trashResult.results.filter(item => item.status === 'trashed')");
    });

    it('intersects selection proposals with currently loaded images before applying', () => {
        expect(pageSource).toContain('const visibleIds = new Set($images.map(item => item.image.id))');
        expect(pageSource).toContain('const visibleApprovedIds = approvedImageIds.filter(id => visibleIds.has(id))');
        expect(pageSource).toContain('selectedIds.set(new Set(visibleApprovedIds))');
        expect(pageSource).toContain('focusedIndex.set(firstIndex)');
        expect(pageSource).toContain('Selection proposal no longer matches this view');
        expect(pageSource).toContain('missing: approvedImageIds.length - visibleApprovedIds.length');
    });

    it('passes image context into the native proposal review dialog', () => {
        expect(pageSource).toContain('<ActionProposalReviewDialog');
        expect(pageSource).toContain('visibleImages={$images}');
    });
});
