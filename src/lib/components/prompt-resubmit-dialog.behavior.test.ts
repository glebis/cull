// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PromptResubmitDialog from './PromptResubmitDialog.svelte';

const apiMocks = vi.hoisted(() => ({
    resubmitPrompt: vi.fn().mockResolvedValue({ job_id: 'job-1' }),
}));

vi.mock('$lib/api', () => ({
    estimateGenerationCost: vi.fn().mockResolvedValue({ estimated_cost: 0.04 }),
    resubmitPrompt: apiMocks.resubmitPrompt,
}));

afterEach(() => cleanup());
beforeEach(() => apiMocks.resubmitPrompt.mockClear());

function props(onclose = vi.fn()) {
    return {
        visible: true,
        initialPrompt: 'A lighthouse in fog',
        sourceImageId: null,
        onclose,
        ongenerated: vi.fn(),
    };
}

describe('PromptResubmitDialog rendered accessibility behavior', () => {
    it('exposes a named modal and a labeled close button', async () => {
        render(PromptResubmitDialog, props());

        const dialog = screen.getByRole('dialog', { name: 'Re-generate' });
        expect(dialog).toHaveAttribute('aria-modal', 'true');
        expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument();
    });

    it('wraps Tab and Shift+Tab inside the dialog', async () => {
        const user = userEvent.setup();
        render(PromptResubmitDialog, props());
        const dialog = screen.getByRole('dialog', { name: 'Re-generate' });
        const first = screen.getByRole('button', { name: 'Close' });
        await waitFor(() => expect(first).toHaveFocus());

        for (let index = 0; index < 20; index += 1) {
            await user.tab();
            expect(dialog).toContainElement(document.activeElement as HTMLElement);
        }
        for (let index = 0; index < 20; index += 1) {
            await user.tab({ shift: true });
            expect(dialog).toContainElement(document.activeElement as HTMLElement);
        }
    });

    it('retains the Cmd+Enter generation shortcut inside the modal', async () => {
        render(PromptResubmitDialog, props());
        const prompt = screen.getByRole('textbox', { name: 'Prompt' });
        await waitFor(() => expect(prompt).toHaveValue('A lighthouse in fog'));

        await fireEvent.keyDown(prompt, { key: 'Enter', metaKey: true });

        await waitFor(() => expect(apiMocks.resubmitPrompt).toHaveBeenCalledOnce());
        expect(apiMocks.resubmitPrompt).toHaveBeenCalledWith(expect.objectContaining({
            prompt: 'A lighthouse in fog',
        }));
    });

    it('closes once on Escape and restores focus to the opener', async () => {
        const user = userEvent.setup();
        const opener = document.createElement('button');
        document.body.append(opener);
        opener.focus();
        const onclose = vi.fn();
        const view = render(PromptResubmitDialog, props(onclose));
        await waitFor(() => expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus());

        await user.keyboard('{Escape}');
        expect(onclose).toHaveBeenCalledOnce();

        await view.rerender({ ...props(onclose), visible: false });
        expect(opener).toHaveFocus();
        opener.remove();
    });
});
