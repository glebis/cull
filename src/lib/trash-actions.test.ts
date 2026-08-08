// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import {
    requestTrashImages,
    resolveTrashRequestIds,
    TRASH_IMAGES_REQUESTED_EVENT,
} from './trash-actions';

describe('shared Trash request', () => {
    it('dispatches one canonical deduplicated request for every caller', () => {
        const listener = vi.fn();
        window.addEventListener(TRASH_IMAGES_REQUESTED_EVENT, listener);

        requestTrashImages(['img-two', 'img-one', 'img-two', '']);

        expect(listener).toHaveBeenCalledTimes(1);
        expect((listener.mock.calls[0][0] as CustomEvent).detail).toEqual({
            imageIds: ['img-two', 'img-one'],
        });
        window.removeEventListener(TRASH_IMAGES_REQUESTED_EVENT, listener);
    });

    it('preserves explicit IDs that are outside the currently loaded view', () => {
        expect(resolveTrashRequestIds(['stale-proposal-id'], ['focused-id']))
            .toEqual(['stale-proposal-id']);
    });

    it('uses the focused selection only when no explicit IDs were requested', () => {
        expect(resolveTrashRequestIds([], ['focused-id', 'focused-id']))
            .toEqual(['focused-id']);
    });
});
