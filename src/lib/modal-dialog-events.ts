export interface ModalKeyboardOptions {
    closeOnEscape: boolean;
    trapFocus: boolean;
    panelElement: HTMLElement | null;
    findFocusableWithin: (root: ParentNode) => HTMLElement[];
    activeElement: Element | null;
    onclose: () => void;
}

export function handleModalOverlayClick(event: Pick<MouseEvent, 'stopPropagation'>, onclose: () => void) {
    event.stopPropagation();
    onclose();
}

export function handleModalKeydown(event: KeyboardEvent, options: ModalKeyboardOptions) {
    event.stopPropagation();

    if (options.closeOnEscape && event.key === 'Escape') {
        event.preventDefault();
        options.onclose();
        return;
    }

    if (!options.trapFocus || event.key !== 'Tab' || !options.panelElement) return;

    const focusables = options.findFocusableWithin(options.panelElement);
    if (!focusables.length) {
        event.preventDefault();
        options.panelElement.focus();
        return;
    }

    const currentIndex = focusables.indexOf(options.activeElement as HTMLElement);
    event.preventDefault();

    if (event.shiftKey) {
        const previous = currentIndex <= 0 ? focusables[focusables.length - 1] : focusables[currentIndex - 1];
        previous.focus();
        return;
    }

    const next = currentIndex === focusables.length - 1 || currentIndex < 0 ? focusables[0] : focusables[currentIndex + 1];
    next.focus();
}
