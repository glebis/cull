<script module lang="ts">
    export interface ActionMenuItem {
        id: string;
        label: string;
        action?: () => void | Promise<void>;
        children?: ActionMenuItem[];
        disabled?: boolean;
        danger?: boolean;
        hidden?: boolean;
        separatorBefore?: boolean;
    }
</script>

<script lang="ts">
    import { onMount, tick } from 'svelte';
    import { clampFloatingPosition, placeAdjacentSubmenu } from '$lib/floating-position';

    interface Props {
        title?: string;
        x: number;
        y: number;
        items: ActionMenuItem[];
        onclose: () => void;
    }

    let { title, x, y, items, onclose }: Props = $props();

    const opener = typeof document !== 'undefined'
        && document.activeElement instanceof HTMLElement
        && document.activeElement !== document.body
        ? document.activeElement
        : null;

    let menuEl = $state<HTMLDivElement>();
    let submenuEl = $state<HTMLDivElement>();
    let menuX = $state(0);
    let menuY = $state(0);
    let menuReady = $state(false);
    let openSubmenuId = $state<string | null>(null);
    let submenuPlacement = $state('');
    let initialFocusSet = false;
    let focusRequest = 0;
    let placementRequest = 0;

    let visibleItems = $derived(items.filter(item => !item.hidden));

    function rootItems(): HTMLButtonElement[] {
        return menuEl
            ? Array.from(menuEl.querySelectorAll<HTMLButtonElement>('button[data-action-root]:not(:disabled)'))
            : [];
    }

    function submenuItems(): HTMLButtonElement[] {
        return submenuEl
            ? Array.from(submenuEl.querySelectorAll<HTMLButtonElement>('button[data-action-child]:not(:disabled)'))
            : [];
    }

    function focusedIndex(elements: HTMLElement[]): number {
        const index = elements.indexOf(document.activeElement as HTMLElement);
        return index < 0 ? 0 : index;
    }

    async function requestFocus(scope: 'root' | 'submenu', index: number) {
        const request = ++focusRequest;
        await tick();
        if (request !== focusRequest) return;
        const elements = scope === 'root' ? rootItems() : submenuItems();
        if (elements.length === 0) return;
        const wrapped = ((index % elements.length) + elements.length) % elements.length;
        elements[wrapped]?.focus({ preventScroll: true });
    }

    async function placeMenu() {
        const request = ++placementRequest;
        menuReady = false;
        menuX = x;
        menuY = y;
        await tick();
        if (request !== placementRequest || !menuEl) return;
        const rect = menuEl.getBoundingClientRect();
        const next = clampFloatingPosition(
            { x, y },
            { width: rect.width, height: rect.height },
            { width: window.innerWidth, height: window.innerHeight },
        );
        menuX = next.x;
        menuY = next.y;
        menuReady = true;
        if (!initialFocusSet) {
            initialFocusSet = true;
            await requestFocus('root', 0);
        }
    }

    async function placeSubmenu() {
        await tick();
        if (!submenuEl) return;
        const parent = submenuEl.closest<HTMLElement>('.submenu-parent');
        if (!parent) return;
        const parentRect = parent.getBoundingClientRect();
        const submenuRect = submenuEl.getBoundingClientRect();
        const placement = placeAdjacentSubmenu(
            {
                x: parentRect.left,
                y: parentRect.top,
                width: parentRect.width,
                height: parentRect.height,
            },
            { width: submenuRect.width, height: submenuRect.height },
            { width: window.innerWidth, height: window.innerHeight },
            460,
        );
        submenuPlacement = [
            `--submenu-left: ${placement.left}px`,
            `--submenu-top: ${placement.top}px`,
            `--submenu-max-height: ${placement.maxHeight}px`,
        ].join('; ');
    }

    $effect(() => {
        if (!menuEl) return;
        void placeMenu();
    });

    $effect(() => {
        if (!openSubmenuId) return;
        submenuEl;
        void placeSubmenu();
    });

    function restoreOpenerFocus() {
        if (!opener?.isConnected) return;
        const active = document.activeElement;
        const owned = !!menuEl && active instanceof Node && menuEl.contains(active);
        if (active !== document.body && active !== menuEl && !owned) return;
        opener.focus({ preventScroll: true });
    }

    onMount(() => {
        function closeFromOutside(event: MouseEvent) {
            if (menuEl && !menuEl.contains(event.target as Node)) onclose();
        }
        function reposition() {
            void placeMenu();
            void placeSubmenu();
        }
        const listenerTimer = window.setTimeout(() => {
            window.addEventListener('click', closeFromOutside);
            window.addEventListener('contextmenu', closeFromOutside);
        });
        window.addEventListener('resize', reposition);
        return () => {
            window.clearTimeout(listenerTimer);
            window.removeEventListener('click', closeFromOutside);
            window.removeEventListener('contextmenu', closeFromOutside);
            window.removeEventListener('resize', reposition);
            focusRequest += 1;
            restoreOpenerFocus();
        };
    });

    async function openSubmenu(item: ActionMenuItem, focusFirst = false) {
        if (!item.children?.some(child => !child.hidden && !child.disabled)) return;
        openSubmenuId = item.id;
        await placeSubmenu();
        if (focusFirst) await requestFocus('submenu', 0);
    }

    async function activate(item: ActionMenuItem) {
        if (item.disabled) return;
        if (item.children?.length) {
            await openSubmenu(item, true);
            return;
        }
        onclose();
        await item.action?.();
    }

    function parentRootIndex(): number {
        const roots = rootItems();
        const parent = roots.findIndex(button => button.dataset.actionId === openSubmenuId);
        return parent < 0 ? 0 : parent;
    }

    async function closeSubmenuAndRestoreParent() {
        const index = parentRootIndex();
        openSubmenuId = null;
        submenuPlacement = '';
        await requestFocus('root', index);
    }

    function handleKeydown(event: KeyboardEvent) {
        const inSubmenu = !!submenuEl
            && event.target instanceof Node
            && submenuEl.contains(event.target);
        const elements = inSubmenu ? submenuItems() : rootItems();
        if (elements.length === 0) return;
        const index = focusedIndex(elements);

        if (event.key === 'ArrowDown') {
            event.preventDefault();
            void requestFocus(inSubmenu ? 'submenu' : 'root', index + 1);
        } else if (event.key === 'ArrowUp') {
            event.preventDefault();
            void requestFocus(inSubmenu ? 'submenu' : 'root', index - 1);
        } else if (event.key === 'Home') {
            event.preventDefault();
            void requestFocus(inSubmenu ? 'submenu' : 'root', 0);
        } else if (event.key === 'End') {
            event.preventDefault();
            void requestFocus(inSubmenu ? 'submenu' : 'root', elements.length - 1);
        } else if (event.key === 'ArrowRight' && !inSubmenu) {
            const item = visibleItems.find(candidate => candidate.id === elements[index]?.dataset.actionId);
            if (item?.children?.length) {
                event.preventDefault();
                void openSubmenu(item, true);
            }
        } else if (event.key === 'ArrowLeft' && inSubmenu) {
            event.preventDefault();
            void closeSubmenuAndRestoreParent();
        } else if (event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            if (inSubmenu || openSubmenuId) void closeSubmenuAndRestoreParent();
            else onclose();
        } else if ((event.key === 'Enter' || event.key === ' ') && event.target instanceof HTMLButtonElement) {
            event.preventDefault();
            event.target.click();
        }
    }
