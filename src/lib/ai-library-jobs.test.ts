import { beforeEach, describe, expect, it, vi } from 'vitest';
import { runAiLibraryJob, type AiLibraryJobDependencies } from './ai-library-jobs';

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

    it('reports partial failures instead of claiming full completion', async () => {
        const deps = dependencies();
        vi.mocked(deps.detectNsfw).mockResolvedValue(1);
        await runAiLibraryJob('sensitive-content', deps);
        expect(deps.toast).toHaveBeenCalledWith(expect.stringContaining('1 of 2'), expect.objectContaining({ type: 'warning' }));
    });
});
