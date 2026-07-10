<script lang="ts">
    import { onMount, tick, type Snippet } from 'svelte';
    import { handleModalKeydown, handleModalOverlayClick } from '$lib/modal-dialog-events';
    import { claimModalShellInert, releaseModalShellInert } from '$lib/modal-layer';

    type InitialFocusTarget =
        | HTMLElement
        | null
        | string
        | (() => HTMLElement | null)
        | undefined;

    interface Props {
        titleId: string;
        descriptionId?: string;
        onclose: () => void;
        closeOnEscape?: boolean;
        trapFocus?: boolean;
        inertBackground?: boolean;
        restoreFocus?: HTMLElement | null;
        initialFocus?: InitialFocusTarget;
        overlayClass?: string;
        panelClass?: string;
        ariaLabel?: string;
        children?: Snippet;
    }

    let {
        titleId,
        descriptionId,
        onclose,
        closeOnEscape = true,
        trapFocus = true,
        inertBackground = true,
        restoreFocus = null,
        initialFocus,
        overlayClass = '',
        panelClass = '',
        ariaLabel,
        children,
    }: Props = $props();

    const FOCUSABLE_SELECTOR = [
        'a[href]',
        'area[href]',
        'button:not(:disabled)',
        'input:not(:disabled)',
        'select:not(:disabled)',
        'textarea:not(:disabled)',
        'summary',
        '[tabindex]',
    ].join(',');

    let overlayElement = $state<HTMLDivElement | null>(null);
    let panelElement = $state<HTMLDivElement | null>(null);
    let opener = $state<HTMLElement | null>(null);

    function isFocusable(node: Element | null): node is HTMLElement {
        if (!node || !(node instanceof HTMLElement)) return false;
        if (node.tabIndex < 0) return false;
        if (node.getAttribute('aria-disabled') === 'true') return false;
        if (node.hidden) return false;
        return true;
    }

    function findFocusableWithin(root: ParentNode): HTMLElement[] {
        return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
            .filter(isFocusable);
    }

    function resolveInitialFocus(): HTMLElement | null {
        if (!panelElement) return null;

        if (initialFocus) {
            if (typeof initialFocus === 'function') {
                return initialFocus();
            }

            if (typeof initialFocus === 'string') {
                return panelElement.querySelector(initialFocus) as HTMLElement | null;
            }

            return initialFocus;
        }

        const explicit = panelElement.querySelector('[data-modal-initial-focus]') as HTMLElement | null;
        if (explicit) return explicit;

        const candidates = findFocusableWithin(panelElement);
        return candidates[0] ?? panelElement;
    }

    function restoreFocusTarget() {
        const target = restoreFocus ?? opener;
        if (!target || !target.isConnected) return;
        target.focus({ preventScroll: true });
    }

    function handleKeydown(event: KeyboardEvent) {
        handleModalKeydown(event, {
            closeOnEscape,
            trapFocus,
            panelElement,
            findFocusableWithin,
            activeElement: document.activeElement,
            onclose,
        });
    }

    function handleOverlayClick(event: MouseEvent) {
        handleModalOverlayClick(event, onclose);
    }

    onMount(() => {
        opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        if (inertBackground) claimModalShellInert();

        tick().then(() => {
            const target = resolveInitialFocus();
            target?.focus({ preventScroll: true });
        });

        return () => {
            if (inertBackground) releaseModalShellInert();
            restoreFocusTarget();
        };
    });
</script>

<div
    class={`modal-overlay ${overlayClass}`}
    bind:this={overlayElement}
    role="presentation"
    tabindex="-1"
    onkeydown={handleKeydown}
    onclick={handleOverlayClick}
>
    <div
        bind:this={panelElement}
        class={`modal-panel ${panelClass}`}
        role="dialog"
        aria-modal="true"
        aria-label={ariaLabel}
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        tabindex="-1"
        onclick={(event: MouseEvent) => event.stopPropagation()}
        onkeydown={handleKeydown}
    >
        {@render children?.()}
    </div>
</div>

<style>
    .modal-overlay {
        position: fixed;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: var(--z-modal);
    }

    .modal-panel {
        outline: none;
    }
</style>