</script>

<div
    class="action-menu"
    style="left: {menuX}px; top: {menuY}px; visibility: {menuReady ? 'visible' : 'hidden'};"
    role="menu"
    tabindex="-1"
    bind:this={menuEl}
    onkeydown={handleKeydown}
>
    {#if title}<div class="menu-header">{title}</div>{/if}
    {#each visibleItems as item}
        {#if item.separatorBefore}<div class="separator" role="separator"></div>{/if}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
            class:submenu-parent={item.children?.length}
            onmouseenter={() => { if (item.children?.length) void openSubmenu(item); }}
            onmouseleave={() => { if (openSubmenuId === item.id && !(document.activeElement instanceof Node && submenuEl?.contains(document.activeElement))) openSubmenuId = null; }}
        >
            <button
                type="button"
                class="action-menu-item"
                class:danger={item.danger}
                class:has-submenu={item.children?.length}
                role="menuitem"
                data-action-root
                data-action-id={item.id}
                aria-haspopup={item.children?.length ? 'menu' : undefined}
                aria-expanded={item.children?.length ? openSubmenuId === item.id : undefined}
                disabled={item.disabled}
                tabindex="-1"
                onclick={() => activate(item)}
            >
                <span>{item.label}</span>
                {#if item.children?.length}<span class="arrow" aria-hidden="true">▸</span>{/if}
            </button>
            {#if item.children?.length && openSubmenuId === item.id}
                <div
                    class="action-submenu"
                    role="menu"
                    bind:this={submenuEl}
                    style={submenuPlacement}
                >
                    {#each item.children.filter(child => !child.hidden) as child}
                        {#if child.separatorBefore}<div class="separator" role="separator"></div>{/if}
                        <button
                            type="button"
                            class="action-menu-item"
                            class:danger={child.danger}
                            role="menuitem"
                            data-action-child
                            disabled={child.disabled}
                            tabindex="-1"
                            onclick={() => activate(child)}
                        >{child.label}</button>
                    {/each}
                </div>
            {/if}
        </div>
    {/each}
</div>

<style>
    .action-menu,
    .action-submenu {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 12px 32px color-mix(in srgb, var(--bg) 80%, transparent);
        min-width: 220px;
        padding: 4px;
        z-index: var(--z-context-menu);
    }
    .action-menu {
        position: fixed;
    }
    .submenu-parent {
        position: relative;
    }
    .action-submenu {
        left: var(--submenu-left, calc(100% - 1px));
        max-height: var(--submenu-max-height, 460px);
        overflow-y: auto;
        position: absolute;
        top: var(--submenu-top, -4px);
    }
    .menu-header {
        color: var(--text-secondary);
        font-size: 10px;
        overflow: hidden;
        padding: 6px 8px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .action-menu-item {
        align-items: center;
        background: none;
        border: none;
        border-radius: var(--radius);
        color: var(--text);
        cursor: pointer;
        display: flex;
        font-family: inherit;
        font-size: 12px;
        gap: 16px;
        justify-content: space-between;
        min-height: 28px;
        padding: 6px 8px;
        text-align: left;
        width: 100%;
    }
    .action-menu-item:hover:not(:disabled),
    .action-menu-item:focus-visible {
        background: var(--border);
        outline: none;
    }
    .action-menu-item:disabled {
        color: var(--text-secondary);
        cursor: default;
        opacity: 0.55;
    }
    .action-menu-item.danger {
        color: var(--red);
    }
    .arrow {
        color: var(--text-secondary);
        margin-left: auto;
    }
    .separator {
        background: var(--border);
        height: 1px;
        margin: 4px;
    }
</style>
