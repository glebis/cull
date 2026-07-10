import { describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { handleModalKeydown, handleModalOverlayClick } from './modal-dialog-events';

const modalDialog = readFileSync(join(process.cwd(), 'src/lib/components/ModalDialog.svelte'), 'utf8');

function keyEvent(key: string, shiftKey = false): KeyboardEvent {
    return {
        key,
        shiftKey,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
    } as unknown as KeyboardEvent;
}

function focusable(label: string): HTMLElement {
    return {
        label,
        focus: vi.fn(),
    } as unknown as HTMLElement;
}

describe('modal dialog accessibility contract', () => {
    it('delegates modal keyboard and overlay behavior to testable event helpers', () => {
        expect(modalDialog).toContain("import { handleModalKeydown, handleModalOverlayClick } from '$lib/modal-dialog-events';");
        expect(modalDialog).toContain('handleModalKeydown(event, {');
        expect(modalDialog).toContain('handleModalOverlayClick(event, onclose);');
        expect(modalDialog).toContain('onkeydown={handleKeydown}');
        expect(modalDialog).toContain('{@render children?.()}');
    });

    it('stops Escape propagation and closes only through the modal close callback', () => {
        const onclose = vi.fn();
        const event = keyEvent('Escape');

        handleModalKeydown(event, {
            closeOnEscape: true,
            trapFocus: true,
            panelElement: null,
            findFocusableWithin: () => [],
            activeElement: null,
            onclose,
        });

        expect(event.stopPropagation).toHaveBeenCalledOnce();
        expect(event.preventDefault).toHaveBeenCalledOnce();
        expect(onclose).toHaveBeenCalledOnce();
    });

    it('keeps Tab focus inside the active modal', () => {
        const first = focusable('first');
        const second = focusable('second');
        const panel = { focus: vi.fn() } as unknown as HTMLElement;
        const event = keyEvent('Tab');

        handleModalKeydown(event, {
            closeOnEscape: true,
            trapFocus: true,
            panelElement: panel,
            findFocusableWithin: () => [first, second],
            activeElement: second,
            onclose: vi.fn(),
        });

        expect(event.stopPropagation).toHaveBeenCalledOnce();
        expect(event.preventDefault).toHaveBeenCalledOnce();
        expect(first.focus).toHaveBeenCalledOnce();
    });

    it('wraps Shift+Tab to the last focusable control', () => {
        const first = focusable('first');
        const second = focusable('second');
        const event = keyEvent('Tab', true);

        handleModalKeydown(event, {
            closeOnEscape: true,
            trapFocus: true,
            panelElement: { focus: vi.fn() } as unknown as HTMLElement,
            findFocusableWithin: () => [first, second],
            activeElement: first,
            onclose: vi.fn(),
        });

        expect(event.preventDefault).toHaveBeenCalledOnce();
        expect(second.focus).toHaveBeenCalledOnce();
    });

    it('prevents nested overlay close clicks from bubbling to parent modals', () => {
        const event = { stopPropagation: vi.fn() } as unknown as MouseEvent;
        const onclose = vi.fn();

        handleModalOverlayClick(event, onclose);

        expect(event.stopPropagation).toHaveBeenCalledOnce();
        expect(onclose).toHaveBeenCalledOnce();
    });
});
