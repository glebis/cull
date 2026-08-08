// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { requestTrashImages, TRASH_IMAGES_REQUESTED_EVENT } from './trash-actions';

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
});
