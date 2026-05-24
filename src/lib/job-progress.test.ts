import { describe, expect, it } from 'vitest';
import { getProgressPresentation, type ProgressJob } from './job-progress';

describe('getProgressPresentation', () => {
    it('shows waiting generation jobs as indeterminate instead of static zero percent', () => {
        const job: ProgressJob = {
            kind: 'generation',
            status: 'running',
            current: 0,
            total: 2,
            message: 'Waiting for OpenAI',
        };

        expect(getProgressPresentation(job)).toEqual({
            text: 'Waiting',
            fraction: 0,
            indeterminate: true,
            ariaValueText: 'Waiting for OpenAI',
        });
    });

    it('keeps numeric progress once generated images are saved', () => {
        const job: ProgressJob = {
            kind: 'generation',
            status: 'running',
            current: 1,
            total: 2,
            message: 'Saved image 1/2',
        };

        expect(getProgressPresentation(job)).toEqual({
            text: '1/2 50%',
            fraction: 0.5,
            indeterminate: false,
            ariaValueText: '1/2 50%',
        });
    });
});
