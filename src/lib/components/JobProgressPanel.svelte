<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { listen } from '@tauri-apps/api/event';

    interface JobInfo {
        job_id: string;
        kind: string;
        status: string;
        current: number;
        total: number;
        message: string | null;
        fadeOut?: boolean;
    }

    let jobs = $state<JobInfo[]>([]);
    let visible = $derived(jobs.length > 0);
    let unlisteners: (() => void)[] = [];
    let fadeTimers: Map<string, ReturnType<typeof setTimeout>> = new Map();

    onMount(async () => {
        try {
            const u1 = await listen<any>('import-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_import`, 'import', 'running', e.payload.current, e.payload.total, e.payload.filename);
            });
            const u2 = await listen<any>('embedding-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_embeddings`, 'embeddings', 'running', e.payload.current, e.payload.total, null);
            });
            const u3 = await listen<any>('detection-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_detection`, 'detection', 'running', e.payload.current, e.payload.total, e.payload.model);
            });
            const u4 = await listen<any>('vision-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_vision`, 'vision', 'running', e.payload.current, e.payload.total, e.payload.model);
            });
            const u5 = await listen<any>('job-status-changed', (e) => {
                const p = e.payload;
                upsertJob(p.job_id, p.kind ?? 'unknown', p.status, p.current ?? 0, p.total ?? 0, null);
                if (['completed', 'failed', 'cancelled'].includes(p.status)) {
                    scheduleFadeOut(p.job_id);
                }
            });
            const u6 = await listen<any>('rescan-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_rescan`, 'rescan', 'running', e.payload.current, e.payload.total, null);
            });
            const u7 = await listen<any>('generation-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_generation`, 'generation', 'running', e.payload.current, e.payload.total, `Generating image ${e.payload.current}/${e.payload.total}`);
            });
            const u8 = await listen<any>('thumbnail-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_thumbnails`, 'thumbnails', 'running', e.payload.current, e.payload.total, null);
            });
            unlisteners = [u1, u2, u3, u4, u5, u6, u7, u8];
        } catch {
            // Not in Tauri environment
        }
    });

    onDestroy(() => {
        unlisteners.forEach(u => u());
        fadeTimers.forEach(t => clearTimeout(t));
    });

    function upsertJob(jobId: string, kind: string, status: string, current: number, total: number, message: string | null) {
        const idx = jobs.findIndex(j => j.job_id === jobId);
        const info: JobInfo = { job_id: jobId, kind, status, current, total, message };
        if (idx >= 0) {
            jobs[idx] = info;
        } else {
            jobs = [...jobs, info];
        }
        if (status === 'running' && current >= total && total > 0) {
            info.status = 'completed';
            scheduleFadeOut(jobId);
        }
    }

    function scheduleFadeOut(jobId: string) {
        if (fadeTimers.has(jobId)) clearTimeout(fadeTimers.get(jobId)!);
        const timer = setTimeout(() => {
            jobs = jobs.map(j => j.job_id === jobId ? { ...j, fadeOut: true } : j);
            setTimeout(() => {
                jobs = jobs.filter(j => j.job_id !== jobId);
            }, 300);
        }, 5000);
        fadeTimers.set(jobId, timer);
    }

    async function cancelJob(jobId: string) {
        try {
            const { invoke } = await import('@tauri-apps/api/core');
            await invoke('cancel_job', { jobId });
        } catch (e) {
            console.error('Failed to cancel job:', e);
        }
    }

    function kindLabel(kind: string): string {
        const labels: Record<string, string> = {
            import: 'Import',
            embeddings: 'Embeddings',
            detection: 'Detection',
            vision: 'Vision',
            rescan: 'Rescan',
            thumbnails: 'Thumbnails',
        };
        return labels[kind] ?? kind;
    }

    function statusIcon(status: string): string {
        switch (status) {
            case 'completed': return '✓';
            case 'failed': return '✕';
            case 'cancelled': return '—';
            case 'cancelling': return '…';
            default: return '●';
        }
    }

    function percent(j: JobInfo): string {
        if (j.total === 0) return '';
        return `${Math.round((j.current / j.total) * 100)}%`;
    }

    function progressFraction(j: JobInfo): number {
        if (j.total === 0) return 0;
        return j.current / j.total;
    }
</script>

{#if visible}
    <div class="job-panel" role="status" aria-label="Background jobs">
        {#each jobs as job (job.job_id)}
            <div class="job-row {job.status}" class:fade-out={job.fadeOut}>
                <div class="job-header">
                    <span class="job-icon {job.status}">{statusIcon(job.status)}</span>
                    <span class="job-label">{kindLabel(job.kind)}</span>
                    {#if job.message}
                        <span class="job-message">{job.message}</span>
                    {/if}
                    <span class="job-progress-text">
                        {#if job.status === 'running' || job.status === 'cancelling'}
                            {job.current}/{job.total} {percent(job)}
                        {:else if job.status === 'completed'}
                            Done
                        {:else if job.status === 'failed'}
                            Failed
                        {:else if job.status === 'cancelled'}
                            Cancelled
                        {/if}
                    </span>
                    {#if job.status === 'running'}
                        <button class="cancel-btn" onclick={() => cancelJob(job.job_id)} title="Cancel">✕</button>
                    {/if}
                </div>
                {#if job.status === 'running' || job.status === 'cancelling'}
                    <div class="progress-track">
                        <div class="progress-fill {job.status}" style="width: {progressFraction(job) * 100}%"></div>
                    </div>
                {/if}
            </div>
        {/each}
    </div>
{/if}

<style>
    .job-panel {
        position: fixed;
        bottom: 40px;
        right: 16px;
        z-index: 900;
        width: 320px;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 6px;
        padding: 6px 0;
        box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
        font-size: 12px;
        font-family: inherit;
    }

    .job-row {
        padding: 6px 12px;
        transition: opacity 0.3s;
    }
    .job-row.fade-out {
        opacity: 0;
    }
    .job-row + .job-row {
        border-top: 1px solid var(--border);
    }

    .job-header {
        display: flex;
        align-items: center;
        gap: 6px;
    }

    .job-icon {
        font-size: 10px;
        flex-shrink: 0;
    }
    .job-icon.running { color: var(--blue); }
    .job-icon.cancelling { color: var(--orange); }
    .job-icon.completed { color: var(--green); }
    .job-icon.failed { color: var(--red); }
    .job-icon.cancelled { color: var(--orange); }

    .job-label {
        color: var(--text);
        font-weight: 500;
    }

    .job-message {
        color: var(--text-secondary);
        flex: 1;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 11px;
    }

    .job-progress-text {
        color: var(--text-secondary);
        font-size: 11px;
        white-space: nowrap;
        margin-left: auto;
    }

    .cancel-btn {
        background: none;
        border: none;
        color: var(--text-secondary);
        font-size: 11px;
        cursor: pointer;
        padding: 0 2px;
        line-height: 1;
        flex-shrink: 0;
    }
    .cancel-btn:hover {
        color: var(--red);
    }

    .progress-track {
        height: 3px;
        background: var(--border);
        border-radius: 2px;
        margin-top: 4px;
        overflow: hidden;
    }

    .progress-fill {
        height: 100%;
        border-radius: 2px;
        transition: width 0.3s ease;
    }
    .progress-fill.running {
        background: var(--blue);
    }
    .progress-fill.cancelling {
        background: var(--orange);
    }
</style>
