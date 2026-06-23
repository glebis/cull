import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { effectiveAgentVisualLevel } from './agent-visual-context';

describe('effective agent visual level', () => {
    it('auto-promotes text curation requests when candidate thumbnails are available', () => {
        expect(effectiveAgentVisualLevel({
            requestedVisualLevel: 'text',
            candidateCount: 5,
            thumbnailCount: 5,
        })).toBe('preview');
    });

    it('keeps text mode when no visual context exists', () => {
        expect(effectiveAgentVisualLevel({
            requestedVisualLevel: 'text',
            candidateCount: 5,
            thumbnailCount: 0,
        })).toBe('text');
    });

    it('does not override explicit visual levels', () => {
        expect(effectiveAgentVisualLevel({
            requestedVisualLevel: 'tiny',
            candidateCount: 5,
            thumbnailCount: 5,
        })).toBe('tiny');
    });

    it('is wired into the Claude agent request path', () => {
        const page = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');

        expect(page).toContain("from '$lib/agent-visual-context'");
        expect(page).toContain('const visualLevel = visualLevelForAgentRequest(rawCandidateImages)');
        expect(page).toContain('imageContextForAgent(rawCandidateImages, visualLevel)');
        expect(page).toContain('visual_level: visualLevel');
    });
});
