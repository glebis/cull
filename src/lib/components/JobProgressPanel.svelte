<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { listen } from '@tauri-apps/api/event';
    import { cancelJob as cancelJobApi, listJobs, pauseJob as pauseJobApi, resumeJob as resumeJobApi, type JobSnapshot } from '$lib/api';
    import { loadImagesForCurrentScope, refreshImageCount } from '$lib/image-loading';
    import { getProgressPresentation } from '$lib/job-progress';

    interface JobInfo {
        job_id: string;
        kind: string;
        status: string;
        current: number;
        total: number;
        message: string | null;
        error?: string | null;
        downloaded?: number;
        totalBytes?: number;
        itemFraction?: number;
        photosSummary?: boolean;
        fadeOut?: boolean;
    }

    let jobs = $state<JobInfo[]>([]);
    let visible = $derived(jobs.length > 0);
    let unlisteners: (() => void)[] = [];
    let fadeTimers: Map<string, ReturnType<typeof setTimeout>> = new Map();

    function handlePhotosImportStarted(event: Event) {
        const detail = (event as CustomEvent<{ job_id: string; total: number }>).detail;
        if (!detail?.job_id) return;
        upsertJob(detail.job_id, 'import', 'running', 0, detail.total, 'Apple Photos');
    }

    async function refreshAfterPhotosImport() {
        await Promise.all([
            loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true }),
            refreshImageCount(),
        ]);
    }

    function formatProgressBytes(bytes: number): string {
        if (bytes >= 1024 * 1024) return `${Math.round(bytes / (1024 * 1024))} MB`;
        if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
        return `${bytes} B`;
    }

    function photosProgressMessage(payload: any, phase: string): string {
        const parts = [payload.filename ? `${phase} ${payload.filename}` : phase];
        if (payload.bytes_current != null && payload.bytes_total != null) {
            parts.push(`${formatProgressBytes(payload.bytes_current)} / ${formatProgressBytes(payload.bytes_total)}`);
        } else if (payload.bytes_current != null) {
            parts.push(formatProgressBytes(payload.bytes_current));
        }
        if (payload.fraction != null) parts.push(`${Math.round(payload.fraction * 100)}%`);
        return parts.join(' · ');
    }

    function displayedProgress(job: JobInfo): number {
        return Math.min(job.total, job.current + (job.itemFraction ?? 0));
    }

    onMount(async () => {
        window.addEventListener('photos-import-started', handlePhotosImportStarted);
        try {
            const recent = await listJobs();
            for (const snapshot of recent) {
                const job = jobFromSnapshot(snapshot);
                upsertJob(job.job_id, job.kind, job.status, job.current, job.total, job.message, job.error ?? null);
                if (isTerminal(job.status)) scheduleFadeOut(job.job_id);
            }

            const u1 = await listen<any>('import-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_import`, 'import', 'running', e.payload.current, e.payload.total, e.payload.filename);
            });
            const u2 = await listen<any>('embedding-progress', (e) => {
                const provider = e.payload.provider === 'gemini'
                    ? 'gemini-embeddings'
                    : e.payload.provider === 'cohere'
                        ? 'cohere-embeddings'
                    : e.payload.provider === 'openai'
                        ? 'openai-embeddings'
                        : e.payload.provider === 'ollama'
                            ? 'ollama-embeddings'
                            : 'embeddings';
                upsertJob(e.payload.job_id ?? `evt_${provider}`, provider, 'running', e.payload.current, e.payload.total, null);
            });
            const u3 = await listen<any>('detection-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_detection`, 'detection', 'running', e.payload.current, e.payload.total, e.payload.model);
            });
            const u4 = await listen<any>('vision-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_vision`, 'vision', 'running', e.payload.current, e.payload.total, e.payload.model);
            });
            const u5 = await listen<any>('job-status-changed', (e) => {
                const p = e.payload;
                const existing = jobs.find(job => job.job_id === p.job_id);
                const preservePhotosSummary = existing?.photosSummary && isTerminal(p.status);
                upsertJob(
                    p.job_id,
                    p.kind ?? 'unknown',
                    p.status,
                    p.current ?? 0,
                    p.total ?? 0,
                    preservePhotosSummary ? existing.message : p.message ?? null,
                    preservePhotosSummary ? existing.error ?? null : p.error ?? null,
                );
                if (isTerminal(p.status)) {
                    scheduleFadeOut(p.job_id);
                }
            });
            const u6 = await listen<any>('rescan-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_rescan`, 'rescan', 'running', e.payload.current, e.payload.total, null);
            });
            const u7 = await listen<any>('generation-progress', (e) => {
                upsertJob(
                    e.payload.job_id ?? `evt_generation`,
                    'generation',
                    'running',
                    e.payload.current,
                    e.payload.total,
                    e.payload.message ?? `Generating image ${e.payload.current}/${e.payload.total}`,
                );
            });
            const u8 = await listen<any>('thumbnail-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_thumbnails`, 'thumbnails', 'running', e.payload.current, e.payload.total, null);
            });
            const u9 = await listen<any>('nsfw-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_nsfw`, 'nsfw', 'running', e.payload.current, e.payload.total, null);
            });
            const u10 = await listen<any>('model-download-progress', (e) => {
                const model = e.payload.model ?? 'clip-vit-b32';
                upsertDownload(
                    e.payload.job_id ?? `evt_${model}_download`,
                    `${model}-download`,
                    e.payload.downloaded,
                    e.payload.total,
                    e.payload.status,
                    e.payload.error ?? null,
                );
            });
            const u11 = await listen<any>('yolo-download-progress', (e) => {
                upsertDownload('evt_yolo_download', 'yolo-download', e.payload.downloaded, e.payload.total, e.payload.status, e.payload.variant);
            });
            const u12 = await listen<any>('nudenet-download-progress', (e) => {
                upsertDownload('evt_nudenet_download', 'nudenet-download', e.payload.downloaded, e.payload.total, e.payload.status);
            });
            const u13 = await listen<any>('auto-detection-start', (e) => {
                upsertJob(e.payload.job_id, 'detection', 'running', e.payload.current ?? 0, e.payload.total ?? 0, e.payload.model ?? 'model');
            });
            const u14 = await listen<any>('auto-detection-progress', (e) => {
                upsertJob(e.payload.job_id, 'detection', 'running', e.payload.current, e.payload.total, e.payload.model ?? 'model');
            });
            const u15 = await listen<any>('auto-detection-complete', (e) => {
                const status = e.payload.cancelled ? 'cancelled' : 'completed';
                upsertJob(e.payload.job_id, 'detection', status, e.payload.current ?? e.payload.total ?? 0, e.payload.total ?? 0, null);
                scheduleFadeOut(e.payload.job_id);
            });
            const u16 = await listen<any>('health-check-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_health_check`, 'health-check', 'running', e.payload.current, e.payload.total, null);
            });
            const u17 = await listen<any>('backfill-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_raw_backfill`, 'raw-backfill', 'running', e.payload.current, e.payload.total, null);
            });
            const u18 = await listen<any>('quality-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_quality`, 'quality', 'running', e.payload.current, e.payload.total, e.payload.analyzer ?? null);
            });
            const u19 = await listen<any>('ocr-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_ocr`, 'ocr', 'running', e.payload.current, e.payload.total, e.payload.model);
            });
            const u20 = await listen<any>('photos-import-progress', (e) => {
                const p = e.payload;
                const total = p.total ?? 0;
                const current = Math.min(Math.max(0, (p.current ?? 1) - 1), total);
                const phase = p.phase === 'download' ? 'Downloading' : p.phase === 'import' ? 'Importing' : 'Preparing';
                upsertJob(p.job_id, 'import', 'running', current, total, photosProgressMessage(p, phase));
                const itemFraction = p.fraction == null ? 0 : Math.min(1, Math.max(0, p.fraction));
                jobs = jobs.map(job => job.job_id === p.job_id ? { ...job, itemFraction } : job);
            });
            const u21 = await listen<any>('photos-import-finished', (e) => {
                const p = e.payload;
                const counted = (p.imported ?? 0) + (p.reused ?? 0) + (p.failed ?? 0) + (p.skipped ?? 0) + (p.inaccessible ?? 0) + (p.cancelled ?? 0);
                const total = counted || jobs.find(job => job.job_id === p.job_id)?.total || 0;
                const status = (p.cancelled ?? 0) > 0
                    ? 'cancelled'
                    : p.error || (p.failed ?? 0) > 0 || (p.inaccessible ?? 0) > 0
                        ? 'failed'
                        : 'completed';
                const countsMessage = [
                    `${p.imported ?? 0} imported`,
                    `${p.reused ?? 0} reused`,
                    `${p.failed ?? 0} failed`,
                    `${p.skipped ?? 0} skipped`,
                    `${p.inaccessible ?? 0} inaccessible`,
                    `${p.cancelled ?? 0} cancelled`,
                ].join(' · ');
                const message = p.error ? `${countsMessage} · ${p.error}` : countsMessage;
                upsertJob(p.job_id, 'import', status, total, total, message, null);
                jobs = jobs.map(job => job.job_id === p.job_id ? { ...job, itemFraction: undefined, photosSummary: true } : job);
                scheduleFadeOut(p.job_id);
                if ((p.imported ?? 0) > 0) {
                    void refreshAfterPhotosImport().catch(error => console.error('Failed to refresh after Apple Photos import:', error));
                }
            });
            unlisteners = [u1, u2, u3, u4, u5, u6, u7, u8, u9, u10, u11, u12, u13, u14, u15, u16, u17, u18, u19, u20, u21];
        } catch {
            // Not in Tauri environment
        }
    });

    onDestroy(() => {
        window.removeEventListener('photos-import-started', handlePhotosImportStarted);
        unlisteners.forEach(u => u());
        fadeTimers.forEach(t => clearTimeout(t));
    });

    function jobFromSnapshot(snapshot: JobSnapshot): JobInfo {
        return {
            job_id: snapshot.job_id,
            kind: snapshot.kind,
            status: snapshot.status,
            current: snapshot.current,
            total: snapshot.total,
            message: snapshot.message,
            error: snapshot.error,
        };
    }

    function isTerminal(status: string): boolean {
        return ['completed', 'failed', 'cancelled'].includes(status);
    }

    function upsertJob(jobId: string, kind: string, status: string, current: number, total: number, message: string | null, error: string | null = null) {
        const idx = jobs.findIndex(j => j.job_id === jobId);
        const existing = idx >= 0 ? jobs[idx] : null;
        const info: JobInfo = { ...existing, job_id: jobId, kind, status, current, total, message, error, fadeOut: false };
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

    function upsertDownload(jobId: string, kind: string, downloaded: number, totalBytes: number, rawStatus: string, message: string | null = null) {
        const status = rawStatus === 'complete' ? 'completed' : rawStatus || 'running';
        upsertJob(jobId, kind, status, downloaded, totalBytes, message);
        jobs = jobs.map(j => j.job_id === jobId ? { ...j, downloaded, totalBytes } : j);
        if (isTerminal(status)) scheduleFadeOut(jobId);
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

    function dismissJob(jobId: string) {
        if (fadeTimers.has(jobId)) clearTimeout(fadeTimers.get(jobId)!);
        jobs = jobs.map(j => j.job_id === jobId ? { ...j, fadeOut: true } : j);
        setTimeout(() => { jobs = jobs.filter(j => j.job_id !== jobId); }, 300);
    }

    async function cancelJob(jobId: string) {
        try {
            await cancelJobApi(jobId);
            const latest = jobs.find(job => job.job_id === jobId);
            if (!latest || isTerminal(latest.status)) return;
            upsertJob(jobId, latest.kind, 'cancelling', latest.current, latest.total, 'Cancelling...');
        } catch {
            dismissJob(jobId);
        }
    }

    async function pauseJob(jobId: string) {
        const job = jobs.find(j => j.job_id === jobId);
        if (!job) return;
        try {
            await pauseJobApi(jobId);
            upsertJob(jobId, job.kind, 'paused', job.current, job.total, 'Paused');
        } catch {
            dismissJob(jobId);
        }
    }

    async function resumeJob(jobId: string) {
        const job = jobs.find(j => j.job_id === jobId);
        if (!job) return;
        try {
            await resumeJobApi(jobId);
            upsertJob(jobId, job.kind, 'running', job.current, job.total, null);
        } catch {
            dismissJob(jobId);
        }
    }

    function isDownloadJob(job: JobInfo): boolean {
        return job.kind.endsWith('-download') || ['clip-download', 'yolo-download', 'nudenet-download'].includes(job.kind);
    }

    function kindLabel(kind: string): string {
        const labels: Record<string, string> = {
            import: 'Import',
            embeddings: 'Embeddings',
            'gemini-embeddings': 'Gemini embeddings',
            'cohere-embeddings': 'Cohere embeddings',
            'openai-embeddings': 'OpenAI embeddings',
            'ollama-embeddings': 'Ollama embeddings',
            detection: 'Detection',
            nsfw: 'NSFW detection',
            vision: 'Vision',
            rescan: 'Rescan',
            thumbnails: 'Thumbnails',
            generation: 'Generation',
            'clip-download': 'CLIP model',
            'yolo-download': 'YOLO model',
            'nudenet-download': 'NudeNet model',
            'auto-detection': 'Auto detection',
            'health-check': 'Library health',
            'raw-backfill': 'RAW previews',
            quality: 'Quality metrics',
            ocr: 'OCR',
        };
        if (labels[kind]) return labels[kind];
        if (kind.endsWith('-download')) {
            const model = kind.slice(0, -'-download'.length);
            const modelLabels: Record<string, string> = {
                'clip-vit-b32': 'CLIP model',
                'dinov2-vits14': 'DINOv2 model',
            };
            return modelLabels[model] ?? `${model} model`;
        }
        return kind;
    }

    function statusIcon(status: string): string {
        switch (status) {
            case 'completed': return '✓';
            case 'failed': return '✕';
            case 'cancelled': return '—';
            case 'cancelling': return '…';
            case 'paused': return '||';
            default: return '●';
        }
    }

</script>

{#if visible}
    <div class="job-panel" role="status" aria-label="Background jobs">
        {#each jobs as job (job.job_id)}
            {@const progress = getProgressPresentation(job)}
            {@const shownProgress = displayedProgress(job)}
            <div class="job-row {job.status}" class:fade-out={job.fadeOut}>
                <div class="job-header">
                    <span class="job-icon {job.status}">{statusIcon(job.status)}</span>
                    <span class="job-label">{kindLabel(job.kind)}</span>
                    {#if job.error || job.message}
                        <span class="job-message" class:error={!!job.error}>{job.error ?? job.message}</span>
                    {/if}
                    <span class="job-progress-text">
                        {#if job.status === 'running' || job.status === 'cancelling'}
                            {progress.text}
                        {:else if job.status === 'paused'}
                            Paused
                        {:else if job.status === 'completed'}
                            Done
                        {:else if job.status === 'failed'}
                            Failed
                        {:else if job.status === 'cancelled'}
                            Cancelled
                        {/if}
                    </span>
                    {#if job.status === 'running' && isDownloadJob(job)}
                        <button class="cancel-btn" onclick={() => pauseJob(job.job_id)} title="Pause" aria-label="Pause {kindLabel(job.kind)}">||</button>
                        <button class="cancel-btn" onclick={() => cancelJob(job.job_id)} title="Cancel" aria-label="Cancel {kindLabel(job.kind)}">✕</button>
                    {:else if job.status === 'paused' && isDownloadJob(job)}
                        <button class="cancel-btn" onclick={() => resumeJob(job.job_id)} title="Resume" aria-label="Resume {kindLabel(job.kind)}">&gt;</button>
                        <button class="cancel-btn" onclick={() => cancelJob(job.job_id)} title="Cancel" aria-label="Cancel {kindLabel(job.kind)}">✕</button>
                    {:else if job.status === 'running'}
                        <button class="cancel-btn" onclick={() => cancelJob(job.job_id)} title="Cancel" aria-label="Cancel {kindLabel(job.kind)}">✕</button>
                    {:else}
                        <button class="cancel-btn" onclick={() => dismissJob(job.job_id)} title="Dismiss" aria-label="Dismiss">✕</button>
                    {/if}
                </div>
                {#if job.status === 'running' || job.status === 'cancelling' || job.status === 'paused'}
                    <div
                        class="progress-track"
                        role="progressbar"
                        aria-valuemin="0"
                        aria-valuemax={job.total}
                        aria-valuenow={progress.indeterminate ? undefined : shownProgress}
                        aria-valuetext={job.itemFraction != null ? `${progress.text} completed · ${job.message}` : progress.ariaValueText}
                    >
                        <div
                            class="progress-fill {job.status}"
                            class:indeterminate={progress.indeterminate}
                            style={progress.indeterminate ? '' : `width: ${job.total > 0 ? (shownProgress / job.total) * 100 : progress.fraction * 100}%`}
                        ></div>
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
        z-index: var(--z-panel);
        width: 320px;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 1.5);
        padding: 6px 0;
        box-shadow: 0 4px 16px color-mix(in srgb, var(--bg) 70%, transparent);
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
    .job-icon.paused { color: var(--orange); }
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
    .job-message.error {
        color: var(--red);
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
    .progress-fill.paused {
        background: var(--orange);
    }
    .progress-fill.indeterminate {
        width: 34%;
        animation: generation-wait 1.2s ease-in-out infinite;
    }

    @keyframes generation-wait {
        0% {
            transform: translateX(-100%);
        }
        100% {
            transform: translateX(300%);
        }
    }
</style>
