import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import type { ImageWithFile } from './api';
import { focusedIndex, images, statusHint, toasts, viewMode } from './stores';

const mocks = vi.hoisted(() => ({
    commandForKeyboardEvent: vi.fn(),
    runCommandPaletteItem: vi.fn(),
    setDecision: vi.fn().mockResolvedValue(undefined),
    setRating: vi.fn(),
    undo: vi.fn().mockResolvedValue(null),
}));

vi.mock('./api', () => ({
    addToCollection: vi.fn(), createCollection: vi.fn(), listCollections: vi.fn(),
    pasteImageFromClipboard: vi.fn(), redo: vi.fn(), rotateImage: vi.fn(),
    setDecision: mocks.setDecision, setRating: mocks.setRating, undo: mocks.undo,
}));
vi.mock('./command-palette', () => ({
    commandForKeyboardEvent: mocks.commandForKeyboardEvent,
    openCommandPalette: vi.fn(),
    runCommandPaletteItem: mocks.runCommandPaletteItem,
}));
vi.mock('./image-loading', () => ({
    invalidateImageCache: vi.fn(),
    loadImagesForCurrentScope: vi.fn(),
}));
vi.mock('./shortcut-reminders', () => ({
    recordShortcutUse: vi.fn(),
    VIEW_CYCLE_SHORTCUT_REMINDER_ID: 'view-cycle',
}));

import { handleKeydown, handleStarRating } from './keys';

function image(id: string): ImageWithFile {
    return {
        image: {
            id, sha256_hash: `hash-${id}`, width: 100, height: 100, format: 'png',
            file_size: 100, created_at: '2026-08-08T00:00:00Z',
            imported_at: '2026-08-08T00:00:00Z', ai_prompt: null, raw_metadata: null,
        },
        path: `/images/${id}.png`, thumbnail_path: null, selection: null,
        source_label: null, missing_at: null,
    };
}

function ratingOf(item: ImageWithFile | undefined): number | null {
    return item?.selection?.star_rating ?? null;
}

function keyEvent(key: string, modifiers: Partial<KeyboardEvent> = {}): KeyboardEvent {
    return {
        key, metaKey: false, shiftKey: false, ctrlKey: false, altKey: false,
        target: null, preventDefault: vi.fn(),
        ...modifiers,
    } as unknown as KeyboardEvent;
}

interface DeferredSave {
    promise: Promise<void>;
    resolve: () => void;
    reject: (error: unknown) => void;
}

function deferredSave(): DeferredSave {
    let resolve!: () => void;
    let reject!: (error: unknown) => void;
    const promise = new Promise<void>((res, rej) => { resolve = res; reject = rej; });
    return { promise, resolve, reject };
}

/** Queue of controllable setRating results, consumed in call order. */
let pendingSaves: DeferredSave[] = [];

function saveInFlight(): DeferredSave {
    const deferred = deferredSave();
    pendingSaves.push(deferred);
    mocks.setRating.mockImplementationOnce(() => deferred.promise);
    return deferred;
}

beforeEach(() => {
    vi.clearAllMocks();
    pendingSaves = [];
    toasts.set([]);
    statusHint.set(null);
    vi.stubGlobal('HTMLElement', class {});
    vi.stubGlobal('HTMLInputElement', class {});
    vi.stubGlobal('HTMLTextAreaElement', class {});
    vi.stubGlobal('HTMLSelectElement', class {});
    vi.stubGlobal('document', { querySelector: vi.fn(() => null), fullscreenElement: null });
    vi.stubGlobal('window', new EventTarget());
    focusedIndex.set(0);
    viewMode.set('grid');
});

