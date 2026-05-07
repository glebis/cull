import { get } from 'svelte/store';
import { images, selectedIds, focusedIndex, thumbnailSize, statusHint } from './stores';
import { setRating, setDecision } from './api';

let waitingForStar = false;

function getColCount(): number {
    // Approximate cols from container width and thumbnail size.
    // We read from the DOM to stay in sync with Grid.
    const container = document.querySelector('.grid-container');
    if (!container) return 4;
    const size = get(thumbnailSize);
    const gap = 4;
    return Math.max(1, Math.floor((container.clientWidth + gap) / (size + gap)));
}

function moveFocus(delta: number) {
    const total = get(images).length;
    if (total === 0) return;
    focusedIndex.update(i => {
        let next = i + delta;
        if (next < 0) next = 0;
        if (next >= total) next = total - 1;
        return next;
    });
    scrollFocusedIntoView();
}

function scrollFocusedIntoView() {
    requestAnimationFrame(() => {
        const container = document.querySelector('.grid-container');
        if (!container) return;
        const idx = get(focusedIndex);
        const size = get(thumbnailSize);
        const gap = 4;
        const cols = getColCount();
        const cellSize = size + gap;
        const row = Math.floor(idx / cols);
        const itemTop = row * cellSize;
        const itemBottom = itemTop + cellSize;

        if (itemTop < container.scrollTop) {
            container.scrollTop = itemTop;
        } else if (itemBottom > container.scrollTop + container.clientHeight) {
            container.scrollTop = itemBottom - container.clientHeight;
        }
    });
}

function toggleSelect() {
    const imgs = get(images);
    const idx = get(focusedIndex);
    const img = imgs[idx];
    if (!img) return;
    selectedIds.update(ids => {
        const next = new Set(ids);
        if (next.has(img.image.id)) {
            next.delete(img.image.id);
        } else {
            next.add(img.image.id);
        }
        return next;
    });
}

async function handleStarRating(n: number) {
    const imgs = get(images);
    const idx = get(focusedIndex);
    const img = imgs[idx];
    if (!img) return;
    try {
        await setRating(img.image.id, n);
        // Update local state
        images.update(all => {
            const copy = [...all];
            const item = { ...copy[idx] };
            item.selection = {
                image_id: img.image.id,
                project_id: item.selection?.project_id ?? null,
                star_rating: n,
                color_label: item.selection?.color_label ?? null,
                decision: item.selection?.decision ?? 'undecided',
            };
            copy[idx] = item;
            return copy;
        });
    } catch (e) {
        console.error('Failed to set rating:', e);
    }
}

async function handleDecision(decision: string) {
    const imgs = get(images);
    const idx = get(focusedIndex);
    const img = imgs[idx];
    if (!img) return;
    try {
        await setDecision(img.image.id, decision);
        images.update(all => {
            const copy = [...all];
            const item = { ...copy[idx] };
            item.selection = {
                image_id: img.image.id,
                project_id: item.selection?.project_id ?? null,
                star_rating: item.selection?.star_rating ?? null,
                color_label: item.selection?.color_label ?? null,
                decision,
            };
            copy[idx] = item;
            return copy;
        });
    } catch (e) {
        console.error('Failed to set decision:', e);
    }
}

function handleResize(delta: number) {
    thumbnailSize.update(s => {
        const next = s + delta;
        if (next < 80) return 80;
        if (next > 400) return 400;
        return next;
    });
}

export function handleKeydown(e: KeyboardEvent) {
    // Ignore if user is typing in an input
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

    // Let native behavior happen for interactive elements (Space on buttons, etc.)
    const tag = (e.target as HTMLElement)?.tagName;
    if (['BUTTON', 'A', 'SELECT'].includes(tag) && (e.key === ' ' || e.key === 'Enter')) return;

    if (waitingForStar) {
        if (e.key === 'Escape') {
            waitingForStar = false;
            statusHint.set(null);
            e.preventDefault();
            return;
        }
        waitingForStar = false;
        statusHint.set(null);
        const n = parseInt(e.key);
        if (n >= 1 && n <= 5) {
            e.preventDefault();
            handleStarRating(n);
            return;
        }
    }

    const cols = getColCount();
    const total = get(images).length;
    const visibleRows = Math.max(1, Math.floor(
        (document.querySelector('.grid-container')?.clientHeight ?? 600) / (get(thumbnailSize) + 4)
    ));

    switch (e.key) {
        case 'h':
        case 'ArrowLeft':
            e.preventDefault();
            moveFocus(-1);
            break;
        case 'l':
        case 'ArrowRight':
            e.preventDefault();
            moveFocus(1);
            break;
        case 'k':
        case 'ArrowUp':
            e.preventDefault();
            moveFocus(-cols);
            break;
        case 'j':
        case 'ArrowDown':
            e.preventDefault();
            moveFocus(cols);
            break;
        case ' ':
            e.preventDefault();
            toggleSelect();
            break;
        case 's':
            e.preventDefault();
            waitingForStar = true;
            statusHint.set('Rate: press 1-5');
            break;
        case '0':
            e.preventDefault();
            handleStarRating(0);
            break;
        case 'a':
            e.preventDefault();
            handleDecision('accept');
            break;
        case 'x':
            e.preventDefault();
            handleDecision('reject');
            break;
        case 'u':
            e.preventDefault();
            handleDecision('undecided');
            break;
        case '+':
        case '=':
            e.preventDefault();
            handleResize(16);
            break;
        case '-':
            e.preventDefault();
            handleResize(-16);
            break;
        case 'Home':
            e.preventDefault();
            focusedIndex.set(0);
            scrollFocusedIntoView();
            break;
        case 'End':
            e.preventDefault();
            if (total > 0) focusedIndex.set(total - 1);
            scrollFocusedIntoView();
            break;
        case 'PageUp':
            e.preventDefault();
            moveFocus(-cols * visibleRows);
            break;
        case 'PageDown':
            e.preventDefault();
            moveFocus(cols * visibleRows);
            break;
    }
}
