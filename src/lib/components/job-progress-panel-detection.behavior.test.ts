// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';

const mocks = vi.hoisted(() => ({
    cancelJob: vi.fn().mockResolvedValue(undefined),
    listen: vi.fn(),
    listJobs: vi.fn().mockResolvedValue([]),
    loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined),
    refreshImageCount: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('$lib/api', () => ({
    cancelJob: mocks.cancelJob,
    listJobs: mocks.listJobs,
    pauseJob: vi.fn(),
    resumeJob: vi.fn(),
}));
vi.mock('$lib/image-loading', () => ({
    loadImagesForCurrentScope: mocks.loadImagesForCurrentScope,
    refreshImageCount: mocks.refreshImageCount,
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

    it('shows a started Photos import, keeps it cancellable, and refreshes the library on completion', async () => {
        const handlers = new Map<string, (event: { payload: Record<string, unknown> }) => void>();
        mocks.listen.mockImplementation(async (name: string, handler: (event: { payload: Record<string, unknown> }) => void) => {
            handlers.set(name, handler);
            return vi.fn();
        });
        const user = userEvent.setup();
        render(JobProgressPanel);
        await waitFor(() => expect(handlers.has('photos-import-progress')).toBe(true));

        window.dispatchEvent(new CustomEvent('photos-import-started', {
            detail: { job_id: 'job_photos', total: 3 },
        }));

        expect(await screen.findByText('Import')).toBeInTheDocument();
        expect(screen.getByText(/0\/3/)).toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Cancel Import' }));
        expect(mocks.cancelJob).toHaveBeenCalledWith('job_photos');

        handlers.get('photos-import-progress')?.({
            payload: {
                job_id: 'job_photos', phase: 'download', current: 2, total: 3, filename: 'Two.jpg',
                bytes_current: 524_288, bytes_total: 1_048_576, fraction: 0.5,
            },
        });
        await waitFor(() => expect(screen.getByText(/1\/3/)).toBeInTheDocument());
        expect(screen.getByText('Downloading Two.jpg · 512 KB / 1 MB · 50%')).toBeInTheDocument();
        expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '1.5');

        handlers.get('photos-import-finished')?.({
            payload: { job_id: 'job_photos', imported: 2, reused: 1, failed: 1, skipped: 3, inaccessible: 4, cancelled: 0 },
        });
        await waitFor(() => expect(mocks.loadImagesForCurrentScope).toHaveBeenCalledWith({
            resetFocus: false,
            force: true,
            invalidateCache: true,
        }));
        expect(mocks.refreshImageCount).toHaveBeenCalledOnce();
        expect(screen.getByText('Failed')).toBeInTheDocument();
        expect(screen.getByText('2 imported · 1 reused · 1 failed · 3 skipped · 4 inaccessible · 0 cancelled')).toBeInTheDocument();

        handlers.get('job-status-changed')?.({
            payload: { job_id: 'job_photos', kind: 'import', status: 'failed', current: 10, total: 10, message: 'generic terminal state' },
        });
        expect(screen.getByText('2 imported · 1 reused · 1 failed · 3 skipped · 4 inaccessible · 0 cancelled')).toBeInTheDocument();
        expect(screen.queryByText('generic terminal state')).not.toBeInTheDocument();

        window.dispatchEvent(new CustomEvent('photos-import-started', {
            detail: { job_id: 'job_photos_fatal', total: 3 },
        }));
        handlers.get('photos-import-finished')?.({
            payload: {
                job_id: 'job_photos_fatal', imported: 1, reused: 0, failed: 2, skipped: 0,
                inaccessible: 0, cancelled: 0, error: 'Database finalization failed',
            },
        });
        expect(await screen.findByText(
            '1 imported · 0 reused · 2 failed · 0 skipped · 0 inaccessible · 0 cancelled · Database finalization failed',
        )).toBeInTheDocument();
        await waitFor(() => expect(mocks.loadImagesForCurrentScope).toHaveBeenCalledTimes(2));
        expect(mocks.refreshImageCount).toHaveBeenCalledTimes(2);
    });
});
