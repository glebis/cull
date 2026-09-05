import { images, showToast } from './stores';
import { setRating } from './api';
import { invalidateImageCache } from './image-loading';
import { updateSelectionCacheItem } from './selection-view';
import { withRating } from './selection-updates';

// Rating writes are serialized per image id: concurrent saves for the same
// image must reach the database in intent order, or a reload could restore an
// older rating. Different images keep saving concurrently. Slots are removed
// once settled, so the map never grows with the library. Every entry point
// (keyboard, command palette, context menu) saves through this queue so
// writes from different UI surfaces cannot interleave for the same image.
const ratingWriteQueues = new Map<string, Promise<void>>();

function enqueueRatingWrite(imageId: string, run: () => Promise<void>): Promise<void> {
    const previous = ratingWriteQueues.get(imageId);
    // Idle images write immediately; queued ones run after the pending write,
    // which may have failed — an earlier failure must not block later writes.
    const write = previous ? previous.then(run, run) : run();
    ratingWriteQueues.set(imageId, write);
    return write.finally(() => {
        // Clean bookkeeping: drop the slot once settled if we are still the tail.
        if (ratingWriteQueues.get(imageId) === write) ratingWriteQueues.delete(imageId);
    });
}

/**
 * Persist one star rating and repaint the image wherever it now sits.
 *
 * `imageId` and `sessionId` must be captured by the caller at invocation time:
 * the images array and session can change while the save is queued or in
 * flight, so an index captured earlier would go stale. The repaint targets the
 * image by id inside the serialized slot, keeping UI updates in the exact
 * order the writes reach the database; if the image left the current view,
 * nothing is repainted. Failures are surfaced as a visible toast and logged;
 * the promise resolves rather than rejects so callers can await it safely.
 */
export async function saveRating(imageId: string, rating: number, sessionId: string | null): Promise<void> {
    try {
        await enqueueRatingWrite(imageId, async () => {
            await setRating(imageId, rating, sessionId);
            invalidateImageCache();
            images.update(all => {
                const target = all.findIndex(item => item.image.id === imageId);
                // The image left the current view (folder replaced or removed).
                if (target < 0) return all;
                const copy = [...all];
                copy[target] = withRating(copy[target], rating);
                return copy;
            });
            // Selection Mode pages cache full ImageWithFile records: repaint
            // the rating there too so a Source/Shortlist switch cannot
            // restore a stale star count. The remembered focus and scroll of
            // those caches stay intact.
            updateSelectionCacheItem(imageId, item => withRating(item, rating));
        });
    } catch (e) {
        console.error('Failed to set rating:', e);
        showToast('Could not save rating', { detail: String(e), type: 'error', duration: 5000 });
    }
}
