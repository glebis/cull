import { get } from 'svelte/store';
import {
    images, selectedIds, focusedIndex, thumbnailSize, statusHint, viewMode,
    compareActiveSide, compareImages, loupeScale, loupePanX, loupePanY,
    sidebarVisible, gridPreset, gridGap, GRID_PRESETS, zenMode,
    collections, collectMode, collectModeTarget, activeCollection,
    showDetectionBoxes, showDetectionInspector, nsfwMode,
    navigateTo, navigateBack, searchOpen, focusedImage,
} from './stores';
import type { NsfwMode } from './stores';
import type { ViewMode } from './stores';
import { setRating, setDecision, createCollection, addToCollection, listCollections, listCollectionImages, rotateImage } from './api';

let waitingForStar = false;

const VIEW_MODE_KEYS: Record<string, ViewMode> = {
    '1': 'grid',
    '2': 'loupe',
    '3': 'compare',
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

function getCompareActiveIndexForSide(side: 0 | 1): number {
    const imgs = get(images);
    const sel = get(selectedIds);
    const idx = get(focusedIndex);

    if (sel.size >= 2) {
        const selArr = Array.from(sel);
        const targetId = selArr[side] ?? selArr[0];
        const found = imgs.findIndex(i => i.image.id === targetId);
        return found >= 0 ? found : idx;
    }
    return idx + side;
}

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

    // Search bar: / or Cmd+F opens search
    if (e.key === '/' && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        searchOpen.set(true);
        return;
    }
    if (e.key === 'f' && e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        searchOpen.set(true);
        return;
    }

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
        if (mode === 'loupe') {
            window.dispatchEvent(new CustomEvent('toggle-loupe-overlays'));
        }
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

    // Detection shortcuts (D, I, B)
    if (e.key === 'd' && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        showDetectionBoxes.update(v => !v);
        return;
    }
    if (e.key === 'i' && !e.metaKey && !e.ctrlKey && !e.altKey && (mode === 'loupe' || mode === 'compare')) {
        e.preventDefault();
        showDetectionInspector.update(v => !v);
        return;
    }
    if (e.key === 'b' && !e.metaKey && !e.ctrlKey && !e.altKey && mode !== 'grid') {
        e.preventDefault();
        const modes: NsfwMode[] = ['blur', 'hide', 'show'];
        nsfwMode.update(v => modes[(modes.indexOf(v) + 1) % modes.length]);
        return;
    }

    // View mode switching with Cmd+number (⌘1-7)
    if (VIEW_MODE_KEYS[e.key] && e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        navigateTo(VIEW_MODE_KEYS[e.key]);
        return;
    }

    // Delete: Backspace → trash, Cmd+Backspace → permanent delete
    if (e.key === 'Backspace' && !e.metaKey) {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('trash-focused-image'));
        return;
    }
    if (e.key === 'Backspace' && e.metaKey) {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('delete-focused-image'));
        return;
    }

    switch (mode) {
        case 'grid':
            handleGridKeys(e);
            break;
        case 'canvas':
            handleCanvasKeys(e);
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

    // Direct 1-5 rating in grid (without Cmd)
    if (!e.metaKey && !e.ctrlKey && !e.altKey) {
        const n = parseInt(e.key);
        if (n >= 1 && n <= 5) {
            e.preventDefault();
            handleStarRating(n);
            return;
        }
    }

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
            if (get(collectMode)) {
                handleCollectModeAdd();
            } else {
                toggleSelect();
            }
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
            navigateTo('loupe');
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
        case 'c':
            e.preventDefault();
            handleCreateCollectionFromSelected(false);
            break;
        case 'C':
            e.preventDefault();
            handleCreateCollectionFromSelected(true);
            break;
        case 'b':
            e.preventDefault();
            handleToggleCollectMode();
            break;
    }
}

async function handleCreateCollectionFromSelected(inverse: boolean) {
    const imgs = get(images);
    const sel = get(selectedIds);

    let imageIds: string[];
    if (inverse) {
        imageIds = imgs.filter(i => !sel.has(i.image.id)).map(i => i.image.id);
    } else {
        imageIds = imgs.filter(i => sel.has(i.image.id)).map(i => i.image.id);
    }

    if (imageIds.length === 0) {
        statusHint.set(inverse ? 'No unselected images' : 'Select images first');
        setTimeout(() => statusHint.set(null), 2000);
        return;
    }

    const name = window.prompt(`Collection name (${imageIds.length} images):`);
    if (!name || !name.trim()) return;

    try {
        const id = await createCollection(name.trim());
        await addToCollection(id, imageIds);
        const c = await listCollections();
        collections.set(c);
        statusHint.set(`Created "${name.trim()}" with ${imageIds.length} images`);
        setTimeout(() => statusHint.set(null), 2000);
    } catch (err) {
        console.error('Failed to create collection:', err);
    }
}

