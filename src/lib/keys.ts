import { get } from 'svelte/store';
import {
    images, selectedIds, focusedIndex, thumbnailSize, statusHint, viewMode,
    compareActiveSide, compareImages, loupeScale, loupePanX, loupePanY,
    sidebarVisible, gridPreset, gridGap, GRID_PRESETS, zenMode, compareImageOnly, exportImageOnly,
    collections, collectMode, collectModeTarget, activeCollection,
    showDetectionBoxes, showDetectionInspector, nsfwMode,
    navigateTo, navigateBack, searchOpen, shortcutsOpen, focusedImage, activeSession,
    requestTextInput, requestCollectionTarget, selectionAnchorIndex, resetLoupeTransform,
    activeFolder,
} from './stores';
import { tabCycleOrder } from './plugins/tab-registry';
import { computeCompareSwap, nextComparePresentationState } from './compare-utils';
import { nextExportPresentationState } from './presentation-utils';
import type { NsfwMode } from './stores';
import type { ViewMode } from './stores';
import { setRating, setDecision, createCollection, addToCollection, listCollections, rotateImage, undo, redo, copyImageToClipboard, pasteImageFromClipboard } from './api';
import { showToast } from './stores';
import { invalidateImageCache, loadImagesForCurrentScope } from './image-loading';
import { focusImagePath } from './transform-results';
import { commandForKeyboardEvent, openCommandPalette, runCommandPaletteItem } from './command-palette';
import { recordShortcutUse, VIEW_CYCLE_SHORTCUT_REMINDER_ID } from './shortcut-reminders';
import { withDecision, withRating, type ImageDecision } from './selection-updates';
import { pasteDestinationForContext } from './clipboard-actions';

let waitingForStar = false;

function viewModeCycle(): ViewMode[] {
    return tabCycleOrder();
}

/** Test seam: exposes the derived cycle without dispatching key events. */
export function viewModeCycleForTest(): ViewMode[] {
    return viewModeCycle();
}

const VIEW_MODE_KEYS: Record<string, ViewMode> = {
    '1': 'grid',
    '2': 'loupe',
    '3': 'compare',
    '4': 'canvas',
    '5': 'lineage',
    '6': 'embeddings',
    '7': 'export',
    '8': 'tinder',
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

function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    return target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        target.isContentEditable;
}

function cycleViewMode(direction: 1 | -1) {
    const currentMode = get(viewMode);
    const cycle = viewModeCycle();
    const currentIndex = cycle.indexOf(currentMode);
    const nextIndex = currentIndex >= 0
        ? (currentIndex + direction + cycle.length) % cycle.length
        : direction === 1 ? 0 : cycle.length - 1;
    navigateTo(cycle[nextIndex]);
    recordShortcutUse(VIEW_CYCLE_SHORTCUT_REMINDER_ID);
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
    selectionAnchorIndex.set(idx);
}

function showSelectionHistoryStatus(label: string) {
    const count = get(selectedIds).size;
    statusHint.set(`${label}: ${count} selected`);
    setTimeout(() => {
        if (get(statusHint) === `${label}: ${count} selected`) statusHint.set(null);
    }, 2000);
}

export async function handleStarRating(n: number, imageIndex?: number) {
    const imgs = get(images);
    const idx = imageIndex ?? get(focusedIndex);
    const img = imgs[idx];
    if (!img) return;
    try {
        await setRating(img.image.id, n, get(activeSession)?.id ?? null);
        invalidateImageCache();
        images.update(all => {
            const copy = [...all];
            copy[idx] = withRating(copy[idx], n);
            return copy;
        });
    } catch (e) {
        console.error('Failed to set rating:', e);
    }
}

