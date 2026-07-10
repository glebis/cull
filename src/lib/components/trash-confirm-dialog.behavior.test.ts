// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import TrashConfirmDialog from './TrashConfirmDialog.svelte';

afterEach(() => cleanup());

describe('TrashConfirmDialog rendered behavior', () => {
    it('intentionally focuses the destructive move action when opened', async () => {
        render(TrashConfirmDialog, {
            visible: true,
            fileName: 'portrait.png',
            onconfirm: vi.fn(),
            oncancel: vi.fn(),
        });

        await waitFor(() => expect(screen.getByRole('button', { name: 'Move to Trash' })).toHaveFocus());
    });

    it('does not confirm from bare Enter on the dialog itself', async () => {
        const user = userEvent.setup();
        const onconfirm = vi.fn();
        const oncancel = vi.fn();
        render(TrashConfirmDialog, { visible: true, fileName: 'portrait.png', onconfirm, oncancel });
        const dialog = screen.getByRole('dialog', { name: 'Move to Trash' });
        await waitFor(() => expect(screen.getByRole('button', { name: 'Move to Trash' })).toHaveFocus());
        dialog.focus();
        expect(dialog).toHaveFocus();

        await user.keyboard('{Enter}');

        expect(onconfirm).not.toHaveBeenCalled();
        expect(oncancel).not.toHaveBeenCalled();
    });

    it('cancels on Escape without confirming', async () => {
        const onconfirm = vi.fn();
        const oncancel = vi.fn();
        render(TrashConfirmDialog, { visible: true, fileName: 'portrait.png', onconfirm, oncancel });
        const dialog = screen.getByRole('dialog', { name: 'Move to Trash' });
        expect(dialog).toHaveAccessibleDescription('Move portrait.png to Trash?');

        await userEvent.setup().keyboard('{Escape}');

        expect(oncancel).toHaveBeenCalledOnce();
        expect(onconfirm).not.toHaveBeenCalled();
    });

    it('cancels from the Cancel button without confirming', async () => {
        const user = userEvent.setup();
        const onconfirm = vi.fn();
        const oncancel = vi.fn();
        render(TrashConfirmDialog, { visible: true, fileName: 'portrait.png', onconfirm, oncancel });

        await user.click(screen.getByRole('button', { name: 'Cancel' }));

        expect(oncancel).toHaveBeenCalledOnce();
        expect(onconfirm).not.toHaveBeenCalled();
    });

    it("confirms once without suppressing future prompts by default", async () => {
        const user = userEvent.setup();
        const onconfirm = vi.fn();
        render(TrashConfirmDialog, {
            visible: true,
            fileName: 'portrait.png',
            onconfirm,
            oncancel: vi.fn(),
        });

        await user.click(screen.getByRole('button', { name: 'Move to Trash' }));

        expect(onconfirm).toHaveBeenCalledOnce();
        expect(onconfirm).toHaveBeenCalledWith('none');
    });

    it("confirms with permanent suppression after selecting Don't ask again and Always", async () => {
        const user = userEvent.setup();
        const onconfirm = vi.fn();
        render(TrashConfirmDialog, {
            visible: true,
            fileName: 'portrait.png',
            onconfirm,
            oncancel: vi.fn(),
        });

        await user.click(screen.getByRole('checkbox', { name: "Don't ask again" }));
        await user.click(screen.getByRole('radio', { name: 'Always' }));
        await user.click(screen.getByRole('button', { name: 'Move to Trash' }));

        expect(onconfirm).toHaveBeenCalledOnce();
        expect(onconfirm).toHaveBeenCalledWith('always');
    });
});
