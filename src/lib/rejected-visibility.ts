import type { ImageWithFile } from './api';
import { loadImagesForCurrentScope } from './image-loading';
import { withDecision, type ImageDecision } from './selection-updates';
import { get } from 'svelte/store';
import { activeSmartCollection, focusedIndex, images, importBatchImageIds, selectedIds, showRejected, showToast, totalCount } from './stores';

export interface VisibleDecisionResult { items: ImageWithFile[]; focusedIndex: number; hidden: boolean; }

export function applyVisibleDecision(items: ImageWithFile[], imageId: string, decision: ImageDecision, includeRejected: boolean, currentFocusedIndex: number): VisibleDecisionResult {
    const changedIndex = items.findIndex(item => item.image.id === imageId);
    if (changedIndex < 0) return { items, focusedIndex: currentFocusedIndex, hidden: false };
    if (decision === 'reject' && !includeRejected) {
        const nextItems = items.filter(item => item.image.id !== imageId);
        const nextFocusedIndex = nextItems.length === 0 ? 0 : Math.min(changedIndex < currentFocusedIndex ? currentFocusedIndex - 1 : currentFocusedIndex, nextItems.length - 1);
        return { items: nextItems, focusedIndex: nextFocusedIndex, hidden: true };
    }
    return { items: items.map(item => item.image.id === imageId ? withDecision(item, decision) : item), focusedIndex: currentFocusedIndex, hidden: false };
}

export function applyDecisionToCurrentView(imageId: string, decision: ImageDecision): VisibleDecisionResult {
    const activeSmart = get(activeSmartCollection);
    // Smart-filter membership is a backend policy. Keep the row optimistically,
    // then let the authoritative query below decide whether it remains visible.
    const includeRejected = get(showRejected) || activeSmart !== null;
    const result = applyVisibleDecision(get(images), imageId, decision, includeRejected, get(focusedIndex));
    images.set(result.items); focusedIndex.set(result.focusedIndex);
    if (result.hidden) {
        selectedIds.update(ids => { if (!ids.has(imageId)) return ids; const next = new Set(ids); next.delete(imageId); return next; });
        importBatchImageIds.update(ids => ids.filter(id => id !== imageId));
        totalCount.update(count => Math.max(0, count - 1));
    }
    if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('cull:decision-changed'));
    if (activeSmart) {
        void loadImagesForCurrentScope({
            resetFocus: false,
            force: true,
            invalidateCache: true,
            throwOnError: true,
        })
            .catch(error => {
                console.error('Failed to refresh smart collection after decision:', error);
                showToast('Failed to refresh smart collection', {
                    detail: String(error),
                    type: 'error',
                    duration: 8000,
                });
            });
    }
    return result;
}