async function handleToggleCollectMode() {
    const current = get(collectMode);
    if (current) {
        // Exit collect mode
        collectMode.set(false);
        collectModeTarget.set(null);
        statusHint.set(null);
        return;
    }

    // Enter collect mode: pick or create a collection
    const cols = get(collections);
    let targetId: string | null = null;

    if (cols.length > 0) {
        const options = cols.map((c, i) => `${i + 1}. ${c[1]}`).join('\n');
        const choice = window.prompt(`Pick collection (number) or type a new name:\n${options}`);
        if (!choice || !choice.trim()) return;
        const num = parseInt(choice.trim());
        if (!isNaN(num) && num >= 1 && num <= cols.length) {
            targetId = cols[num - 1][0];
        } else {
            // Create new
            try {
                targetId = await createCollection(choice.trim());
                const c = await listCollections();
                collections.set(c);
            } catch (err) {
                console.error('Failed to create collection:', err);
                return;
            }
        }
    } else {
        const name = window.prompt('No collections yet. Name for new collection:');
        if (!name || !name.trim()) return;
        try {
            targetId = await createCollection(name.trim());
            const c = await listCollections();
            collections.set(c);
        } catch (err) {
            console.error('Failed to create collection:', err);
            return;
        }
    }

    collectMode.set(true);
    collectModeTarget.set(targetId);
    const colName = get(collections).find(c => c[0] === targetId)?.[1] ?? '';
    statusHint.set(`Collect mode: Space to add, B to exit [${colName}]`);
}

async function handleCollectModeAdd() {
    const target = get(collectModeTarget);
    if (!target) return;

    const imgs = get(images);
    const idx = get(focusedIndex);
    const img = imgs[idx];
    if (!img) return;

    try {
        await addToCollection(target, [img.image.id]);
        const c = await listCollections();
        collections.set(c);
        // If we're viewing this collection, refresh
        if (get(activeCollection) === target) {
            const updated = await listCollectionImages(target);
            images.set(updated);
        }
        statusHint.set(`Added to collection. Space for next, B to exit`);
    } catch (err) {
        console.error('Failed to add to collection:', err);
    }
}

function compareSwapFocusedImage(direction: 1 | -1) {
    const imgs = get(images);
    const side = get(compareActiveSide);
    const sel = get(selectedIds);
    const idx = get(focusedIndex);

    if (sel.size >= 2) {
        const selArr = Array.from(sel);
        const targetId = selArr[side];
        const currentIdx = imgs.findIndex(i => i.image.id === targetId);
        const newIdx = Math.max(0, Math.min(imgs.length - 1, currentIdx + direction));
        const newId = imgs[newIdx]?.image.id;
        if (newId && newId !== selArr[1 - side]) {
            selArr[side] = newId;
            selectedIds.set(new Set(selArr));
        }
    } else {
        if (side === 0) {
            const newIdx = Math.max(0, idx + direction);
            if (newIdx !== idx + 1) focusedIndex.set(newIdx);
        } else {
            const rightIdx = idx + 1 + direction;
            if (rightIdx >= 0 && rightIdx < imgs.length && rightIdx !== idx) {
                const newFocused = Math.min(rightIdx - 1, imgs.length - 2);
                focusedIndex.set(Math.max(0, newFocused));
            }
        }
    }
}

function handleCanvasKeys(e: KeyboardEvent) {
    if (!e.metaKey && !e.ctrlKey && !e.altKey) {
        const n = parseInt(e.key);
        if (n >= 1 && n <= 5) {
            e.preventDefault();
            handleStarRating(n);
            return;
        }
    }

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
            moveFocus(-1);
            break;
        case 'j':
        case 'ArrowDown':
            e.preventDefault();
            moveFocus(1);
            break;
        case ' ':
            e.preventDefault();
            if (get(collectMode)) {
                handleCollectModeAdd();
            } else {
                toggleSelect();
            }
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
        case 'Enter':
            e.preventDefault();
            navigateTo('loupe');
            break;
        case 'Escape':
            e.preventDefault();
            navigateBack() || navigateTo('grid');
            break;
        case 'f':
            e.preventDefault();
            toggleFullscreen();
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
            compareSwapFocusedImage(1);
            break;
        case 'k':
        case 'ArrowUp':
            e.preventDefault();
            compareSwapFocusedImage(-1);
            break;
        case 'Tab':
            e.preventDefault();
            compareActiveSide.update(s => (s === 0 ? 1 : 0) as 0 | 1);
            break;
        case '1':
            // Accept left, reject right
            e.preventDefault();
            handleDecision('accept', getCompareActiveIndexForSide(0));
            handleDecision('reject', getCompareActiveIndexForSide(1));
            break;
        case '2':
            // Accept right, reject left
            e.preventDefault();
            handleDecision('accept', getCompareActiveIndexForSide(1));
            handleDecision('reject', getCompareActiveIndexForSide(0));
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
            navigateBack() || navigateTo('grid');
            break;
    }
}

function handleLoupeKeys(e: KeyboardEvent) {
    // Direct 1-5 rating in loupe (no chord needed)
    if (!e.metaKey && !e.ctrlKey && !e.altKey) {
        const n = parseInt(e.key);
        if (n >= 1 && n <= 5) {
            e.preventDefault();
            handleStarRating(n);
            return;
        }
    }

    if (e.key === '[') {
        e.preventDefault();
        const img = get(focusedImage);
        if (img) {
            rotateImage(img.image.id, 270).then(() => {
                window.dispatchEvent(new CustomEvent('image-updated'));
            }).catch(err => console.error('Rotate failed:', err));
        }
        return;
    }
    if (e.key === ']') {
        e.preventDefault();
        const img = get(focusedImage);
        if (img) {
            rotateImage(img.image.id, 90).then(() => {
                window.dispatchEvent(new CustomEvent('image-updated'));
            }).catch(err => console.error('Rotate failed:', err));
        }
        return;
    }
    if (e.key === 'c' && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('enter-crop-mode'));
        return;
    }

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
            navigateBack() || navigateTo('grid');
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