export async function handleDecision(decision: ImageDecision, imageIndex?: number) {
    const imgs = get(images);
    const idx = imageIndex ?? get(focusedIndex);
    const img = imgs[idx];
    if (!img) return;
    try {
        await setDecision(img.image.id, decision, get(activeSession)?.id ?? null);
        invalidateImageCache();
        images.update(all => {
            const copy = [...all];
            copy[idx] = withDecision(copy[idx], decision);
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

function filenameForPath(path: string): string {
    return path.split('/').filter(Boolean).pop() ?? path;
}

function currentClipboardImage() {
    if (get(viewMode) === 'compare') {
        return get(images)[getCompareActiveIndex()] ?? null;
    }
    return get(focusedImage);
}

async function handleCopyCurrentImage() {
    const img = currentClipboardImage();
    if (!img) {
        showToast('No current image to copy', { type: 'warning', duration: 3000 });
        return;
    }

    try {
        await copyImageToClipboard(img.image.id);
        showToast('Copied image', { detail: filenameForPath(img.path), type: 'success', duration: 2500 });
    } catch (err) {
        console.error('Failed to copy image:', err);
        showToast('Could not copy image', { detail: String(err), type: 'error', duration: 5000 });
    }
}

async function handlePasteImage() {
    const destination = pasteDestinationForContext(get(activeFolder), get(focusedImage)?.path ?? null);
    if (!destination) {
        showToast('No folder available for paste', { type: 'warning', duration: 3500 });
        return;
    }

    try {
        const result = await pasteImageFromClipboard(destination, get(activeSession)?.id ?? null);
        invalidateImageCache();
        await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        showToast('Pasted image', { detail: filenameForPath(result.path), type: 'success', duration: 3500 });
    } catch (err) {
        console.error('Failed to paste image:', err);
        showToast('Could not paste image', { detail: String(err), type: 'error', duration: 5000 });
    }
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
    resetLoupeTransform();
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
    if (e.key.toLowerCase() === 'k' && e.metaKey && !e.shiftKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        openCommandPalette('all');
        return;
    }

    if (e.key.toLowerCase() === 'p' && e.metaKey && !e.shiftKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        openCommandPalette('commands');
        return;
    }

    if (e.key.toLowerCase() === 'p' && e.metaKey && e.shiftKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        openCommandPalette('commands');
        return;
    }

    const mode = get(viewMode);

    if (mode === 'export' && e.key === 'Enter' && e.metaKey && !e.shiftKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('cull-export-launch'));
        return;
    }

    if (isEditableTarget(e.target)) return;

    const tag = (e.target as HTMLElement)?.tagName;
    if (['BUTTON', 'A', 'SELECT'].includes(tag) && (e.key === ' ' || e.key === 'Enter')) return;

    if (e.key.toLowerCase() === 'c' && e.metaKey && !e.shiftKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        void handleCopyCurrentImage();
        return;
    }

    if (e.key.toLowerCase() === 'v' && e.metaKey && !e.shiftKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        void handlePasteImage();
        return;
    }

    // Bare Tab is reserved for native focus traversal — never consume it.
    // View cycling lives on Ctrl+Tab / Ctrl+Shift+Tab (plus Cmd+1-7).
    if (e.key === 'Tab' && e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        cycleViewMode(e.shiftKey ? -1 : 1);
        return;
    }

    // '?' (Shift+/) opens the keyboard-shortcuts help
    if (e.key === '?' && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        shortcutsOpen.set(true);
        return;
    }

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

    const commandItem = commandForKeyboardEvent(e);
    if (commandItem) {
        e.preventDefault();
        runCommandPaletteItem(commandItem)
            .catch(err => console.error('Failed to run command hotkey:', err));
        return;
    }

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

    // Shift+. (>) toggles zen mode; compare/export cycle through an image-only state too.
    if (e.key === '>' || (e.shiftKey && e.key === '.')) {
        e.preventDefault();
        if (mode === 'compare') {
            const next = nextComparePresentationState({
                zen: get(zenMode),
                imageOnly: get(compareImageOnly),
            });
            zenMode.set(next.zen);
            compareImageOnly.set(next.imageOnly);
            return;
        }
        if (mode === 'export') {
            const next = nextExportPresentationState({
                zen: get(zenMode),
                imageOnly: get(exportImageOnly),
            });
            zenMode.set(next.zen);
            exportImageOnly.set(next.imageOnly);
            return;
        }
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

    if (e.metaKey && e.key === '0' && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        e.preventDefault();
        resetLoupeZoom();
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

    // View mode switching with Cmd+number
    if (VIEW_MODE_KEYS[e.key] && e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        navigateTo(VIEW_MODE_KEYS[e.key]);
        return;
    }

    // Undo: Cmd+Z
    if (e.key.toLowerCase() === 'z' && e.metaKey && !e.shiftKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        if (mode === 'grid' && selectedIds.undo()) {
            showSelectionHistoryStatus('Selection restored');
            return;
        }
        undo().then(label => {
            if (label) {
                showToast(`Undone: ${label}`, { type: 'info', duration: 4000 });
                window.dispatchEvent(new CustomEvent('reload-images'));
            }
        });
        return;
    }

    // Redo: Cmd+Shift+Z
    if (e.key.toLowerCase() === 'z' && e.metaKey && e.shiftKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        if (mode === 'grid' && selectedIds.redo()) {
            showSelectionHistoryStatus('Selection redone');
            return;
        }
        redo().then(label => {
            if (label) {
                showToast(`Redone: ${label}`, { type: 'info', duration: 4000 });
                window.dispatchEvent(new CustomEvent('reload-images'));
            }
        });
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
        case 'tinder':
            handleTinderKeys(e);
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
        case 'f':
            e.preventDefault();
            toggleFullscreen();
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

    const name = await requestTextInput({
        title: inverse ? 'Create Collection from Unselected' : 'Create Collection from Selection',
        label: 'Collection name',
        description: `${imageIds.length} images will be added.`,
        placeholder: 'Collection name',
        confirmLabel: 'Create',
    });
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

    const target = await requestCollectionTarget({
        title: 'Collect Mode',
        description: cols.length > 0
            ? 'Choose the collection that Space will add images to, or create a new one.'
            : 'Create a collection that Space will add images to.',
        collections: cols,
        confirmLabel: 'Start',
    });
    if (!target) return;

    if (target.type === 'existing') {
        targetId = target.collectionId;
    } else {
        try {
            targetId = await createCollection(target.name);
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
        invalidateImageCache();
        const c = await listCollections();
        collections.set(c);
        // If we're viewing this collection, refresh
        if (get(activeCollection) === target) {
            await loadImagesForCurrentScope({ resetFocus: false, force: true });
        }
        statusHint.set(`Added to collection. Space for next, B to exit`);
    } catch (err) {
        console.error('Failed to add to collection:', err);
    }
}

function compareSwapFocusedImage(direction: 1 | -1) {
    const imgs = get(images);
    const imageIds = imgs.map(i => i.image.id);
    const result = computeCompareSwap(
        imageIds,
        get(selectedIds),
        get(focusedIndex),
        get(compareActiveSide),
        direction
    );
    if (result) {
        selectedIds.set(result.newSelectedIds);
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
            rotateImage(img.image.id, 270).then(focusImagePath).catch(err => console.error('Rotate failed:', err));
        }
        return;
    }
    if (e.key === ']') {
        e.preventDefault();
        const img = get(focusedImage);
        if (img) {
            rotateImage(img.image.id, 90).then(focusImagePath).catch(err => console.error('Rotate failed:', err));
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
        case ' ':
            e.preventDefault();
            toggleSelect();
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

function handleTinderKeys(e: KeyboardEvent) {
    switch (e.key) {
        case 'ArrowLeft':
        case 'h':
            e.preventDefault();
            window.dispatchEvent(new CustomEvent('tinder-choose', { detail: 'left' }));
            break;
        case 'ArrowRight':
        case 'l':
            e.preventDefault();
            window.dispatchEvent(new CustomEvent('tinder-choose', { detail: 'right' }));
            break;
        case 'ArrowDown':
        case 'j':
            e.preventDefault();
            window.dispatchEvent(new CustomEvent('tinder-choose', { detail: 'skip' }));
            break;
        case 'Escape':
            e.preventDefault();
            navigateBack() || navigateTo('grid');
            break;
        case 'z':
            e.preventDefault();
            window.dispatchEvent(new CustomEvent('tinder-undo'));
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
