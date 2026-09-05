import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import type { ImageWithFile } from './api';
import { focusedIndex, images, toasts } from './stores';

const mocks = vi.hoisted(() => ({
    setDecision: vi.fn().mockResolvedValue(undefined),
    setRating: vi.fn(),
    undo: vi.fn().mockResolvedValue(null),
    redo: vi.fn().mockResolvedValue(null),
}));

vi.mock('./api', () => ({
    addToCollection: vi.fn(),
    analyzeImages: vi.fn(),
    checkOllama: vi.fn(),
    copyImageToClipboard: vi.fn(),
    createCollection: vi.fn(),
    detectNsfw: vi.fn(),
    detectObjects: vi.fn(),
    findSimilarImages: vi.fn(),
    getAppSetting: vi.fn(),
    getClientFeedback: vi.fn(),
    getImageByPath: vi.fn(),
    getImagesByIds: vi.fn(),
    getOllamaConfig: vi.fn(),
    isNudenetAvailable: vi.fn(),
    isYoloAvailable: vi.fn(),
    listCanvases: vi.fn(),
    listClientFeedback: vi.fn(),
    listCollections: vi.fn(),
    listImageIdsMissingDetection: vi.fn(),
    listImageIdsMissingVision: vi.fn(),
    pasteImageFromClipboard: vi.fn(),
    redo: mocks.redo,
    rotateImage: vi.fn(),
    saveTextToPath: vi.fn(),
    setClientFeedback: vi.fn(),
    setDecision: mocks.setDecision,
    setRating: mocks.setRating,
    undo: mocks.undo,
    validateSessionFolder: vi.fn(),
}));
vi.mock('./image-loading', () => ({
    clearImageScope: vi.fn(),
    invalidateImageCache: vi.fn(),
    loadAllImages: vi.fn(),
    loadImagesForCurrentScope: vi.fn(),
    resetImagePaging: vi.fn(),
}));
vi.mock('./shortcut-reminders', () => ({
    recordShortcutUse: vi.fn(),
    VIEW_CYCLE_SHORTCUT_REMINDER_ID: 'view-cycle',
}));

import { handleStarRating } from './keys';
import { getCommandPaletteItems, runCommandPaletteItem } from './command-palette';
import { saveRating } from './rating-actions';

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

/** Runs a palette rating command against the image at the current focus. */
async function runPaletteRating(rating: number): Promise<unknown> {
    const item = getCommandPaletteItems('all').find(i => i.id === `image.rating.${rating}`);
    if (!item) throw new Error(`palette item image.rating.${rating} not found`);
    return runCommandPaletteItem(item);
}

beforeEach(() => {
    vi.clearAllMocks();
    pendingSaves = [];
    toasts.set([]);
    vi.stubGlobal('HTMLElement', class {});
    vi.stubGlobal('HTMLInputElement', class {});
    vi.stubGlobal('HTMLTextAreaElement', class {});
    vi.stubGlobal('HTMLSelectElement', class {});
    vi.stubGlobal('document', { querySelector: vi.fn(() => null), fullscreenElement: null });
    vi.stubGlobal('window', new EventTarget());
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
        getItem: (key: string) => store.get(key) ?? null,
        setItem: (key: string, value: string) => store.set(key, value),
        removeItem: (key: string) => store.delete(key),
        clear: () => store.clear(),
    });
    focusedIndex.set(0);
});

describe('shared rating queue across entry points', () => {
    it('serializes keyboard and palette writes per image across navigation; the older persisted write never overwrites later intent', async () => {
        images.set([image('a'), image('b')]);
        focusedIndex.set(0);

        // Keyboard write for 'a' is slow and stays in flight.
        const save3 = saveInFlight();
        const keyboard3 = handleStarRating(3);
        expect(mocks.setRating).toHaveBeenLastCalledWith('a', 3, null);

        // Navigate to 'b' while the write is in flight, then rate via palette.
        focusedIndex.set(1);
        const save5 = saveInFlight();
        const palette5 = runPaletteRating(5);
        // Different image: the palette write starts without waiting for 'a'.
        expect(mocks.setRating).toHaveBeenCalledTimes(2);
        expect(mocks.setRating).toHaveBeenLastCalledWith('b', 5, null);

        // Navigate back to 'a' and rate via palette: this write shares the
        // keyboard entry's per-image slot and must wait for it.
        focusedIndex.set(0);
        const save4 = saveInFlight();
        const palette4 = runPaletteRating(4);
        expect(mocks.setRating).toHaveBeenCalledTimes(2);

        // Settling the keyboard write releases the queued palette write.
        save3.resolve();
        await keyboard3;
        await vi.waitFor(() => expect(mocks.setRating).toHaveBeenCalledTimes(3));
        expect(mocks.setRating).toHaveBeenLastCalledWith('a', 4, null);

        save4.resolve();
        await palette4;
        save5.resolve();
        await palette5;

        // Database order: a:3 then b:5 (concurrent) then a:4 — the newest
        // 'a' intent persisted last, so a reload cannot restore the older 3.
        expect(mocks.setRating).toHaveBeenNthCalledWith(1, 'a', 3, null);
        expect(mocks.setRating).toHaveBeenNthCalledWith(2, 'b', 5, null);
        expect(mocks.setRating).toHaveBeenNthCalledWith(3, 'a', 4, null);
        expect(ratingOf(get(images)[0])).toBe(4);
        expect(ratingOf(get(images)[1])).toBe(5);
    });

    it('a failed keyboard save does not block a queued palette write for the same image', async () => {
        images.set([image('a')]);
        const save2 = saveInFlight();
        const keyboard2 = handleStarRating(2);
        const save5 = saveInFlight();
        const palette5 = runPaletteRating(5);
        expect(mocks.setRating).toHaveBeenCalledTimes(1);

        save2.reject(new Error('database unavailable'));
        await keyboard2; // resolves; the failure is surfaced, not thrown
        await vi.waitFor(() => expect(mocks.setRating).toHaveBeenCalledTimes(2));
        expect(mocks.setRating).toHaveBeenLastCalledWith('a', 5, null);

        save5.resolve();
        await palette5;
        expect(ratingOf(get(images)[0])).toBe(5);
        const failure = get(toasts).find(t => t.type === 'error');
        expect(failure?.message).toBe('Could not save rating');
        expect(failure?.detail).toContain('database unavailable');
    });

    it('repaints by id when the shared handler saves from a stale navigation target', async () => {
        images.set([image('a'), image('b')]);
        focusedIndex.set(0);
        const save = saveInFlight();
        const saving = saveRating('a', 4, null);
        // The array is reordered while the save is in flight.
        images.set([image('c'), image('a'), image('b')]);
        save.resolve();
        await saving;

        const all = get(images);
        expect(ratingOf(all[0])).toBeNull();
        expect(ratingOf(all[1])).toBe(4); // repainted at its new position
        expect(ratingOf(all[2])).toBeNull();
    });
});