describe('star rating save ordering', () => {
    it('repaints the rated image by id after a folder replacement moves it', async () => {
        images.set([image('a'), image('b'), image('c')]);
        const save = saveInFlight();

        const rating = handleStarRating(5);
        // The folder is replaced while the save is in flight: the rated image
        // keeps its id but moves, and unrelated images take the old indexes.
        images.set([image('d'), image('a'), image('e')]);
        save.resolve();
        await rating;

        const all = get(images);
        expect(all[0].image.id).toBe('d');
        expect(ratingOf(all[0])).toBeNull(); // 'd' must not inherit the rating
        expect(all[1].image.id).toBe('a');
        expect(ratingOf(all[1])).toBe(5);
        expect(mocks.setRating).toHaveBeenCalledWith('a', 5, null);
    });

    it('does not repaint anything when the image was removed mid-save', async () => {
        images.set([image('a'), image('b')]);
        const save = saveInFlight();

        const rating = handleStarRating(4);
        images.set([image('b')]); // 'a' left the current view
        save.resolve();
        await rating;

        const all = get(images);
        expect(all).toHaveLength(1);
        expect(all[0].image.id).toBe('b');
        expect(ratingOf(all[0])).toBeNull();
    });

    it('serializes same-image saves so the newest intent reaches the database last', async () => {
        images.set([image('a')]);
        const save3 = saveInFlight();
        const rating3 = handleStarRating(3);
        const save5 = saveInFlight();
        const rating5 = handleStarRating(5);

        // The second backend call waits: only the first write is in flight.
        expect(mocks.setRating).toHaveBeenCalledTimes(1);
        expect(mocks.setRating).toHaveBeenLastCalledWith('a', 3, null);

        save3.resolve();
        await rating3;
        await vi.waitFor(() => expect(mocks.setRating).toHaveBeenCalledTimes(2));
        expect(mocks.setRating).toHaveBeenLastCalledWith('a', 5, null);

        save5.resolve();
        await rating5;
        // Writes persisted 3 then 5; the newest successful intent is both in
        // the database call order and in the UI.
        expect(mocks.setRating).toHaveBeenNthCalledWith(1, 'a', 3, null);
        expect(mocks.setRating).toHaveBeenNthCalledWith(2, 'a', 5, null);
        expect(ratingOf(get(images)[0])).toBe(5);
    });

    it('a failed save does not block the next one for the same image', async () => {
        images.set([image('a')]);
        const save3 = saveInFlight();
        const rating3 = handleStarRating(3);
        const save5 = saveInFlight();
        const rating5 = handleStarRating(5);

        save3.reject(new Error('database unavailable'));
        await rating3; // handleStarRating reports the failure and resolves
        // The queued write still runs after the earlier failure.
        await vi.waitFor(() => expect(mocks.setRating).toHaveBeenCalledTimes(2));
        expect(mocks.setRating).toHaveBeenLastCalledWith('a', 5, null);

        save5.resolve();
        await rating5;
        expect(ratingOf(get(images)[0])).toBe(5);
    });

    it('runs a fresh write immediately once earlier writes have settled', async () => {
        images.set([image('a')]);
        const save3 = saveInFlight();
        const rating3 = handleStarRating(3);
        save3.resolve();
        await rating3;

        // Queue bookkeeping is cleaned: the next write does not queue behind a
        // settled slot, so its backend call happens synchronously.
        const save5 = saveInFlight();
        handleStarRating(5);
        expect(mocks.setRating).toHaveBeenCalledTimes(2);
        expect(mocks.setRating).toHaveBeenLastCalledWith('a', 5, null);

        save5.resolve();
    });

    it('keeps writes to different images concurrent', async () => {
        images.set([image('a'), image('b')]);
        focusedIndex.set(0);
        const saveA = saveInFlight();
        const ratingA = handleStarRating(3);
        focusedIndex.set(1);
        const saveB = saveInFlight();
        const ratingB = handleStarRating(4);

        // Both backend calls started without waiting for each other.
        expect(mocks.setRating).toHaveBeenCalledTimes(2);
        expect(mocks.setRating).toHaveBeenNthCalledWith(1, 'a', 3, null);
        expect(mocks.setRating).toHaveBeenNthCalledWith(2, 'b', 4, null);

        saveB.resolve();
        await ratingB;
        saveA.resolve();
        await ratingA;
        expect(ratingOf(get(images)[0])).toBe(3);
        expect(ratingOf(get(images)[1])).toBe(4);
    });

    it('surfaces a failed save as a visible error toast, not just the console', async () => {
        images.set([image('a')]);
        const save = saveInFlight();
        const rating = handleStarRating(2);

        save.reject(new Error('database locked'));
        await rating;
        const failure = get(toasts).find(t => t.type === 'error');
        expect(failure?.message).toBe('Could not save rating');
        expect(failure?.detail).toContain('database locked');
        expect(ratingOf(get(images)[0])).toBeNull();
    });

    it('keeps the grid rating chord targeting the focused image', async () => {
        images.set([image('a'), image('b')]);
        focusedIndex.set(1);
        mocks.commandForKeyboardEvent.mockReturnValue(null);

        handleKeydown(keyEvent('s'));
        expect(get(statusHint)).toBe('Rate: press 1-5');
        handleKeydown(keyEvent('4'));

        await vi.waitFor(() => expect(mocks.setRating).toHaveBeenCalledWith('b', 4, null));
        expect(ratingOf(get(images)[1])).toBe(4);
        expect(ratingOf(get(images)[0])).toBeNull();
    });
});