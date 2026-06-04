<script lang="ts">
    import { onMount, tick } from 'svelte';
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
            .filter(isFocusable)
            .filter(node => node.closest('[tabindex="-1"]') === null);
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
        if (closeOnEscape && event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            onclose();
            return;
        }

        if (!trapFocus || event.key !== 'Tab' || !panelElement) return;

        const focusables = findFocusableWithin(panelElement);
        if (!focusables.length) {
            event.preventDefault();
            panelElement.focus();
            return;
        }

        const active = document.activeElement;
        const currentIndex = active instanceof HTMLElement ? focusables.indexOf(active) : -1;
        event.preventDefault();
        event.stopPropagation();

        if (event.shiftKey) {
            const previous = currentIndex <= 0 ? focusables[focusables.length - 1] : focusables[currentIndex - 1];
            previous.focus();
            return;
        }

        const next = currentIndex === focusables.length - 1 || currentIndex < 0 ? focusables[0] : focusables[currentIndex + 1];
        next.focus();
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
    onclick={onclose}
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
    >
        <slot />
    </div>
</div>

<style>
    .modal-overlay {
        position: fixed;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1200;
    }

    .modal-panel {
        outline: none;
    }
</style>
