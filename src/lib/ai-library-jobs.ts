export type AiLibraryJobKind = 'objects' | 'sensitive-content' | 'descriptions';

interface ToastOptions {
    type: 'success' | 'info' | 'warning' | 'error';
    detail?: string;
    duration?: number;
    actions?: { label: string; onclick: () => void }[];
}

export interface AiLibraryJobDependencies {
    getAppSetting: (key: string) => Promise<string | null>;
    isYoloAvailable: (variant?: string) => Promise<boolean>;
    isNudenetAvailable: () => Promise<boolean>;
    checkOllama: () => Promise<string[]>;
    getOllamaConfig: () => Promise<[string, string]>;
    listMissingDetection: (model: string) => Promise<string[]>;
    listMissingVision: (source: string) => Promise<string[]>;
    detectObjects: (imageIds: string[], variant?: string) => Promise<number>;
    detectNsfw: (imageIds: string[]) => Promise<number>;
    analyzeImages: (imageIds: string[]) => Promise<number>;
    toast: (message: string, options: ToastOptions) => void;
    openAiSettings: () => void;
    refreshLibrary: (detectionsChanged: boolean) => void | Promise<void>;
}

const running = new Set<AiLibraryJobKind>();
const YOLO_MODELS: Record<string, string> = { nano: 'yolo11n', small: 'yolo11s', medium: 'yolo11m' };

function normalizedModel(model: string): string { return model.replace(/:latest$/, ''); }

function missingPrerequisite(deps: AiLibraryJobDependencies, message: string) {
    deps.toast(message, {
        type: 'warning',
        duration: 8000,
        actions: [{ label: 'Open AI Settings', onclick: deps.openAiSettings }],
    });
}

export async function runAiLibraryJob(kind: AiLibraryJobKind, deps: AiLibraryJobDependencies): Promise<void> {
    if (running.has(kind)) {
        deps.toast('This library job is already running', { type: 'info', duration: 3000 });
        return;
    }
    running.add(kind);
    try {
        let pending: string[];
        let processed: number;
        let label: string;
        let detectionsChanged = false;

        if (kind === 'objects') {
            const variant = (await deps.getAppSetting('yolo_variant')) || 'medium';
            if (!(await deps.isYoloAvailable(variant))) { missingPrerequisite(deps, 'YOLO is not configured'); return; }
            pending = await deps.listMissingDetection(YOLO_MODELS[variant] ?? YOLO_MODELS.medium);
            if (pending.length === 0) { deps.toast('Object detection is already complete', { type: 'info', duration: 3000 }); return; }
            processed = await deps.detectObjects(pending, variant);
            label = 'Object detection';
            detectionsChanged = true;
        } else if (kind === 'sensitive-content') {
            if (!(await deps.isNudenetAvailable())) { missingPrerequisite(deps, 'NudeNet is not configured'); return; }
            pending = await deps.listMissingDetection('nudenet');
            if (pending.length === 0) { deps.toast('Sensitive-content scanning is already complete', { type: 'info', duration: 3000 }); return; }
            processed = await deps.detectNsfw(pending);
            label = 'Sensitive-content scan';
            detectionsChanged = true;
        } else {
            const [, model] = await deps.getOllamaConfig();
            const available = await deps.checkOllama().catch(() => []);
            if (!available.some(candidate => normalizedModel(candidate) === normalizedModel(model))) { missingPrerequisite(deps, 'The configured Ollama vision model is unavailable'); return; }
            pending = await deps.listMissingVision(model);
            if (pending.length === 0) { deps.toast('Image descriptions are already complete', { type: 'info', duration: 3000 }); return; }
            processed = await deps.analyzeImages(pending);
            label = 'Image descriptions';
        }

        await deps.refreshLibrary(detectionsChanged);
        if (processed === pending.length) {
            deps.toast(`${label} complete`, { type: 'success', detail: `${processed} images processed`, duration: 5000 });
        } else {
            deps.toast(`${label}: ${processed} of ${pending.length} images processed`, { type: 'warning', detail: `${pending.length - processed} images could not be processed`, duration: 8000 });
        }
    } catch (error) {
        deps.toast('Library AI job failed', { type: 'error', detail: String(error), duration: 8000 });
    } finally {
        running.delete(kind);
    }
}
