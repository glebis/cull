import { get } from 'svelte/store';
import {
    images, selectedIds, focusedIndex, thumbnailSize, statusHint, viewMode,
    compareActiveSide, loupeScale, loupePanX, loupePanY,
    sidebarVisible, gridPreset, gridGap, GRID_PRESETS, zenMode,
} from './stores';
import type { ViewMode } from './stores';
import { setRating, setDecision } from './api';

let waitingForStar = false;

const VIEW_MODE_KEYS: Record<string, ViewMode> = {
    '1': 'grid',
    '2': 'compare',
    '3': 'loupe',
    '4': 'canvas',
    '5': 'lineage',
    '6': 'embeddings',
    '7': 'export',
};

function getColCount(): number {
    const container = document.querySelector('.grid-container');
    if (!container) return 4;
    const size = get(thumbnailSize);
    const gap = get(gridGap);
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
        const gap = get(gridGap);
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

export async function handleStarRating(n: number, imageIndex?: number) {
    const imgs = get(images);
    const idx = imageIndex ?? get(focusedIndex);
    const img = imgs[idx];
    if (!img) return;
    try {
        await setRating(img.image.id, n);
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

export async function handleDecision(decision: string, imageIndex?: number) {
    const imgs = get(images);
    const idx = imageIndex ?? get(focusedIndex);
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

// ---- Compare helpers ----

function getCompareActiveIndex(): number {
    const imgs = get(images);
    const sel = get(selectedIds);
    const idx = get(focusedIndex);
    const side = get(compareActiveSide);

    if (sel.size >= 2) {
        const selArr = Array.from(sel);
        const targetId = selArr[side] ?? selArr[0];
        const found = imgs.findIndex(i => i.image.id === targetId);
        return found >= 0 ? found : idx;
    }
    return idx + side;
}

function compareNextPair() {
    const imgs = get(images);
    const idx = get(focusedIndex);
    const next = Math.min(idx + 2, Math.max(0, imgs.length - 2));
    focusedIndex.set(next);
    selectedIds.set(new Set());
    compareActiveSide.set(0);
}

function comparePrevPair() {
    const idx = get(focusedIndex);
    const prev = Math.max(0, idx - 2);
    focusedIndex.set(prev);
    selectedIds.set(new Set());
    compareActiveSide.set(0);
}

// ---- Loupe helpers ----

function resetLoupeZoom() {
    loupeScale.set(1);
    loupePanX.set(0);
    loupePanY.set(0);
}

function moveLoupeFocus(delta: number) {
    const total = get(images).length;
    if (total === 0) return;
    focusedIndex.update(i => {
        let next = i + delta;
        if (next < 0) next = 0;
        if (next >= total) next = total - 1;
        return next;
    });
    resetLoupeZoom();
}

// ---- Main handler ----

export function handleKeydown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

    const tag = (e.target as HTMLElement)?.tagName;
    if (['BUTTON', 'A', 'SELECT'].includes(tag) && (e.key === ' ' || e.key === 'Enter')) return;

    const mode = get(viewMode);

    // Star rating chord (works in all modes)
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
            if (mode === 'compare') {
                handleStarRating(n, getCompareActiveIndex());
            } else {
                handleStarRating(n);
            }
            return;
        }
    }

    // Shift+. (>) toggles zen mode
    if (e.key === '>' || (e.shiftKey && e.key === '.')) {
        e.preventDefault();
        zenMode.update(v => !v);
        return;
    }

    // Escape exits zen mode (in addition to other escape behaviors)
    if (e.key === 'Escape' && get(zenMode)) {
        e.preventDefault();
        zenMode.set(false);
        return;
    }

    // Cmd+B toggles sidebar
    if (e.metaKey && e.key === 'b') {
        e.preventDefault();
        sidebarVisible.update(v => !v);
        return;
    }

    // View mode switching with number keys (1-7)
    if (VIEW_MODE_KEYS[e.key] && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        viewMode.set(VIEW_MODE_KEYS[e.key]);
        return;
    }

    switch (mode) {
        case 'grid':
            handleGridKeys(e);
            break;
        case 'compare':
            handleCompareKeys(e);
            break;
        case 'loupe':
            handleLoupeKeys(e);
            break;
    }
}

function handleGridKeys(e: KeyboardEvent) {
    const cols = getColCount();
    const total = get(images).length;
    const visibleRows = Math.max(1, Math.floor(
        (document.querySelector('.grid-container')?.clientHeight ?? 600) / (get(thumbnailSize) + get(gridGap))
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
        case 'Enter':
            e.preventDefault();
            viewMode.set('loupe');
            break;
        case '\\':
            e.preventDefault();
            sidebarVisible.update(v => !v);
            break;
        case 'g':
            e.preventDefault();
            gridPreset.update(p => {
                const next = (p + 1) % GRID_PRESETS.length;
                const preset = GRID_PRESETS[next];
                thumbnailSize.set(preset.size);
                gridGap.set(preset.gap);
                return next;
            });
            break;
    }
}

function handleCompareKeys(e: KeyboardEvent) {
    switch (e.key) {
        case 'h':
        case 'ArrowLeft':
            e.preventDefault();
            compareActiveSide.set(0);
            break;
        case 'l':
        case 'ArrowRight':
            e.preventDefault();
            compareActiveSide.set(1);
            break;
        case 'j':
        case 'ArrowDown':
            e.preventDefault();
            compareNextPair();
            break;
        case 'k':
        case 'ArrowUp':
            e.preventDefault();
            comparePrevPair();
            break;
        case 'Tab':
            e.preventDefault();
            compareNextPair();
            break;
        case 'Enter':
            e.preventDefault();
            handleDecision('accept', getCompareActiveIndex());
            break;
        case 'x':
            e.preventDefault();
            handleDecision('reject', getCompareActiveIndex());
            break;
        case 'a':
            e.preventDefault();
            handleDecision('accept', getCompareActiveIndex());
            break;
        case 's':
            e.preventDefault();
            waitingForStar = true;
            statusHint.set('Rate: press 1-5');
            break;
        case '0':
            e.preventDefault();
            handleStarRating(0, getCompareActiveIndex());
            break;
        case 'u':
            e.preventDefault();
            handleDecision('undecided', getCompareActiveIndex());
            break;
        case 'Escape':
            e.preventDefault();
            viewMode.set('grid');
            break;
    }
}

function handleLoupeKeys(e: KeyboardEvent) {
    switch (e.key) {
        case 'h':
        case 'ArrowLeft':
            e.preventDefault();
            moveLoupeFocus(-1);
            break;
        case 'l':
        case 'ArrowRight':
            e.preventDefault();
            moveLoupeFocus(1);
            break;
        case 'k':
        case 'ArrowUp':
            e.preventDefault();
            moveLoupeFocus(-1);
            break;
        case 'j':
        case 'ArrowDown':
            e.preventDefault();
            moveLoupeFocus(1);
            break;
        case '+':
        case '=':
            e.preventDefault();
            loupeScale.update(s => Math.min(20, s * 1.25));
            break;
        case '-':
            e.preventDefault();
            loupeScale.update(s => {
                const next = Math.max(0.1, s / 1.25);
                if (next <= 1) { loupePanX.set(0); loupePanY.set(0); }
                return next;
            });
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
        case 'Escape':
            e.preventDefault();
            viewMode.set('grid');
            break;
        case 'Home':
            e.preventDefault();
            resetLoupeZoom();
            break;
        case 'f':
            e.preventDefault();
            toggleFullscreen();
            break;
    }
}

function toggleFullscreen() {
    if (document.fullscreenElement) {
        document.exitFullscreen();
    } else {
        document.documentElement.requestFullscreen();
    }
}
