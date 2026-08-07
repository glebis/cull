import { beforeEach, describe, expect, it, vi } from 'vitest';
import { runAiLibraryJob, type AiLibraryJobDependencies, type AiLibraryJobKind } from './ai-library-jobs';

function dependencies(): AiLibraryJobDependencies {
    return {
        getAppSetting: vi.fn().mockResolvedValue('medium'),
        isYoloAvailable: vi.fn().mockResolvedValue(true),
        isNudenetAvailable: vi.fn().mockResolvedValue(true),
        checkOllama: vi.fn().mockResolvedValue(['minicpm-v:latest']),
        getOllamaConfig: vi.fn().mockResolvedValue(['http://localhost:11434', 'minicpm-v']),
        listMissingDetection: vi.fn().mockResolvedValue(['one', 'two']),
        listMissingVision: vi.fn().mockResolvedValue(['one', 'two']),
        detectObjects: vi.fn().mockResolvedValue(2),
        detectNsfw: vi.fn().mockResolvedValue(2),
        analyzeImages: vi.fn().mockResolvedValue(2),
        toast: vi.fn(),
        openAiSettings: vi.fn(),
        refreshLibrary: vi.fn().mockResolvedValue(undefined),
    };
}

describe('AI library jobs', () => {
    beforeEach(() => vi.clearAllMocks());

    it('runs YOLO only for IDs pending the active variant', async () => {
        const deps = dependencies();
        await runAiLibraryJob('objects', deps);
        expect(deps.listMissingDetection).toHaveBeenCalledWith('yolo11m');
        expect(deps.detectObjects).toHaveBeenCalledWith(['one', 'two'], 'medium');
        expect(deps.refreshLibrary).toHaveBeenCalledWith(true);
    });

    it('uses the exact NudeNet model status and pending IDs', async () => {
        const deps = dependencies();
        await runAiLibraryJob('sensitive-content', deps);
        expect(deps.listMissingDetection).toHaveBeenCalledWith('nudenet');
        expect(deps.detectNsfw).toHaveBeenCalledWith(['one', 'two']);
    });

    it.each([
        ['nano', 'yolo11n'],
        ['small', 'yolo11s'],
    ])('maps the %s variant to exact model %s and passes the selected variant', async (variant, model) => {
        const deps = dependencies();
        vi.mocked(deps.getAppSetting).mockResolvedValue(variant);

        await runAiLibraryJob('objects', deps);

        expect(deps.listMissingDetection).toHaveBeenCalledWith(model);
        expect(deps.detectObjects).toHaveBeenCalledWith(['one', 'two'], variant);
    });

    it('uses the configured Ollama vision model and accepts the latest alias', async () => {
        const deps = dependencies();
        await runAiLibraryJob('descriptions', deps);
        expect(deps.listMissingVision).toHaveBeenCalledWith('minicpm-v');
        expect(deps.analyzeImages).toHaveBeenCalledWith(['one', 'two']);
    });

    it('deep-links to AI Settings when a prerequisite is missing', async () => {
        const deps = dependencies();
        vi.mocked(deps.isYoloAvailable).mockResolvedValue(false);
        await runAiLibraryJob('objects', deps);
        expect(deps.detectObjects).not.toHaveBeenCalled();
        const options = vi.mocked(deps.toast).mock.calls[0][1];
        expect(options.actions?.[0].label).toBe('Open AI Settings');
        options.actions?.[0].onclick();
        expect(deps.openAiSettings).toHaveBeenCalled();
    });

    it.each<[
        AiLibraryJobKind,
        (deps: AiLibraryJobDependencies) => void,
    ]>([
        ['objects', deps => vi.mocked(deps.listMissingDetection).mockResolvedValue([])],
        ['sensitive-content', deps => vi.mocked(deps.listMissingDetection).mockResolvedValue([])],
        ['descriptions', deps => vi.mocked(deps.listMissingVision).mockResolvedValue([])],
    ])('does not process or refresh when %s has no pending work', async (kind, arrangeNoWork) => {
        const deps = dependencies();
        arrangeNoWork(deps);

        await runAiLibraryJob(kind, deps);

        expect(deps.detectObjects).not.toHaveBeenCalled();
        expect(deps.detectNsfw).not.toHaveBeenCalled();
        expect(deps.analyzeImages).not.toHaveBeenCalled();
        expect(deps.refreshLibrary).not.toHaveBeenCalled();
    });

    it('rejects a duplicate same-kind run while the first run is pending', async () => {
        const deps = dependencies();
        let announceStarted!: () => void;
        let finishFirst!: (processed: number) => void;
        const started = new Promise<void>(resolve => { announceStarted = resolve; });
        const pendingProcessor = new Promise<number>(resolve => { finishFirst = resolve; });
        vi.mocked(deps.detectObjects).mockImplementation(async () => {
            announceStarted();
            return pendingProcessor;
        });

        const firstRun = runAiLibraryJob('objects', deps);
        await started;
        await runAiLibraryJob('objects', deps);

        expect(deps.detectObjects).toHaveBeenCalledOnce();
        expect(deps.toast).toHaveBeenCalledWith('This library job is already running', expect.objectContaining({ type: 'info' }));

        finishFirst(2);
        await firstRun;
    });

    it('releases the running guard after a thrown processor error so retry works', async () => {
        const deps = dependencies();
        vi.mocked(deps.detectObjects)
            .mockRejectedValueOnce(new Error('engine failed'))
            .mockResolvedValueOnce(2);

        await runAiLibraryJob('objects', deps);
        await runAiLibraryJob('objects', deps);

        expect(deps.detectObjects).toHaveBeenCalledTimes(2);
        expect(deps.toast).toHaveBeenCalledWith('Library AI job failed', expect.objectContaining({ type: 'error' }));
        expect(deps.toast).toHaveBeenCalledWith('Object detection complete', expect.objectContaining({ type: 'success' }));
    });

    it('reports partial failures instead of claiming full completion', async () => {
        const deps = dependencies();
        vi.mocked(deps.detectNsfw).mockResolvedValue(1);
        await runAiLibraryJob('sensitive-content', deps);
        expect(deps.toast).toHaveBeenCalledWith(expect.stringContaining('1 of 2'), expect.objectContaining({ type: 'warning' }));
    });
});
