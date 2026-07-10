// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Harness from './ModalDialog.test-harness.svelte';

afterEach(() => cleanup());

describe('ModalDialog rendered accessibility behavior', () => {
    it('exposes a named and described modal and isolates the app shell', async () => {
        render(Harness, { onclose: vi.fn() });
        const dialog = screen.getByRole('dialog', { name: 'Accessible test dialog' });
        expect(dialog).toHaveAttribute('aria-modal', 'true');
        expect(dialog).toHaveAccessibleDescription('Modal behavior under test');
        await waitFor(() => expect(screen.getByRole('button', { name: 'First action' })).toHaveFocus());
        expect(screen.getByText('Background action').parentElement)
            .toHaveAttribute('aria-hidden', 'true');
    });

    it('wraps Tab and Shift+Tab inside the modal', async () => {
        const user = userEvent.setup();
        render(Harness, { onclose: vi.fn() });
        const first = screen.getByRole('button', { name: 'First action' });
        const last = screen.getByRole('button', { name: 'Last action' });
        await waitFor(() => expect(first).toHaveFocus());
        last.focus();
        await user.tab();
        expect(first).toHaveFocus();
        await user.tab({ shift: true });
        expect(last).toHaveFocus();
    });

    it('closes once on Escape and restores focus after unmount', async () => {
        const user = userEvent.setup();
        const opener = document.createElement('button');
        document.body.append(opener);
        opener.focus();
        const onclose = vi.fn();
        const view = render(Harness, { onclose });
        await user.keyboard('{Escape}');
        expect(onclose).toHaveBeenCalledOnce();
        view.unmount();
        expect(opener).toHaveFocus();
        opener.remove();
    });

    it('closes from the overlay but not from dialog content', async () => {
        const onclose = vi.fn();
        render(Harness, { onclose });
        await fireEvent.click(screen.getByRole('dialog'));
        expect(onclose).not.toHaveBeenCalled();
        await fireEvent.click(screen.getByRole('presentation'));
        expect(onclose).toHaveBeenCalledOnce();
    });
});
