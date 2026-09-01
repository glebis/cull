// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Harness from './ActionMenu.test-harness.svelte';

afterEach(() => cleanup());

function renderMenu() {
    const opener = document.createElement('button');
    opener.textContent = 'External opener';
    document.body.append(opener);
    opener.focus();
    const handlers = {
        onclose: vi.fn(),
        onopen: vi.fn(),
        onalpha: vi.fn(),
        onbeta: vi.fn(),
        ondelete: vi.fn(),
        opener,
        targetKey: 'initial',
    };
    const view = render(Harness, handlers);
    return { handlers, opener, view };
}

describe('ActionMenu rendered behavior', () => {
    it('focuses the first action and wraps through enabled root actions', async () => {
        const user = userEvent.setup();
        renderMenu();

        const open = await screen.findByRole('menuitem', { name: 'Open' });
        await waitFor(() => expect(open).toHaveFocus());
        await user.keyboard('{ArrowUp}');
        expect(screen.getByRole('menuitem', { name: 'Delete…' })).toHaveFocus();
        await user.keyboard('{ArrowDown}');
        expect(open).toHaveFocus();
    });

    it('enters and leaves a submenu without focusing disabled actions', async () => {
        const user = userEvent.setup();
        renderMenu();

        await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Open' })).toHaveFocus());
        await user.keyboard('{ArrowDown}{ArrowRight}');
        expect(screen.getByRole('menuitem', { name: 'Alpha' })).toHaveFocus();
        await user.keyboard('{ArrowDown}');
        expect(screen.getByRole('menuitem', { name: 'Beta' })).toHaveFocus();
        await user.keyboard('{ArrowLeft}');
        expect(screen.getByRole('menuitem', { name: /Add to Collection/ })).toHaveFocus();
    });

    it('dispatches the selected action and closes the menu', async () => {
        const user = userEvent.setup();
        const { handlers } = renderMenu();

        await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Open' })).toHaveFocus());
        await user.keyboard('{ArrowDown}{ArrowRight}{ArrowDown}{Enter}');
        expect(handlers.onbeta).toHaveBeenCalledOnce();
        expect(handlers.onclose).toHaveBeenCalledOnce();
    });

    it('closes on Escape or outside click and restores opener focus after unmount', async () => {
        const user = userEvent.setup();
        const first = renderMenu();
        await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Open' })).toHaveFocus());
        await user.keyboard('{Escape}');
        expect(first.handlers.onclose).toHaveBeenCalledOnce();
        first.view.unmount();
        expect(first.opener).toHaveFocus();
        first.opener.remove();

        cleanup();
        const second = renderMenu();
        await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Open' })).toHaveFocus());
        await fireEvent.click(document.body);
        expect(second.handlers.onclose).toHaveBeenCalledOnce();
        second.view.unmount();
        second.opener.remove();
    });

    it('closes on Escape before asynchronous placement moves focus into the menu', async () => {
        const { handlers, opener, view } = renderMenu();
        await fireEvent.keyDown(window, { key: 'Escape' });
        expect(handlers.onclose).toHaveBeenCalledOnce();
        view.unmount();
        opener.remove();
    });

    it('does not close when a handled contextmenu event retargets the active menu', async () => {
        const { handlers, opener, view } = renderMenu();
        await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Open' })).toHaveFocus());
        const nextOpener = document.createElement('button');
        nextOpener.textContent = 'Next opener';
        document.body.append(nextOpener);
        const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
        event.preventDefault();
        nextOpener.dispatchEvent(event);
        expect(handlers.onclose).not.toHaveBeenCalled();
        await view.rerender({ ...handlers, opener: nextOpener, targetKey: 'next' });
        await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Open' })).toHaveFocus());
        view.unmount();
        expect(nextOpener).toHaveFocus();
        nextOpener.remove();
        opener.remove();
    });
});
