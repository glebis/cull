// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import AiSettings from './AiSettings.svelte';
import { checkOllama, getAppSetting, isNudenetAvailable, isYoloAvailable, setAppSetting, setOllamaConfig } from '$lib/api';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));
vi.mock('$lib/api', () => ({
    checkOllama: vi.fn().mockResolvedValue([]),
    deleteApiKey: vi.fn().mockResolvedValue(undefined),
    getAppSetting: vi.fn(),
    getOllamaConfig: vi.fn().mockResolvedValue(['http://localhost:11434', 'llava']),
    hasApiKey: vi.fn().mockResolvedValue(false),
    isNudenetAvailable: vi.fn().mockResolvedValue(false),
    isYoloAvailable: vi.fn(),
    setApiKey: vi.fn().mockResolvedValue(undefined),
    setAppSetting: vi.fn().mockResolvedValue(undefined),
    setOllamaConfig: vi.fn().mockResolvedValue(undefined),
    validateApiKey: vi.fn().mockResolvedValue(true),
}));

afterEach(() => cleanup());
beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getAppSetting).mockImplementation(async (key) => key === 'yolo_variant' ? 'small' : null);
    vi.mocked(checkOllama).mockResolvedValue([]);
    vi.mocked(isNudenetAvailable).mockResolvedValue(false);
});

function yoloReadinessRow(): HTMLElement {
    return screen.getByText('Object detection · YOLO').parentElement as HTMLElement;
}

function nudenetReadinessRow(): HTMLElement {
    return screen.getByText('Content filter · NudeNet').parentElement as HTMLElement;
}

function ollamaReadinessRow(): HTMLElement {
    return screen.getByText('Image descriptions · Ollama').parentElement as HTMLElement;
}

describe('AI Settings YOLO readiness', () => {
    it('does not present default controls as final while initialization is pending', async () => {
        let resolveVariant!: (value: string | null) => void;
        const pendingVariant = new Promise<string | null>((resolve) => { resolveVariant = resolve; });
        vi.mocked(getAppSetting).mockImplementation((key) => key === 'yolo_variant' ? pendingVariant : Promise.resolve(null));

        render(AiSettings);

        expect(await screen.findByText('Loading AI settings…')).toBeVisible();
        expect(screen.queryByRole('combobox', { name: 'YOLO variant' })).not.toBeInTheDocument();
        resolveVariant('small');
        expect(await screen.findByRole('combobox', { name: 'YOLO variant' })).toHaveValue('small');
    });

    it('shows a tab-local initialization error and retries successfully', async () => {
        vi.mocked(getAppSetting).mockRejectedValueOnce(new Error('database unavailable'));
        const user = userEvent.setup();
        render(AiSettings);

        expect(await screen.findByRole('alert')).toHaveTextContent('Could not load AI settings.');
        expect(screen.queryByRole('combobox', { name: 'YOLO variant' })).not.toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Retry' }));

        expect(await screen.findByRole('combobox', { name: 'YOLO variant' })).toHaveValue('small');
        expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });

    it('checks and renders readiness for the saved YOLO variant', async () => {
        vi.mocked(isYoloAvailable).mockImplementation(async (variant) => variant === 'small');

        render(AiSettings);

        await waitFor(() => expect(yoloReadinessRow()).toHaveTextContent('Ready'));
        expect(isYoloAvailable).toHaveBeenCalledWith('small');
    });

    it('refreshes readiness for a newly selected YOLO variant', async () => {
        vi.mocked(isYoloAvailable).mockImplementation(async (variant) => variant === 'small');
        const user = userEvent.setup();
        render(AiSettings);
        await waitFor(() => expect(yoloReadinessRow()).toHaveTextContent('Ready'));

        await user.selectOptions(screen.getByRole('combobox', { name: 'YOLO variant' }), 'nano');

        await waitFor(() => expect(yoloReadinessRow()).toHaveTextContent('Not installed'));
        expect(isYoloAvailable).toHaveBeenLastCalledWith('nano');
    });

    it('rolls back the selector and readiness when variant persistence fails', async () => {
        vi.mocked(isYoloAvailable).mockImplementation(async (variant) => variant === 'small');
        vi.mocked(setAppSetting).mockRejectedValueOnce(new Error('disk full'));
        const user = userEvent.setup();
        render(AiSettings);
        await waitFor(() => expect(yoloReadinessRow()).toHaveTextContent('Ready'));
        const selector = screen.getByRole('combobox', { name: 'YOLO variant' });

        await user.selectOptions(selector, 'nano');

        expect(await screen.findByRole('alert')).toHaveTextContent('Could not save YOLO variant. The previous selection was kept.');
        expect(selector).toHaveValue('small');
        expect(yoloReadinessRow()).toHaveTextContent('Ready');
        expect(setAppSetting).toHaveBeenCalledWith('yolo_variant', 'nano');
        expect(setOllamaConfig).not.toHaveBeenCalled();
        expect(isYoloAvailable).not.toHaveBeenCalledWith('nano');
    });

    it('shows exact installed and missing states for YOLO and NudeNet', async () => {
        vi.mocked(isYoloAvailable).mockResolvedValue(false);
        vi.mocked(isNudenetAvailable).mockResolvedValue(true);

        render(AiSettings);

        await waitFor(() => expect(nudenetReadinessRow()).toHaveTextContent('Ready'));
        expect(yoloReadinessRow()).toHaveTextContent('Not installed');
    });

    it('shows Ollama readiness with the installed model count', async () => {
        vi.mocked(isYoloAvailable).mockResolvedValue(false);
        vi.mocked(checkOllama).mockResolvedValue(['llava', 'minicpm-v']);

        render(AiSettings);

        await waitFor(() => expect(ollamaReadinessRow()).toHaveTextContent('Ready · 2 models installed'));
    });

    it('shows that Ollama is reachable but has no installed models', async () => {
        vi.mocked(isYoloAvailable).mockResolvedValue(false);
        vi.mocked(checkOllama).mockResolvedValue([]);

        render(AiSettings);

        await waitFor(() => expect(ollamaReadinessRow()).toHaveTextContent('No models installed'));
    });

    it('shows Service unavailable when the Ollama check fails', async () => {
        vi.mocked(isYoloAvailable).mockResolvedValue(false);
        vi.mocked(checkOllama).mockRejectedValue(new Error('connection refused'));

        render(AiSettings);

        await waitFor(() => expect(ollamaReadinessRow()).toHaveTextContent('Service unavailable'));
    });

    it('explains that local detection weights are user-supplied and separately licensed', async () => {
        vi.mocked(isYoloAvailable).mockResolvedValue(false);

        render(AiSettings);

        expect(await screen.findByText('YOLO and NudeNet weights are user-supplied and separately licensed.')).toBeVisible();
    });

    it('gives every provider key field an accessible name', async () => {
        vi.mocked(isYoloAvailable).mockResolvedValue(false);
        render(AiSettings);

        for (const provider of ['OpenAI', 'Google', 'Cohere', 'OpenRouter']) {
            expect(await screen.findByLabelText(`${provider} API key`)).toHaveAttribute('type', 'password');
        }
    });
});
