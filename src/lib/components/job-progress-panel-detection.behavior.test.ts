// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';

const mocks = vi.hoisted(() => ({
    cancelJob: vi.fn().mockResolvedValue(undefined),
    listen: vi.fn(),
    listJobs: vi.fn().mockResolvedValue([]),
}));

vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('$lib/api', () => ({
    cancelJob: mocks.cancelJob,
    listJobs: mocks.listJobs,
    pauseJob: vi.fn(),
    resumeJob: vi.fn(),
}));

import JobProgressPanel from './JobProgressPanel.svelte';

afterEach(() => cleanup());
beforeEach(() => vi.clearAllMocks());

describe('JobProgressPanel detection jobs', () => {
    it('keeps both model stages on the real cancellable job row', async () => {
        const handlers = new Map<string, (event: { payload: Record<string, unknown> }) => void>();
        mocks.listen.mockImplementation(async (name: string, handler: (event: { payload: Record<string, unknown> }) => void) => {
            handlers.set(name, handler);
            return vi.fn();
        });
        const user = userEvent.setup();
        render(JobProgressPanel);
        await waitFor(() => expect(handlers.has('auto-detection-progress')).toBe(true));

        handlers.get('job-status-changed')?.({
            payload: { job_id: 'job_detection_real', kind: 'detection', status: 'running', current: 0, total: 20 },
        });
        handlers.get('auto-detection-start')?.({
            payload: { job_id: 'job_detection_real', model: 'yolo11m', current: 0, total: 20 },
        });
        handlers.get('auto-detection-progress')?.({
            payload: { job_id: 'job_detection_real', model: 'yolo11m', current: 10, total: 20 },
        });
        handlers.get('auto-detection-start')?.({
            payload: { job_id: 'job_detection_real', model: 'nudenet', current: 10, total: 20 },
        });
        handlers.get('auto-detection-progress')?.({
            payload: { job_id: 'job_detection_real', model: 'nudenet', current: 11, total: 20 },
        });

        await waitFor(() => expect(screen.getAllByText('Detection')).toHaveLength(1));
        expect(screen.getByText(/11\/20/)).toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Cancel Detection' }));
        expect(mocks.cancelJob).toHaveBeenCalledWith('job_detection_real');
    });
});
