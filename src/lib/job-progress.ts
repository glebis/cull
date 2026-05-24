export interface ProgressJob {
    kind: string;
    status: string;
    current: number;
    total: number;
    message?: string | null;
    downloaded?: number;
    totalBytes?: number;
}

export interface ProgressPresentation {
    text: string;
    fraction: number;
    indeterminate: boolean;
    ariaValueText: string;
}

function percent(job: ProgressJob): string {
    if (job.total === 0) return '';
    return `${Math.round((job.current / job.total) * 100)}%`;
}

function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const mb = bytes / (1024 * 1024);
    if (mb >= 1) return `${mb.toFixed(0)} MB`;
    const kb = bytes / 1024;
    return `${kb.toFixed(0)} KB`;
}

function progressText(job: ProgressJob): string {
    if (job.downloaded !== undefined && job.totalBytes !== undefined) {
        if (job.totalBytes > 0) {
            return `${formatBytes(job.downloaded)} / ${formatBytes(job.totalBytes)} ${percent(job)}`;
        }
        return formatBytes(job.downloaded);
    }
    if (job.total > 0) return `${job.current}/${job.total} ${percent(job)}`;
    return 'Working';
}

function progressFraction(job: ProgressJob): number {
    if (job.total === 0) return 0;
    return Math.min(1, job.current / job.total);
}

export function getProgressPresentation(job: ProgressJob): ProgressPresentation {
    if (job.kind === 'generation' && job.status === 'running' && job.current === 0 && job.total > 0) {
        const ariaValueText = job.message?.trim() || 'Waiting for generation provider';
        return {
            text: 'Waiting',
            fraction: 0,
            indeterminate: true,
            ariaValueText,
        };
    }

    const text = progressText(job);
    return {
        text,
        fraction: progressFraction(job),
        indeterminate: false,
        ariaValueText: text,
    };
}
